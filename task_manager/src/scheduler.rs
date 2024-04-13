use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use futures_util::{SinkExt, StreamExt, TryFutureExt};
use quinn::{Endpoint, RecvStream, SendStream};
use shared::scheduler::{SchedulerRequest, SchedulerResponse};
use shared::utils::bin_codec::BinCodec;
use shared::utils::channel::stream_channel;
use shared::utils::retry::establish_connection;
use futures_util::TryStreamExt;
use tokio::select;
use tokio::sync::OwnedMutexGuard;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::sync::{CancellationToken, PollSender};
use tracing::error;

pub struct SchedulerConnection {
    endpoint: Endpoint,
    pub ready: bool,
    sink: PollSender<SchedulerRequest>,
    stream: ReceiverStream<SchedulerEvent>,
    cancellation_token: CancellationToken
}

impl SchedulerConnection {
    pub fn connect(endpoint: Endpoint, addr: SocketAddr) -> Self {
        let (stream, response_sink, ) = stream_channel(1024);
        let (request_stream, sink, ) = stream_channel(1024);
        let cancellation_token = CancellationToken::new();

        tokio::spawn(handle_scheduler(endpoint.clone(), addr, cancellation_token.clone(), request_stream, response_sink));

        Self {
            ready: false,
            endpoint,
            sink,
            stream,
            cancellation_token,
        }
    }

    pub async fn send(&mut self, request: SchedulerRequest){
        self.sink.send(request).await;
    }

    pub async fn recv(&mut self) -> Option<SchedulerEvent>{
        self.stream.next().await
    }

    pub fn reconnect(&mut self, addr: SocketAddr){
        *self = Self::connect(self.endpoint.clone(), addr);
    }
}

pub async fn handle_scheduler(endpoint: Endpoint, addr: SocketAddr, cancellation_token: CancellationToken, request_stream: ReceiverStream<SchedulerRequest>, mut response_sink: PollSender<SchedulerEvent>) {
    let a: Result<_, _> = try {
        let Ok((send_stream, recv_stream)) = establish_connection(&endpoint, addr, "scheduler").await else {
            response_sink.send(SchedulerEvent::ConnectionNotEstablished).await?;
            return;
        };
        let response_stream = FramedRead::new(recv_stream, BinCodec::<SchedulerResponse>::new());
        let mut request_sink = FramedWrite::new(send_stream, BinCodec::<SchedulerRequest>::new());
        let mut response_sink = response_sink
            .sink_map_err(|x| io::Error::from(ErrorKind::BrokenPipe));
        request_sink.send(SchedulerRequest::HelloMessage).await?;
        response_sink.send(SchedulerEvent::Connected).await?;
        let forward_response = response_stream
            .map_ok(|message| SchedulerEvent::Message(message))
            .forward(&mut response_sink);

        let forward_request = request_stream
            .map(Result::Ok)
            .forward(request_sink);

        select! {
                    _ = forward_request => {
                        response_sink.send(SchedulerEvent::Disconnected).await?;

                        Err(io::Error::from(ErrorKind::ConnectionRefused))?
                    },
                    _ = forward_response => {
                         response_sink.send(SchedulerEvent::Disconnected).await?;

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

#[derive(Debug)]
pub enum  SchedulerEvent{
    Connected,
    Disconnected,
    Message(SchedulerResponse),
    ConnectionNotEstablished,
}