use std::io::Error;
use std::net::SocketAddr;
use futures_util::{SinkExt};
use quinn::SendStream;
use serde::{Deserialize, Serialize};
use shared::node_manager::{NodeManagerRequest, NodeManagerResponse};
use shared::platform::Platform;
use shared::utils::bin_codec::BinCodec;
use shared::utils::bit_set::BitSet;
use shared::utils::linked_vec_set::LinkedVecSet;
use shared::utils::channel::Return;
use tokio_util::codec::FramedWrite;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use crate::node_manager::Id;

pub type Set = BitSet<Vec<u64>>;
pub type SchedulerSink = FramedWrite<SendStream, BinCodec<NodeManagerResponse>>;

#[derive(Debug)]
pub struct Scheduler{
    pub uuid: Uuid,
    pub name: Option<String>,
    pub assigned_workers: LinkedVecSet,
    pub address: SocketAddr,
    pub(crate) cancellation_token: CancellationToken,
    pub(crate) sink: SchedulerSink,
    pub(crate) disconnect_buffer: Option<Vec<NodeManagerResponse>>
}

impl Scheduler{
    pub async fn send(&mut self, response: NodeManagerResponse) -> Result<(), Error> {
        if let Some(ref mut buffer) = self.disconnect_buffer{
            buffer.push(response);
            Ok(())
        } else {
            self.sink.send(response).await
        }
    }
}

#[derive(Clone, Deserialize,Serialize, Debug)]
pub struct WorkersDemand{
    pub provided_workers: u32,
    pub required_workers: Option<u32>,
    pub supported_platforms: Vec<Platform>
}

#[derive(Debug)]
pub enum SchedulerEvent {
    Connect{
        uuid: Uuid,
        name: Option<String>,
        address: SocketAddr,
        cancellation_token: CancellationToken,
        sink: SchedulerSink,
        returning: Return<Id>
    },
    Disconnect(Id),
    Message(Id, NodeManagerRequest),
}

impl Drop for Scheduler{
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}