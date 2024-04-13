use anyhow::anyhow;
use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use shared::task_manager::job_data::JobData;
use shared::utils::channel::return_channel;
use tokio::sync::mpsc::Sender;
use uuid::Uuid;
use crate::command::{Command};
use crate::http_server::anyhow_error::HttpError;
use crate::http_server::json_job_data::{Id, JsonJob, JsonJobData};
use crate::job::Job;
use crate::job_extractor::extract_job;


pub async fn add_job(channel: State<Sender<Command>>, job_data: String) -> Result<Json<Id>, HttpError>{
    let (result, returning) = return_channel();
    let job_data = extract_job(&job_data)?;

    channel.send(Command::AddJob(job_data, returning)).await?;

    Ok(Json(Id{
        id: result.await?
    }))

}

pub async fn cancel_job(channel: State<Sender<Command>>, uuid: Json<Id>) -> Result<Json<Option<JsonJob>>, HttpError>{
    let (result, returning) = return_channel();
    channel.send(Command::CancelJob(uuid.id, returning)).await?;

    Ok(Json(result.await?))
}

pub async fn completed_jobs(channel: State<Sender<Command>>) -> Result<Json<Vec<JsonJob>>, HttpError>{
    let (result, returning) = return_channel();
    channel.send(Command::FinishedJobs(returning)).await?;

    Ok(Json(result.await?))
}

pub async fn queued_jobs(channel: State<Sender<Command>>) -> Result<Json<Vec<JsonJob>>, HttpError>{
    let (result, returning) = return_channel();
    channel.send(Command::QueuedJobs(returning)).await?;

    Ok(Json(result.await?))
}


#[axum_macros::debug_handler]
pub async fn current_job(channel: State<Sender<Command>>) -> Result<Json<Option<JsonJob>>, HttpError>{
    let (result, returning) = return_channel();
    channel.send(Command::CurrentJob(returning)).await?;

    Ok(Json(result.await?))
}