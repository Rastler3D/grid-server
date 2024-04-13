use tracing::{debug, error, event, info, instrument, Instrument, Level};
use std::io;
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;
use std::pin::Pin;

use std::time::Duration;
use async_stream::stream;
use futures_sink::Sink;
use quinn::{Endpoint, RecvStream, SendStream};
use shared::node::{NodeRequest, NodeResponse};
use shared::node_manager::manager_discovery::NodeManagerDiscoveryService;
use shared::utils::bin_codec::BinCodec;
use shared::worker::{WorkerRequest, WorkerResponse};
use tokio::{pin, select};
use tokio::time::{interval, sleep};
use tokio_stream::{Stream};
use tokio_stream::wrappers::{IntervalStream, ReceiverStream};
use tokio_util::codec::{FramedRead, FramedWrite};
use anyhow::anyhow;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use shared::utils::retry::establish_connection;
use tokio_util::sync::{CancellationToken, PollSender};
use shared::utils::channel::stream_channel;

#[derive(Debug)]
pub enum NodeManagerEvent{
    Connected,
    Disconnected,
    Message(NodeRequest),
    ConnectionNotEstablished,
}

pub struct NodeManager{
    stream: ReceiverStream<NodeManagerEvent>,
    sink: PollSender<NodeResponse>,
    pub cancellation_token: CancellationToken,
}

impl NodeManager{

    #[instrument(skip_all)]
    pub fn connect(endpoint: Endpoint, addr: Option<SocketAddr>) -> Self{
        let (stream, request_sink,) = stream_channel(1024);
        let (response_stream, sink,) = stream_channel(1024);
        let cancellation_token = CancellationToken::new();

        tokio::spawn(Self::connect_node_manager(endpoint,addr, cancellation_token.clone(), response_stream, request_sink));

        Self{
            sink,
            stream,
            cancellation_token
        }

    }

    #[instrument(skip_all)]
    pub async fn connect_discovery(endpoint: &Endpoint) -> Result<(SendStream,RecvStream), anyhow::Error>{
        info!("Discovering node manager");
        loop {
            let discovery = NodeManagerDiscoveryService::discover();
            pin!(discovery);

            while let Some(addr) = discovery.next().await {
                let addr = addr?;
                info!(%addr, "Node manager discovered. Connecting...");
                let connection: Result<_,anyhow::Error> = try {
                    endpoint.connect(addr, "addr")?
                        .await?
                        .open_bi()
                        .await?
                };

                match connection {
                    Ok(connection) => return Ok(connection),
                    Err(err) => {
                        error!(%err, "Failed to connect to node manager");
                        info!("Discovering another node manager");
                    }
                }
            }

            sleep(Duration::from_millis(100)).await;
        }
    }

    #[instrument(skip_all, fields(%addr))]
    pub async fn connect_to_address(endpoint: &Endpoint, addr: SocketAddr) -> Result<(SendStream, RecvStream), anyhow::Error>{
        info!("Connecting to node manager");
        establish_connection(&endpoint, addr, "node manager").await
    }

    pub async fn send(&mut self, item: NodeResponse) -> Result<(), anyhow::Error>{
        Ok(self.sink.send(item).await?)
    }

    pub async fn recv(&mut self) -> Option<NodeManagerEvent>{
        self.stream.next().await
    }

    pub async fn connect_node_manager(endpoint: Endpoint, addr: Option<SocketAddr>, cancellation_token: CancellationToken, response_stream: ReceiverStream<NodeResponse>, mut request_sink: PollSender<NodeManagerEvent>){
        let a: Result<_, _> = try {
            let Ok((send_stream, recv_stream)) = (if let Some(addr) = addr {
                Self::connect_to_address(&endpoint, addr).await
            } else {
                Self::connect_discovery(&endpoint).await
            }) else {
                request_sink.send(NodeManagerEvent::ConnectionNotEstablished).await?;
                return;
            };

            let request_stream = FramedRead::new(recv_stream, BinCodec::<NodeRequest>::new());
            let response_sink = FramedWrite::new(send_stream, BinCodec::<NodeResponse>::new());
            let mut request_sink = request_sink
                .sink_map_err(|x| io::Error::from(ErrorKind::BrokenPipe));

            request_sink.send(NodeManagerEvent::Connected).await?;
            let forward_response = request_stream
                .map_ok(|message| NodeManagerEvent::Message(message))
                .forward(&mut request_sink);

            let forward_request = response_stream
                .map(Result::Ok)
                .forward(response_sink);

            select! {
                    _ = forward_request => {
                        request_sink.send(NodeManagerEvent::Disconnected).await?;

                        Err(io::Error::from(ErrorKind::ConnectionRefused))?
                    },
                    _ = forward_response => {
                         request_sink.send(NodeManagerEvent::Disconnected).await?;


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
}

impl Drop for NodeManager{
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}