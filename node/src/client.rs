use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use futures_sink::Sink;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use quinn::{Connection, Endpoint, RecvStream, SendStream};
use shared::node_manager::NodeManagerRequest;
use shared::utils::bin_codec::BinCodec;
use shared::utils::channel::stream_channel;
use shared::worker::{WorkerRequest, WorkerResponse};
use tokio::{pin, select};
use tokio::task::yield_now;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::sync::{CancellationToken, PollSender};
use tracing::{error, info, instrument};
use crate::node_manager::NodeManager;
use crate::worker::Worker;

pub enum WorkerEvent{
    Connected,
    Disconnected,
    Message(WorkerRequest)
}

pub struct Client{
    pub connected: bool,
    pub buffer: Vec<WorkerResponse>,
    pub addr: SocketAddr,
    pub sink: PollSender<WorkerResponse>,
    pub stream: ReceiverStream<WorkerEvent>,
    pub cancellation_token: CancellationToken,
}

impl Client{
    pub fn new(endpoint: Endpoint, addr: SocketAddr) -> Self{
        let (stream, request_sink,) = stream_channel(1024);
        let (response_stream, sink,) = stream_channel(1024);
        let cancellation_token = CancellationToken::new();
        tokio::spawn(Self::accept_client(endpoint.clone(), addr, cancellation_token.clone(), response_stream, request_sink));

        Self{
            connected: false,
            buffer: Vec::new(),
            addr,
            sink,
            stream,
            cancellation_token
        }
    }
    pub async fn accept_client(endpoint: Endpoint, addr: SocketAddr, cancellation_token: CancellationToken, mut response_stream: ReceiverStream<WorkerResponse>, mut request_sink: PollSender<WorkerEvent>){
        let a: Result<_, _> = try {
            loop {
                let mut waiting_client = Self::wait_client(&endpoint, addr);
                pin!(waiting_client);
                let (send_stream, recv_stream) = select! {
                    client = &mut waiting_client => client?,
                    _ = cancellation_token.cancelled() => break
                };

                let request_stream = FramedRead::new(recv_stream, BinCodec::<WorkerRequest>::new());
                let response_sink = FramedWrite::new(send_stream, BinCodec::<WorkerResponse>::new());
                let mut request_sink = (&mut request_sink)
                    .sink_map_err(|x| io::Error::from(ErrorKind::BrokenPipe));
                request_sink.send(WorkerEvent::Connected).await?;
                let forward_response = request_stream
                    .map_ok(|message| WorkerEvent::Message(message))
                    .forward(&mut request_sink);

                let forward_request = (&mut response_stream)
                    .map(Result::Ok)
                    .forward(response_sink);

                select! {
                    _ = forward_request => {
                        request_sink.send(WorkerEvent::Disconnected).await?;

                        Err(io::Error::from(ErrorKind::ConnectionRefused))?
                    },
                    _ = forward_response => {
                        request_sink.send(WorkerEvent::Disconnected).await?;

                        Err(io::Error::from(ErrorKind::ConnectionRefused))?
                    }
                    _ = cancellation_token.cancelled() => {
                        break;
                    }
                }
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

    #[instrument(skip(endpoint))]
    async fn wait_client(endpoint: &Endpoint, addr: SocketAddr) -> Result<(SendStream,RecvStream), anyhow::Error>{
        loop {
            info!("Waiting for incoming client connection");
            if let Some(connection) = endpoint.accept().await {
                info!(incoming = %connection.remote_address(),"Incoming connection");
                if connection.remote_address() != addr {
                    continue;
                };
                let connection = connection.await?;
                let stream = connection.accept_bi().await?;

                break Ok(stream);
            }
        }
    }

    pub async fn recv(&mut self) -> Option<WorkerEvent>{
        self.stream.next().await
    }

    pub async fn send(&mut self, item: WorkerResponse) -> Result<(), anyhow::Error>{
        if self.connected{
            Ok(self.sink.send(item).await?)
        } else {
            self.buffer.push(item);
            Ok(())
        }

    }

}

impl Drop for Client{
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}