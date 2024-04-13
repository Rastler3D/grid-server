#![feature(iter_next_chunk)]
#![feature(let_chains)]
#![feature(diagnostic_namespace)]
#![feature(try_blocks)]
#![feature(impl_trait_in_assoc_type)]

use clap::Parser;
use tracing::error;
use tracing::level_filters::LevelFilter;
use crate::cli::Config;
use crate::task_manager::TaskManager;

pub mod task_manager;
pub mod job;
pub mod splitter;
pub mod reducer;
pub mod task;
pub mod command;
pub mod job_data;
pub mod scheduler;
pub mod cli;
pub mod endpoint_config;
pub mod http_server;
mod job_extractor;


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
        let mut scheduler = TaskManager::new(config).await?;
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
