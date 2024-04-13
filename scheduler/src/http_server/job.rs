
use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use shared::utils::channel::return_channel;
use tokio::sync::mpsc::Sender;
use crate::command::{Command};
use crate::http_server::anyhow_error::HttpError;
use crate::http_server::json_job_data::{Id, JsonScheduledJob};
use crate::scheduler::{Task, Worker};

#[derive(Deserialize)]
pub struct Limit{
    limit: u64
}

pub async fn current_job(channel: State<Sender<Command>>) -> Result<Json<Option<JsonScheduledJob>>, HttpError>{
    let (result, returning) = return_channel();
    channel.send(Command::ScheduledJob(returning)).await?;

    Ok(Json(result.await?))
}

pub async fn tasks(channel: State<Sender<Command>>, query: Query<Limit>) -> Result<Json<Vec<Task>>, HttpError>{
    let (result, returning) = return_channel();
    channel.send(Command::Tasks(query.limit, returning)).await?;

    Ok(Json(result.await?))
}


pub async fn workers(channel: State<Sender<Command>>, query: Query<Limit>) -> Result<Json<Vec<Worker>>, HttpError>{
    let (result, returning) = return_channel();
    channel.send(Command::Workers(query.limit, returning)).await?;

    Ok(Json(result.await?))
}