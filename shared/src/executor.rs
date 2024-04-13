use std::time::Duration;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::worker::TaskId;

#[derive(Serialize,Deserialize,Debug)]
pub struct ExecutorJob {
    pub job_id: Uuid,
    pub job_data: Vec<u8>,
    pub task_id: TaskId,
    pub task_data: Vec<u8>

}

#[derive(Serialize,Deserialize,Debug)]
pub struct ExecutorResult {
    pub job_id: Uuid,
    pub task_id: TaskId,
    pub execution_time: Duration,
    pub result: Result<Vec<u8>, String>
}