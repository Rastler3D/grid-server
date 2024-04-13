use std::io;
use std::net::SocketAddr;
use futures_util::{SinkExt, StreamExt};
use quinn::{Endpoint, RecvStream, SendStream};
use shared::utils::bin_codec::BinCodec;
use shared::utils::channel::stream_channel;
use shared::utils::retry::establish_connection;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::sync::{CancellationToken, PollSender};
use crate::connection::handle_node_manager;
use crate::nodes::WorkerEvent;
use shared::node_manager::{NodeManagerRequest, NodeManagerResponse};

#[derive(Debug)]
pub enum NodeManagerEvent{
    Connected,
    ConnectionNotEstablished,
    Disconnected,
    Message(NodeManagerResponse)
}


pub struct NodeManagerConnection{
    pub connected: bool,
    stream: ReceiverStream<NodeManagerEvent>,
    sink: PollSender<NodeManagerRequest>,
    cancellation_token: CancellationToken
}

impl NodeManagerConnection {

    pub fn connect(endpoint: Endpoint, addr: SocketAddr) -> Self{
        let (stream, response_sink,) = stream_channel(1024);
        let (request_stream, sink,) = stream_channel(1024);
        let cancellation_token = CancellationToken::new();

        tokio::spawn(handle_node_manager(endpoint, addr, cancellation_token.clone(), request_stream, response_sink));

        Self{
            connected: false,
            sink,
            stream,
            cancellation_token,
        }
    }
    pub async fn send(&mut self, request: NodeManagerRequest){
        self.sink.send(request).await;
    }

    pub async fn recv(&mut self) -> Option<NodeManagerEvent>{
        self.stream.next().await
    }
}