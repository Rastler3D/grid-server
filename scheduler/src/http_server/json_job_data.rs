
use std::fmt::Write;
use std::time::Duration;
use serde::{Deserialize, Deserializer, Serialize};
use serde::de::{DeserializeSeed, SeqAccess, Visitor};
use shared::platform::Platform;
use uuid::Uuid;
use shared::scheduler::WorkerJobData;


use crate::scheduler::ScheduledJob;

#[derive(Debug,Deserialize,Serialize)]
pub struct Id{
    pub(crate) id: Uuid
}

#[derive(Debug,Deserialize,Serialize)]
pub struct JsonScheduledJob{
    failed_tasks: u128,
    avg_task_completion_time: String,
    received_tasks: u128,
    total_tasks: u128,
    scheduled_tasks: u128,
    completed_tasks: u128,
    job_platforms: Vec<Platform>,
    id: Uuid,
}

impl From<&ScheduledJob> for JsonScheduledJob{
    fn from(value: &ScheduledJob) -> Self {
        Self{
            id: value.id,
            scheduled_tasks: value.scheduled_tasks,
            completed_tasks: value.completed_tasks,
            failed_tasks: value.failed_tasks,
            received_tasks: value.received_tasks,
            total_tasks: value.total_tasks,
            job_platforms: value.job.data.keys().cloned().collect(),
            avg_task_completion_time: humantime::format_duration(value.avg_task_completion_time).to_string(),
        }
    }
}




