#![feature(tuple_trait)]
#![feature(unboxed_closures)]
#![feature(box_into_inner)]
#![feature(let_chains)]
#![feature(impl_trait_in_assoc_type)]
#![feature(try_blocks)]

use anyhow::Error;
use clap::Parser;
use tracing::error;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use node_manager::NodeManager;
use crate::cli::Config;

pub mod node_manager;
pub mod server_config;
pub mod node;
pub mod connection;
pub mod scheduler;
pub mod cli;
pub mod http_server;
mod command;

#[tokio::main]
async fn main() -> anyhow::Result<()>  {
    let result: Result<(), anyhow::Error> = try {
        let tracing_subscriber = tracing_subscriber::fmt()
            .with_ansi(true)
            .with_level(true)
            .with_target(true)
            .with_max_level(LevelFilter::INFO)
            .init();
        let config = Config::parse();
        let mut node_manager = NodeManager::new(config).await?;
        node_manager.start().await;
    };

    match result {
        Ok(()) => {}
        Err(err) => {
            error!("Error: {err}. Aborting application.")
        }
    }

    Ok(())
}
