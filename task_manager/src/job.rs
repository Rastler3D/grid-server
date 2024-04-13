use std::fs::File;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use shared::scheduler::JobStats;
use uuid::Uuid;
use shared::task_manager::job_data::JobData;
use shared::task_manager::task::Distance;
use shared::utils::multilevel_vec_map::MultilevelVecMap;
use crate::reducer::TaskReducer;
use crate::splitter::TaskSplitter;
use crate::task::{TaskSet};

pub struct Job {
    pub(crate) id: Uuid,
    pub(crate) data: JobData,
    pub(crate) state: JobState,
}

pub enum JobState {
    NotStarted,
    Started{
        stats: Option<JobStats>,
        parts: Vec<MultilevelVecMap<Distance>>,
        tasks: TaskSet,
        phase: JobPhase
    },
    Finished(JobResult),
}

#[derive(Debug,Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
#[serde(rename_all = "camelCase")]
pub enum JobResult {
    Completed{
        distance: Distance,
        stats: Option<JobStats>
    },
    Error{
        error_message: String,
        stats: Option<JobStats>
    }
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct CompletedJob{
    result: Distance,
    stats: Option<JobStats>
}

pub enum JobPhase{
    Splitting{
        splitter: TaskSplitter,
    },
    WaitingTasksCompletions,
    Reducing{
        reducer: TaskReducer,
        minimal: Distance
    }
}
