use std::net::SocketAddr;
use std::sync::Arc;
use quinn::Endpoint;
use tokio::task::JoinHandle;
use crate::connection::{accept_incoming};
use crate::node_manager::NodeManagerConnection;
use crate::nodes::Nodes;
use crate::task_manager::TaskManagerConnection;

pub struct ConnectionManager{
    endpoint: Endpoint,
    pub task_manager: TaskManagerConnection,
    pub node_manager: NodeManagerConnection,
    pub nodes: Nodes
}

impl ConnectionManager {
    pub fn new(endpoint: Endpoint, node_manager_addr: SocketAddr) -> Result<Self, anyhow::Error> {
        let node_manager = NodeManagerConnection::connect(endpoint.clone(), node_manager_addr);
        let task_manager = TaskManagerConnection::start_accepting(endpoint.clone());
        let nodes = Nodes::new(endpoint.clone());

        Ok(Self{
            endpoint,
            task_manager,
            node_manager,
            nodes
        })
    }

    pub fn connect_node_manager(&mut self, addr: SocketAddr){
        self.node_manager = NodeManagerConnection::connect(self.endpoint.clone(), addr);
    }
}