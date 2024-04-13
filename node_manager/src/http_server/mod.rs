pub mod json_job_data;
pub mod job;
pub mod anyhow_error;


use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use axum::Router;
use axum::routing::{get};
use tokio::sync::mpsc::Sender;
use crate::command::Command;
use crate::http_server::job::{nodes, queue, schedulers};


pub async fn start_server(bind_addr: Option<SocketAddr>, sender: Sender<Command>) -> Result<(), anyhow::Error>{
    let bind_addr = bind_addr.unwrap_or(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8080));
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    let router = Router::new()
        .route("/node", get(nodes))
        .route("/scheduler", get(schedulers))
        .route("/queue", get(queue))
        .with_state(sender);

    tokio::spawn(async move{ axum::serve(listener, router).await});

    Ok(())

}

