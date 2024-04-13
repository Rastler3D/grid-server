use std::fmt::Write;
use std::net::SocketAddr;
use serde::{Deserialize, Deserializer, Serialize};
use serde::de::{DeserializeSeed, SeqAccess, Visitor};
use shared::platform::Platform;
use uuid::Uuid;
use crate::node::{Node};
use crate::scheduler::{Scheduler};

#[derive(Debug,Deserialize,Serialize)]
pub struct Id{
    pub(crate) id: Uuid
}

#[derive(Debug,Deserialize,Serialize)]
pub struct JsonScheduler{
    pub uuid: Uuid,
    pub name: Option<String>,
    pub assigned_workers: Vec<usize>,
}

impl From<&Scheduler> for JsonScheduler{
    fn from(value: &Scheduler) -> Self {
        Self{
            uuid: value.uuid,
            name: value.name.clone(),
            assigned_workers: (&value.assigned_workers).into_iter().collect()
        }
    }
}

#[derive(Debug,Deserialize,Serialize)]
pub struct JsonNode{
    pub uuid: Uuid,
    pub name: Option<String>,
    pub subworkers: usize,
    pub platform: Platform,
    pub address: SocketAddr,
    pub assigned: Option<usize>,
}

impl From<&Node> for JsonNode {
    fn from(value: &Node) -> Self {
        Self{
            uuid: value.uuid,
            name: value.name.clone(),
            platform: value.platform,
            subworkers: value.subworkers,
            address: value.address,
            assigned: value.assigned
        }
    }
}



