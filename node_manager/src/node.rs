use std::io;
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;
use std::str::FromStr;
use futures_util::sink;
use quinn::SendStream;
use rustls::Certificate;
use tokio_util::codec::FramedWrite;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use x509_parser::prelude::*;
use x509_parser::prelude::GeneralName::DNSName;
use futures_util::sink::{SinkExt};
use shared::certificate::{PLATFORM_OID, SUBWORKERS_OID};
use shared::node::{NodeRequest, NodeResponse};
use shared::node_manager::NodeManagerResponse;
use shared::platform::Platform;
use shared::utils::bin_codec::BinCodec;
use shared::utils::channel::Return;
use shared::worker::{WorkerRequest, WorkerResponse};
use tokio_util::time::delay_queue::Key;
use x509_parser::oid_registry::Oid;
use crate::node_manager::Id;
use crate::scheduler::SchedulerSink;

pub type NodeSink = FramedWrite<SendStream, BinCodec<NodeRequest>>;

#[derive(Debug)]
pub struct Node{
    pub uuid: Uuid,
    pub name: Option<String>,
    pub subworkers: usize,
    pub platform: Platform,
    pub address: SocketAddr,
    pub expiration_key: Key,
    pub assigned: Option<Id>,
    pub(crate) cancellation_token: CancellationToken,
    pub(crate) sink: NodeSink,
    pub(crate) disconnect_buffer: Option<Vec<NodeRequest>>
}

impl Node{
    pub async fn send(&mut self, response: NodeRequest) -> Result<(), Error> {
        if let Some(ref mut buffer) = self.disconnect_buffer{
            buffer.push(response);
            Ok(())
        } else {
            self.sink.send(response).await
        }
    }
}

impl Drop for Node{
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}

#[derive(Debug)]
pub enum NodeEvent {
    Connect{
        uuid: Uuid,
        name: Option<String>,
        platform: Platform,
        subworkers: usize,
        address: SocketAddr,
        cancellation_token: CancellationToken,
        sink: NodeSink,
        returning: Return<Id>
    },
    Disconnect(Id),
    Message(Id, NodeResponse),
}