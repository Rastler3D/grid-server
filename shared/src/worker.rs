use std::time::Duration;
use serde::{Deserialize, Serialize};
use uuid::Uuid;


pub type WorkerId = usize;
pub type TaskId = usize;
#[derive(Serialize,Deserialize,Debug)]
pub enum WorkerResponse {
    Error(WorkerError),
    Result(WorkerResult),
}

#[derive(Serialize,Deserialize,Debug)]
pub enum WorkerError{
    TaskExecutionError{
        task_id: TaskId,
        error: String
    },
    LibraryMissing{
        tasks: Vec<TaskId>,
        replacements: Vec<(TaskId,TaskId)>
    },
    NoFreeExecutors{
        tasks: Vec<TaskId>,
    },
}


#[derive(Serialize,Deserialize,Debug)]
pub struct WorkerRequest{
    pub job: Uuid,
    pub job_data: Option<(Vec<u8>, Vec<u8>)>,
    pub replacements: Vec<(TaskId,WorkerTask)>,
    pub tasks: Vec<WorkerTask>,
}

#[derive(Serialize,Deserialize,Debug,Eq,PartialEq,Clone)]
pub struct WorkerTask{
    pub id: TaskId,
    pub data: Vec<u8>
}
#[derive(Serialize,Deserialize,Debug, Clone)]
pub struct WorkerResult{
    pub id: TaskId,
    pub execution_time: Duration,
    pub data: Vec<u8>
}