use std::cmp::min;
use std::collections::VecDeque;
use std::mem::take;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::ControlFlow;
use std::ops::ControlFlow::{Break, Continue};
use anyhow::{anyhow, Error};
use awaitable_bool::AwaitableBool;
use bincode::config;
use futures_util::{SinkExt, StreamExt};
use pyo3::Python;
use quinn::Endpoint;
use shared::scheduler::{JobStats, SchedulerRequest, SchedulerResponse, WorkerJobData};
use shared::task_manager::task::{Distance, split, TaskResult};
use shared::utils::channel::DuplexChannel;
use shared::utils::multilevel_vec_map::MultilevelVecMap;
use shared::worker::{WorkerResult, WorkerTask};
use tokio::select;
use tokio::sync::mpsc::{channel, Receiver};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info, instrument};
use uuid::Uuid;

use crate::cli::Config;
use crate::endpoint_config::endpoint_config::EndpointConfigBuilder;
use crate::job::{Job, JobPhase, JobResult, JobState};
use crate::job_data::JOB_DATA;
use crate::command::{Command};
use crate::http_server::start_server;
use crate::job_extractor::extract_job;
use crate::reducer::{reduce};
use crate::scheduler::{SchedulerConnection, SchedulerEvent};
use crate::splitter::TaskSplitter;
use crate::task::TaskSet;

type Queue<T> = VecDeque<T>;
pub struct TaskManager {
    commands: ReceiverStream<Command>,
    config: Config,
    scheduler: SchedulerConnection,
    jobs_finished: Vec<Job>,
    jobs_queue: Queue<Job>,
    current_job: Option<Job>,
    need_process_job: AwaitableBool,
}

impl TaskManager{

    pub async fn new(config: Config, ) -> Result<Self, anyhow::Error>{
        let (sender,receiver) = channel(10);
        let _ = start_server(config.bind_api_server_address,sender).await?;
        let client_config = EndpointConfigBuilder::new().build()?;
        let mut endpoint = Endpoint::client(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0))?;
        endpoint.set_default_client_config(client_config);

        Ok(Self{
            jobs_queue: VecDeque::new(),
            jobs_finished: Vec::new(),
            scheduler: SchedulerConnection::connect(endpoint, config.scheduler_address),
            commands: ReceiverStream::new(receiver),
            current_job: None,
            need_process_job: AwaitableBool::new(false),
            config,
        })
    }

    #[instrument(skip_all)]
    pub async fn start(&mut self) -> Result<(), anyhow::Error>{
        loop {
            let result: ControlFlow<Result<JobResult, anyhow::Error>> = try {
                select! {
                    Some(response) = self.scheduler.recv() => {
                        self.process_scheduler(response).await?
                    },
                    Some(command) = self.commands.next() => {
                        self.process_command(command).await?
                    }
                    _ = self.need_process_job.wait_true() => {
                        self.process_job().await?
                    }
                }
            };

            match result {
                Continue(()) => continue,
                Break(Err(err)) => {
                    return Err(err);
                },
                Break(Ok(job_result)) => {
                    match &job_result {
                        JobResult::Completed{ result, ..} => {
                            info!("Job completed. Result: {result}");
                        }
                        JobResult::Error{ error_message, ..} => {
                            info!("Job failed. Error: {error_message}")
                        }
                    }
                    if let Some(mut job) = self.current_job.take() {
                        job.state = JobState::Finished(job_result);

                        self.jobs_finished.push(job);
                    }
                }
            }
        }
    }

    #[instrument(skip_all)]
    async fn process_job(&mut self) -> ControlFlow<Result<JobResult, anyhow::Error>>{
        if let Some(ref mut job) = self.current_job{
            match &mut job.state {
                JobState::NotStarted => {
                    if !self.scheduler.ready{
                        self.need_process_job.set_false();
                        return Continue(());
                    }
                    info!(%job.id, "Starting job");
                    let job_data = &job.data;

                    let task_splitter = match TaskSplitter::new(job_data.clone())  {
                        Ok(task_splitter) => task_splitter,
                        Err(err) => return Break(Ok(JobResult::Error{ error_message: format!("Failed to create task splitter: {:?}", err.to_string()), stats: None }))
                    };

                    let data = JOB_DATA.iter().map(|&(platform,code)| (platform,code.to_vec())).collect();
                    let worker_job_data = WorkerJobData { args: job_data.clone(), data };
                    let total_tasks =task_splitter.total_tasks();
                    self.scheduler.send(SchedulerRequest::NewJob {
                        total_tasks,
                        id: job.id.clone(),
                        job: worker_job_data,
                    }).await;

                    job.state = JobState::Started {
                        stats: None,
                        tasks: TaskSet::with_capacity((total_tasks % 64 + 1) as usize ),
                        results: Vec::with_capacity(total_tasks as usize),
                        phase: JobPhase::Splitting {
                            splitter: task_splitter,
                        }
                    };
                    info!(%job.id, %total_tasks ,"Job transited to phase 'Splitting'");

                }
                JobState::Started { tasks, results, phase, stats } => {
                    match phase {
                        JobPhase::Splitting { ref mut splitter } => {
                            if !self.scheduler.ready{
                                self.need_process_job.set_false();
                                return Continue(());
                            }

                            match Python::with_gil(|py| splitter.iter(py).next_chunk::<128>()) {
                                Ok(tasks_batch) => {
                                    let mut workers_tasks = Vec::with_capacity(1024);
                                    for task in tasks_batch {
                                        match task{
                                            Ok((id,data)) => {
                                                tasks.insert(id);
                                                workers_tasks.push(WorkerTask{ id,data });
                                            },
                                            Err(err) => {
                                                return Break(Ok(JobResult::Error{ error_message: err.to_string(), stats: None }))
                                            }
                                        }
                                    }
                                    self.scheduler.send(SchedulerRequest::Task(workers_tasks)).await;
                                }
                                Err(left) => {
                                    let mut workers_tasks = Vec::with_capacity(1024);
                                    for task in left {
                                        match task{
                                            Ok((id,data)) => {
                                                tasks.insert(id);
                                                workers_tasks.push(WorkerTask{ id,data });
                                            },
                                            Err(err) => {
                                                return Break(Ok(JobResult::Error{ error_message: err.to_string(), stats: None }))
                                            }
                                        }
                                    }
                                    self.scheduler.send(SchedulerRequest::Task(workers_tasks)).await;

                                    *phase = JobPhase::WaitingTasksCompletions;
                                    info!(%job.id, waiting_tasks = %tasks.len() ,"Job transited to phase 'WaitingTasksCompletions'");
                                }
                            }
                        }
                        JobPhase::WaitingTasksCompletions => {
                            if !self.scheduler.ready{
                                self.need_process_job.set_false();
                                return Continue(());
                            }
                            if tasks.is_empty() {


                                *phase = JobPhase::Reducing;
                                info!(%job.id, "Job transited to phase 'Reducing'");
                            }
                        }
                        JobPhase::Reducing => {
                            let results = results.iter_mut().map(|x| take(&mut x.data)).collect();
                            let output = match reduce(job.data.clone(), results){
                                Ok(output) => output,
                                Err(err) => {
                                    return Break(Ok(JobResult::Error{ error_message: format!("Failed to create reduce tasks: {:?}", err.to_string()), stats: None }))
                                }
                            };
                            info!(%job.id, "Job transition to state 'Completed'");
                            return Break(Ok(JobResult::Completed{ result: output, stats: stats.take() } ))
                        }
                    }
                }
                JobState::Finished(_) => {

                }
            }
        } else {
            match self.jobs_queue.pop_front(){
                None => self.need_process_job.set_false(),
                Some(job) => {
                    self.current_job = Some(job);
                    self.need_process_job.set_true();
                }
            }
        }

        Continue(())
    }

    #[instrument(skip_all)]
    pub async fn process_scheduler(&mut self, response: SchedulerEvent) -> ControlFlow<Result<JobResult, anyhow::Error>>{
        match response {
            SchedulerEvent::Message(SchedulerResponse::JobComplete(mut left, job_stats)) => {
                info!(results = %left.len(), "Scheduler sent task results");
                info!(?job_stats, "Scheduler complete job");
                if let Some(current_job) = &mut self.current_job{
                    if let JobState::Started { results, tasks , stats, ..  } = &mut current_job.state{
                        *stats = Some(job_stats);
                        for result in left{
                            if tasks.remove(result.id){
                                results.push(result);
                            }
                        }
                    }
                }
            },
            SchedulerEvent::Connected => {
                info!("Scheduler connected");
            },
            SchedulerEvent::Disconnected => {
                info!("Scheduler disconnected");
                self.scheduler.ready = false;
                self.scheduler.reconnect(self.config.scheduler_address);
                if let Some(current_job) = &mut self.current_job{
                    if let JobState::Started { phase, .. } = &mut current_job.state{
                        match phase {
                            JobPhase::Splitting { .. } | JobPhase::WaitingTasksCompletions => {
                                current_job.state = JobState::NotStarted;
                            },
                            _ => {}
                        }
                    }
                }
            },
            SchedulerEvent::Message(SchedulerResponse::TaskResults(mut task_results)) =>{
                info!(results = %task_results.len(), "Scheduler sent task results");
                if let Some(job) = &mut self.current_job{
                    if let JobState::Started{ results, tasks , stats, .. } = &mut job.state{
                       for result in task_results{
                           if tasks.remove(result.id){
                                results.push(result);
                           }
                       }
                    }
                }
            },
            SchedulerEvent::Message(SchedulerResponse::SchedulerBusy) => {
                info!("Scheduler busy");
                self.scheduler.ready = false;
                if let Some(current_job) = &mut self.current_job{
                    if let JobState::Started { phase, .. } = &mut current_job.state{
                        match phase {
                            JobPhase::Splitting { .. } | JobPhase::WaitingTasksCompletions => {
                                current_job.state = JobState::NotStarted;
                            },
                            _ => {}
                        }
                    }
                }
            },
            SchedulerEvent::Message(SchedulerResponse::SchedulerFree) => {
                info!("Scheduler become free");
                self.scheduler.ready = true;
                self.need_process_job.set_true();
            },
            SchedulerEvent::Message(SchedulerResponse::TaskExecutionError(error, stats)) => {
                info!(?stats, %error, "Scheduler returned error");
                return Break(Ok(JobResult::Error{ error_message: error, stats: Some(stats) }))
            }
            SchedulerEvent::ConnectionNotEstablished => {
                return Break(Err(anyhow!("Failed to establish connection with scheduler")))
            }
        }

        Continue(())
    }

    #[instrument(skip_all)]
    pub async fn process_command(&mut self, command: Command) -> ControlFlow<Result<JobResult, anyhow::Error>>{
        match command {
            Command::AddJob(job_data, returning) => {
                info!(?job_data, "Received new job");
                let job = Job{
                    id: Uuid::new_v4(),
                    data: job_data,
                    state: JobState::NotStarted
                };
                let id = job.id;
                self.jobs_queue.push_back(job);
                self.need_process_job.set_true();

                returning(id)
            },
            Command::CancelJob(uuid, returning) => {
                info!(%uuid, "Remove job request");
                if let Some(job) = &self.current_job && job.id == uuid{
                    returning(Some(job.into()));
                    let stats = if let JobState::Started {stats, ..} = job.state{
                        stats
                    } else { None };
                    return Break(Ok(JobResult::Error{ error_message: "Job has been cancelled".to_string(), stats: stats }))
                } else {
                    let pos = self.jobs_queue.iter().position(|x| x.id == uuid);
                    if let Some(pos) = pos{
                        returning(self.jobs_queue.remove(pos).map(|ref x| x.into()))
                    } else {
                        returning(None)
                    }
                }

            },
            Command::FinishedJobs(returning) => {
                let jobs = self.jobs_finished.iter().map(|x| x.into()).collect();

                returning(jobs)
            }
            Command::QueuedJobs(returning) => {
                let jobs = self.jobs_finished.iter().map(|x| x.into()).collect();

                returning(jobs)
            }
            Command::CurrentJob(returning) => {
                let job = self.current_job.as_ref().map(|x| x.into());

                returning(job.into())
            }
        }

        Continue(())
    }
}


pub fn process_result(parts: &mut Vec<MultilevelVecMap<Distance>>, mut tasks: &mut TaskSet, result: WorkerResult, stats: Option<JobStats>) -> ControlFlow<Result<JobResult, anyhow::Error>> {
    match bincode::serde::decode_from_slice::<TaskResult,_>(&result.data, config::standard()) {
        Err(err) => {

            return Break(Ok(JobResult::Error{ error_message: err.to_string(), stats }))
        },
        Ok((task_result, _)) => {
            let taxi = task_result.taxi;
            let [combination1, combination2] = task_result.result;
            if tasks.remove(result.id) {

                parts[taxi].insert(&split(combination1.0), combination1.1);
                parts[taxi].insert(&split(combination2.0), combination2.1);
            }
        }
    }

    Continue(())
}

