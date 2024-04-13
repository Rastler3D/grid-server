use std::collections::HashMap;
use std::time::Duration;
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;
use crate::platform::Platform;
use crate::worker::{WorkerTask, WorkerResult};

#[derive(Serialize,Deserialize,Debug)]
pub enum SchedulerRequest{
    HelloMessage,
    NewJob{
        id: Uuid,
        total_tasks: u128,
        job: WorkerJobData,
    },
    Task(Vec<WorkerTask>)
}

#[derive(Serialize,Deserialize,Debug)]
pub enum SchedulerResponse{
    TaskResults(Vec<WorkerResult>),
    SchedulerBusy,
    SchedulerFree,
    TaskExecutionError(String, JobStats),
    JobComplete(Vec<WorkerResult>, JobStats)
}

#[derive(Serialize,Deserialize,Debug, Clone, Copy)]
pub struct JobStats{
    pub total_tasks: u128,
    pub completed_tasks: u128,
    pub failed_tasks: u128,
    pub avg_task_completion_time: Duration
}



#[derive(Serialize,Deserialize,Debug)]
pub struct WorkerJobData {
    pub args: Vec<u8>,
    pub data: HashMap<Platform, Vec<u8>>
}