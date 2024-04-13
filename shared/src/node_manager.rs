pub mod manager_discovery;

use std::net::SocketAddr;
use serde_derive::{Deserialize, Serialize};
use crate::platform::Platform;
use crate::worker::{WorkerId};

#[derive(Serialize,Deserialize,Debug)]
pub enum NodeManagerRequest {
    HelloMessage,
    WorkersDemand{
        required_workers: Option<u32>,
        supported_platforms: Vec<Platform>,
    },
    ReleaseDemand
}

#[derive(Serialize,Deserialize,Debug)]
pub enum NodeManagerResponse {
    CreatedSession,
    RestoredSession,
    Disconnect(WorkerId),
    Connect(Vec<WorkerInfo>)
}

#[derive(Serialize,Deserialize,Debug, Copy, Clone)]
pub struct WorkerInfo{
    pub worker_id: WorkerId,
    pub subworkers: usize,
    pub address: SocketAddr,
    pub platform: Platform

}
