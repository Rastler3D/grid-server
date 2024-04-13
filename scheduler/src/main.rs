#![feature(try_blocks)]
#![feature(let_chains)]

use tracing::level_filters::LevelFilter;
use scheduler::Scheduler;
use cli::Config;
use clap::Parser;
use tracing::error;

pub mod scheduler;
pub mod connection;
pub mod connection_manager;
pub mod node_manager;
pub mod nodes;
pub mod task_manager;
pub mod endpoint_config;
pub mod cli;
mod http_server;
mod command;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let result: Result<(), anyhow::Error> = try {
        let tracing_subscriber = tracing_subscriber::fmt()
            .with_ansi(true)
            .with_level(true)
            .with_target(true)
            .with_max_level(LevelFilter::INFO)
            .init();
        let config = Config::parse();
        let mut scheduler = Scheduler::new(config).await?;
        scheduler.start().await?;
    };
    match result {
        Ok(()) => {}
        Err(err) => {
            error!("Error: {err}. Aborting application.")
        }
    }
    Ok(())
}
