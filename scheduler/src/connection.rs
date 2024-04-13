use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::ops::DerefMut;
use std::sync::Arc;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use quinn::{Connecting, Connection, Endpoint, RecvStream, SendStream};
use rustls::Certificate;
use shared::certificate::{SERVICE_TYPE_OID, ServiceType};
use shared::node_manager::{NodeManagerRequest, NodeManagerResponse, WorkerInfo};
use shared::scheduler::{SchedulerRequest, SchedulerResponse};
use shared::utils::bin_codec::BinCodec;
use shared::utils::channel::stream_channel;
use shared::utils::retry::{establish_connection};
use shared::worker::{WorkerRequest, WorkerResponse};
use tokio::{pin, select};
use tokio::sync::{Mutex, OwnedMutexGuard};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::sync::{CancellationToken, PollSender};
use tracing::error;
use x509_parser::certificate::X509Certificate;
use x509_parser::oid_registry::Oid;
use x509_parser::traits::FromDer;
use crate::node_manager::NodeManagerEvent;
use crate::nodes::{Node, WorkerEvent};
use crate::task_manager::{TaskManagerConnection, TaskManagerEvent};

pub async fn accept_incoming(endpoint: Endpoint, cancellation_token: CancellationToken, queue: Arc<Mutex<(ReceiverStream<SchedulerResponse>,PollSender<TaskManagerEvent>)>>) {
   loop {
       select! {
           Some(connecting) = endpoint.accept() => {
               tokio::spawn(handle_connection(connecting, queue.clone(), cancellation_token.child_token()));
           },
           _ = cancellation_token.cancelled() => return,
           else => return
        }
   }
}
pub async fn handle_connection(connecting: Connecting, mut queue: Arc<Mutex<(ReceiverStream<SchedulerResponse>,PollSender<TaskManagerEvent>)>>, cancellation_token: CancellationToken){
    let a: Result<_, _> = try {
        let connection = connecting.await?;
        // let certificate = connection
        //     .peer_identity()
        //     .and_then(|x| x.downcast::<Vec<Certificate>>().ok())
        //     .and_then(|mut x| x.first().cloned())
        //     .ok_or(io::Error::from(ErrorKind::PermissionDenied))?;
        // let (_, certificate) = X509Certificate::from_der(certificate.as_ref())
        //     .map_err(|err| io::Error::new(ErrorKind::InvalidData, err))?;
        // let service_type = parse_service_type(&certificate)?;
        // match service_type {
        //     Some(ServiceType::TaskManager) => {
        handle_task_manager_queue(connection, queue, cancellation_token).await?
        //     },
        //     _ => Err(io::Error::from(ErrorKind::Unsupported))?,
        // }
    };
    let _a = match a {
        Ok(a) => a,
        Err::<_, anyhow::Error>(err) => {
            error!(%err, "Connection error");
            return;
        }
    };

}

pub async fn handle_task_manager_queue(connecting: Connection, mut queue: Arc<Mutex<(ReceiverStream<SchedulerResponse>,PollSender<TaskManagerEvent>)>>, cancellation_token: CancellationToken) -> Result<(), anyhow::Error>{
    let (send_stream, recv_stream) = connecting.accept_bi().await?;
    let mut seize = queue.lock_owned();
    let mut request_stream = FramedRead::new(recv_stream, BinCodec::new());
    let mut response_sink = FramedWrite::new(send_stream, BinCodec::new());
    pin!(seize);
    let lock = loop {
        select! {
            guard = &mut seize => {
                response_sink.send(SchedulerResponse::SchedulerFree).await?;

                break guard;
            },
            res = request_stream.next() => {
                match res{
                    Some(Ok(message)) => {
                        response_sink.send(SchedulerResponse::SchedulerBusy).await?;
                    },
                    _ => {
                        return Err(io::Error::from(ErrorKind::ConnectionAborted).into())
                    }
                }
            },
            _ = cancellation_token.cancelled() => return Ok(())
        }
    };

    handle_task_manager(request_stream,response_sink,lock, cancellation_token, connecting.remote_address()).await
}

pub async fn handle_task_manager(
    request_stream: FramedRead<RecvStream, BinCodec<SchedulerRequest>>, response_sink: FramedWrite<SendStream, BinCodec<SchedulerResponse>>,
    mut lock: OwnedMutexGuard<(ReceiverStream<SchedulerResponse>, PollSender<TaskManagerEvent>)>, cancellation_token: CancellationToken, addr: SocketAddr)
    -> Result<(), anyhow::Error>{
    let (response_stream, request_sink) = lock.deref_mut();
    request_sink.send(TaskManagerEvent::Connected(addr)).await?;

    let mut request_sink_ = request_sink
        .sink_map_err(|x| io::Error::from(ErrorKind::BrokenPipe));
    let forward_request = request_stream
        .map_ok(|message| TaskManagerEvent::Message(message))
        .forward(request_sink_);
    let forward_response = response_stream
        .map(Result::Ok)
        .forward(response_sink);

    select! {
        _ = forward_request => {
            request_sink.send(TaskManagerEvent::Disconnect).await?;
            Err(io::Error::from(ErrorKind::ConnectionRefused).into())
        },
        _ = forward_response => {
             request_sink.send(TaskManagerEvent::Disconnect).await?;

             Err(io::Error::from(ErrorKind::ConnectionRefused).into())
        }
        _ = cancellation_token.cancelled() => Ok(())
    }
}

pub async fn handle_node(response_sink: PollSender<WorkerEvent>, endpoint: Endpoint, worker_info: WorkerInfo){
    let result = try {
        let (request_stream, sink) = stream_channel(1024);
        let (send_stream, recv_stream) = establish_connection(&endpoint, worker_info.address, "node").await?;
        let response_stream = FramedRead::new(recv_stream, BinCodec::<WorkerResponse>::new());
        let request_sink = FramedWrite::new(send_stream, BinCodec::<WorkerRequest>::new());
        let cancellation_token = CancellationToken::new();
        let mut response_sink = response_sink
            .sink_map_err(|x| io::Error::from(ErrorKind::BrokenPipe));
        let node = Node{
            sink,
            cancellation_token: cancellation_token.clone()
        };
        response_sink.send(WorkerEvent::Connected(worker_info, node)).await?;
        let forward_response = response_stream
            .map_ok(|message| WorkerEvent::Message(worker_info.worker_id, message))
            .forward(&mut response_sink);

        let forward_request = request_stream
            .map(Result::Ok)
            .forward(request_sink);

        select! {
                    _ = forward_request => {
                        response_sink.send(WorkerEvent::Disconnected(worker_info.worker_id)).await?;

                        Err(io::Error::from(ErrorKind::ConnectionRefused))?
                    },
                    _ = forward_response => {
                        response_sink.send(WorkerEvent::Disconnected(worker_info.worker_id)).await?;

                        Err(io::Error::from(ErrorKind::ConnectionRefused))?
                    }
                    _ = cancellation_token.cancelled() => {}
                }
    };

    match result {
        Ok(a) => (),
        Err::<_, anyhow::Error>(err) => {
            error!(%err, "Connection error");

            return;
        }
    }
}

pub async fn handle_node_manager(endpoint: Endpoint, addr: SocketAddr, cancellation_token: CancellationToken, request_stream: ReceiverStream<NodeManagerRequest>, mut response_sink: PollSender<NodeManagerEvent>){
    let a: Result<_, _> = try {
        let Ok((send_stream, recv_stream)) = establish_connection(&endpoint, addr, "node manager").await else {
            response_sink.send(NodeManagerEvent::ConnectionNotEstablished).await?;
            return;
        };
        let response_stream = FramedRead::new(recv_stream, BinCodec::<NodeManagerResponse>::new());
        let mut request_sink = FramedWrite::new(send_stream, BinCodec::<NodeManagerRequest>::new());
        let mut response_sink = response_sink
            .sink_map_err(|x| io::Error::from(ErrorKind::BrokenPipe));
        request_sink.send(NodeManagerRequest::HelloMessage).await?;
        response_sink.send(NodeManagerEvent::Connected).await?;
        let forward_response = response_stream
            .map_ok(|message| NodeManagerEvent::Message(message))
            .forward(&mut response_sink);

        let forward_request = request_stream
            .map(Result::Ok)
            .forward(request_sink);

        select! {
                    _ = forward_request => {
                        response_sink.send(NodeManagerEvent::Disconnected).await?;

                        Err(io::Error::from(ErrorKind::ConnectionRefused))?
                    },
                    _ = forward_response => {
                         response_sink.send(NodeManagerEvent::Disconnected).await?;


                         Err(io::Error::from(ErrorKind::ConnectionRefused))?
                    }
                    _ = cancellation_token.cancelled() => {}
                }
    };

    let _a = match a {
        Ok(a) => a,
        Err::<_, anyhow::Error>(err) => {
            error!(%err, "Connection error");
            return;
        }
    };
}

fn parse_service_type(certificate: &X509Certificate) -> io::Result<Option<ServiceType>>{
    let service_type_oid = Oid::from(SERVICE_TYPE_OID).unwrap();
    let service_type = certificate
        .tbs_certificate
        .find_extension(&service_type_oid)
        .and_then(|x| {
            bincode::serde::decode_from_slice::<ServiceType, _>(
                x.value,
                bincode::config::standard(),
            )
                .ok()
        })
        .map(|x| x.0);

    Ok(service_type)
}

