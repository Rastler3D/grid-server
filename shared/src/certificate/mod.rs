pub mod verifier;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SUBWORKERS_OID: &[u64] = &[2,5,29,100];
pub const PLATFORM_OID: &[u64] = &[2,5,29,101];
pub const SERVICE_TYPE_OID: &[u64] = &[2,5,29,102];

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error(transparent)]
    RcGen(#[from] rcgen::Error),
    #[error(transparent)]
    Rustls(#[from] rustls::Error),
}


#[derive(Serialize,Deserialize, Debug)]
pub enum ServiceType{
    Node,
    NodeManager,
    Scheduler,
    TaskManager
}