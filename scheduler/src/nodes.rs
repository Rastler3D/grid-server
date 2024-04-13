use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use quinn::Endpoint;
use shared::node_manager::WorkerInfo;
use shared::utils::bin_codec::BinCodec;
use shared::utils::channel::stream_channel;
use shared::utils::retry::establish_connection;
use shared::utils::vec_map::VecMap;
use shared::worker::{WorkerId, WorkerRequest, WorkerResponse};
use tokio::select;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::sync::{CancellationToken, PollSender};
use tracing::error;
use crate::connection::handle_node;

pub struct Node{
    pub(crate) sink: PollSender<WorkerRequest>,
    pub(crate) cancellation_token: CancellationToken
}

impl Drop for Node{
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}


pub struct Nodes{
    endpoint: Endpoint,
    sender: PollSender<WorkerEvent>,
    pub stream: ReceiverStream<WorkerEvent>,
    pub nodes: VecMap<Node>,
}

impl Nodes{
    pub fn new(endpoint: Endpoint) -> Self{
        let (stream, sender) = stream_channel(1024);
        Self{
            nodes: VecMap::new(),
            endpoint,
            sender,
            stream,
        }
    }


    pub async fn send(&mut self, worker_id: WorkerId, request: WorkerRequest) -> bool{
        if let Some(node) = self.nodes.get_mut(worker_id){
            node.sink.send(request).await;

            return true;
        }
        false
    }

    pub async fn recv(&mut self) -> Option<WorkerEvent>{
        self.stream.next().await
    }

    pub fn insert_connected_node(&mut self, worker_id: WorkerId, node: Node){
        self.nodes.insert(worker_id,node);
    }

    pub fn remove_node(&mut self, worker_id: WorkerId){
        self.nodes.remove(worker_id);
    }
    pub fn connect_node(&mut self, worker_info: WorkerInfo){
        let mut response_sink = self.sender.clone();
        let endpoint = self.endpoint.clone();
        tokio::spawn(handle_node(response_sink,endpoint,worker_info));
    }
}

pub enum WorkerEvent{
    Connected(WorkerInfo, Node),
    Disconnected(WorkerId),
    Message(WorkerId, WorkerResponse)
}