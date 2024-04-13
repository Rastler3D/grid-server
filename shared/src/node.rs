use std::net::SocketAddr;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize,Deserialize,Debug)]
pub enum NodeRequest{
    AssignClient(SocketAddr),
    UnassignClient
}

#[derive(Serialize,Deserialize,Debug)]
pub enum NodeResponse{
    Heartbeat
}
