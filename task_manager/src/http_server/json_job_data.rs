use std::fmt;
use std::fmt::Write;
use serde::{Deserialize, Deserializer, Serialize};
use serde::de::{DeserializeSeed, SeqAccess, Visitor};
use serde_json::Serializer;
use shared::task_manager::job_data::JobData;
use uuid::Uuid;
use crate::job::{Job, JobPhase, JobResult, JobState};

#[derive(Debug,Deserialize,Serialize)]
pub struct Id{
    pub(crate) id: Uuid
}

#[derive(Debug,Deserialize,Serialize)]
pub struct JsonJob{
    id: Uuid,
    data: JobData,
    state: JsonJobState
}

#[derive(Debug,Deserialize,Serialize)]
pub enum JsonJobState{
    NotStarted,
    Started{
        incomplete_tasks: usize,
        phase: JsonJobPhase
    },
    Completed{
        result: JobResult
    }
}

#[derive(Debug,Deserialize,Serialize)]
pub enum JsonJobPhase{
    Splitting,
    WaitingTasksCompletion,
    Reducing
}

impl From<&Job> for JsonJob{
    fn from(job: &Job) -> Self {
        Self{
            id: job.id,
            data: job.data.clone(),
            state: match &job.state {
                JobState::NotStarted => {
                    JsonJobState::NotStarted
                }
                JobState::Started { phase, tasks, .. } => {
                    JsonJobState::Started{
                        incomplete_tasks: tasks.len(),
                        phase: match phase {
                            JobPhase::Splitting { .. } => JsonJobPhase::Splitting,
                            JobPhase::WaitingTasksCompletions => JsonJobPhase::WaitingTasksCompletion,
                            JobPhase::Reducing { .. } => JsonJobPhase::Reducing
                        }
                    }
                }
                JobState::Finished(result) => {
                    JsonJobState::Completed{
                        result: result.clone()
                    }
                }
            }
        }
    }
}

#[derive(Serialize,Deserialize,Debug,Clone)]
#[non_exhaustive]
pub struct JsonJobData{
    #[serde(deserialize_with="null_to_max")]
    graph: Vec<Vec<u32>>,
    taxies:Vec<usize>,
    passengers: Vec<(usize,usize)>
}

impl Into<JobData> for JsonJobData{
    fn into(self) -> JobData {
        JobData{
            graph: self.graph,
            taxies: self.taxies,
            passengers: self.passengers
        }
    }
}


fn null_to_max<'de, D>(deserializer: D) -> Result<Vec<Vec<u32>>, D::Error>
    where
        D: Deserializer<'de>,
{
    struct SeqDeserialize;
    impl<'de> DeserializeSeed<'de> for SeqDeserialize{
        type Value = Vec<u32>;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error> where D: Deserializer<'de> {
            deserializer.deserialize_seq(SeqVisitor)
        }
    }
    struct SeqVisitor;

    impl<'de> Visitor<'de> for SeqVisitor
    {
        type Value = Vec<u32>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a sequence")
        }

        fn visit_seq<S>(self, mut seq: S) -> Result<Vec<u32>, S::Error>
            where
                S: SeqAccess<'de>,
        {
            let mut vec = if let Some(capacity) = seq.size_hint(){
                Vec::with_capacity(capacity)
            } else { Vec::new() };

            while let Some(value) = seq.next_element::<Option<u32>>()? {
                vec.push(value.unwrap_or(u32::MAX))
            }

            Ok(vec)
        }
    }

    struct SeqSeqVisitor;

    impl<'de> Visitor<'de> for SeqSeqVisitor
    {
        type Value = Vec<Vec<u32>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a sequence")
        }

        fn visit_seq<S>(self, mut seq: S) -> Result<Vec<Vec<u32>>, S::Error>
            where
                S: SeqAccess<'de>,
        {
            let mut vec = if let Some(capacity) = seq.size_hint(){
                Vec::with_capacity(capacity)
            } else { Vec::new() };

            while let Some(value) = seq.next_element_seed(SeqDeserialize)? {
                vec.push(value)
            }

            Ok(vec)
        }
    }
    deserializer.deserialize_seq(SeqSeqVisitor)
}