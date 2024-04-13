#![feature(try_blocks)]
#![feature(hash_extract_if)]

use crate::wasm::wasm_worker::WasmWorker;
use crate::dll::dll_worker::DllWorker;
use std::net::{SocketAddr, ToSocketAddrs};
use clap::{Parser, ValueEnum};
use shared::node_manager::manager_discovery::NodeManagerDiscoveryService;
use tracing::{debug, error, info};
use tracing::metadata::LevelFilter;
use wasmtime::component::__internal::anyhow;
use crate::node::Node;
use cli::*;

pub mod endpoint_config;
pub mod cli;
pub mod node_manager;
pub mod node;
pub mod worker;
pub mod client;
pub mod dll;
pub mod wasm;


#[tokio::main]
async fn main() -> anyhow::Result<()>{
    let result: Result<(), anyhow::Error> = try {
        let tracing_subscriber = tracing_subscriber::fmt()
        .with_ansi(true)
        .with_level(true)
        .with_target(true)
        .with_max_level(LevelFilter::INFO)
        .init();
        let config = Config::parse();

        match config.library_type {
            LibraryType::Wasm => {
                let mut node = Node::new_wasm(config).await?;
                node.start().await?;
            },
            LibraryType::Dll => {
                let mut node = Node::new_dll(config).await?;
                node.start().await?;
            }
        }
    };

    match result {
        Ok(()) => {}
        Err(err) => {
            error!("Error: {err}. Aborting application.")
        }
    }

    Ok(())
}
