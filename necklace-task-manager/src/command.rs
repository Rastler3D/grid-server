use shared::task_manager::job_data::JobData;
use shared::utils::channel::Return;
use uuid::Uuid;
use crate::http_server::json_job_data::JsonJob;
use crate::job::Job;

pub enum Command{
    AddJob(Vec<u8>, Return<Uuid>),
    CancelJob(Uuid, Return<Option<JsonJob>>),
    FinishedJobs(Return<Vec<JsonJob>>),
    QueuedJobs(Return<Vec<JsonJob>>),
    CurrentJob(Return<Option<JsonJob>>)

}
