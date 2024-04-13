use shared::utils::channel::Return;
use crate::http_server::json_job_data::{JsonNode, JsonScheduler};
use crate::scheduler::WorkersDemand;

pub enum Command{
    Nodes(Return<Vec<JsonNode>>),
    Schedulers(Return<Vec<JsonScheduler>>),
    Queue(Return<Vec<WorkersDemand>>)
}