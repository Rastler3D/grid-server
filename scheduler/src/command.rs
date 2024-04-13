use shared::utils::channel::Return;
use crate::http_server::json_job_data::JsonScheduledJob;
use crate::scheduler::{Task, Worker};

pub enum Command{
    ScheduledJob(Return<Option<JsonScheduledJob>>),
    Workers(u64, Return<Vec<Worker>>),
    Tasks(u64, Return<Vec<Task>>)
}