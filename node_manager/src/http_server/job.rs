use axum::extract::State;
use axum::Json;
use shared::utils::channel::return_channel;
use tokio::sync::mpsc::Sender;
use crate::command::{Command};
use crate::http_server::anyhow_error::HttpError;
use crate::http_server::json_job_data::{Id, JsonNode,  JsonScheduler};
use crate::scheduler::WorkersDemand;


pub async fn nodes(channel: State<Sender<Command>>) -> Result<Json<Vec<JsonNode>>, HttpError>{
    let (result, returning) = return_channel();
    channel.send(Command::Nodes(returning)).await?;

    Ok(Json(result.await?))
}

pub async fn schedulers(channel: State<Sender<Command>>) -> Result<Json<Vec<JsonScheduler>>, HttpError>{
    let (result, returning) = return_channel();
    channel.send(Command::Schedulers(returning)).await?;

    Ok(Json(result.await?))
}

pub async fn queue(channel: State<Sender<Command>>) -> Result<Json<Vec<WorkersDemand>>, HttpError>{
    let (result, returning) = return_channel();
    channel.send(Command::Queue(returning)).await?;

    Ok(Json(result.await?))
}