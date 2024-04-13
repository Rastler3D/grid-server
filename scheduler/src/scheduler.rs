use std::cmp::Ordering;
use std::net::SocketAddr;
use std::time::Duration;
use anyhow::anyhow;
use awaitable_bool::AwaitableBool;
use futures_util::{SinkExt, StreamExt};
use quinn::Endpoint;
use serde::{Deserialize, Serialize};
use shared::node_manager::{NodeManagerRequest, NodeManagerResponse};
use shared::platform::Platform;
use shared::scheduler::{WorkerJobData, SchedulerRequest, SchedulerResponse, JobStats};
use shared::utils::buffer::Buffer;
use shared::utils::priority_queue::{Max, Min, PriorityQueue, RefMut};
use shared::worker::{TaskId, WorkerError, WorkerId, WorkerRequest, WorkerResponse, WorkerResult, WorkerTask};
use tokio::select;
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{info, instrument};
use uuid::Uuid;
use crate::cli::Config;
use crate::command::Command;
use crate::connection_manager::ConnectionManager;
use crate::endpoint_config::endpoint_config::EndpointConfigBuilder;
use crate::http_server::start_server;
use crate::node_manager::{NodeManagerConnection, NodeManagerEvent};
use crate::nodes::WorkerEvent;
use crate::task_manager::{TaskManagerConnection, TaskManagerEvent};



const DEFAULT_PORT: u16 = 7295;
pub struct ScheduledJob{
    pub total_tasks: u128,
    pub scheduled_tasks: u128,
    pub received_tasks: u128,
    pub completed_tasks: u128,
    pub failed_tasks: u128,
    pub avg_task_completion_time: Duration,
    pub job: WorkerJobData,
    pub id: Uuid,

}

pub struct Scheduler {
    commands: ReceiverStream<Command>,
    scheduled_job: Option<ScheduledJob>,
    connection_manager: ConnectionManager,
    tasks_queue: PriorityQueue<Task,Min>,
    workers_queue: PriorityQueue<Worker,Max>,
    results_buffer: Buffer<WorkerResult,200>,
    need_schedule: AwaitableBool,
    config: Config
}

impl Scheduler {
    pub async fn new(config: Config) -> Result<Self, anyhow::Error> {
        let (sender,receiver) = channel(1024);
        let _ = start_server(config.bind_api_server_address, sender).await?;
        let (client_config, server_config) = EndpointConfigBuilder {
            certificate_provider: None,
            name: config.name.clone(),
        }.build()?;
        let mut endpoint = Endpoint::server(server_config, config.bind_address)?;
        endpoint.set_default_client_config(client_config);

        let connection_manager = ConnectionManager::new(endpoint, config.node_manager_address)?;

        Ok(Scheduler {
            scheduled_job: None,
            commands: ReceiverStream::new(receiver),
            connection_manager,
            tasks_queue: PriorityQueue::new(),
            workers_queue: PriorityQueue::new(),
            results_buffer: Buffer::new(),
            need_schedule: AwaitableBool::new(false),
            config,
        })
    }
    pub async fn start(&mut self ) -> Result<(), anyhow::Error> {
        loop {
            select! {
                Some(request) = self.connection_manager.task_manager.recv() => {
                    self.process_task_manager(request).await
                },
                Some(request) = self.connection_manager.node_manager.recv() => {
                    self.process_node_manager(request).await?
                },
                Some(request) = self.connection_manager.nodes.recv() => {
                    self.process_workers(request).await;
                },
                Some(command) = self.commands.next() => {
                    self.process_command(command).await;
                }
                _ = self.need_schedule.wait_true() => {
                    self.process_scheduling().await;
                }
            }
        }

        Ok(())
    }
    #[instrument(skip_all)]
    pub async fn process_scheduling(&mut self) {
        let Some(scheduled_job) = &mut self.scheduled_job else { return };
        info!("Scheduling tasks");
        match self.workers_queue.peek_mut().map(|x| (x.can_accept(), x.need_replace(), x)){
            Some((can_accept, need_replace, mut worker)) if can_accept > 0 || need_replace > 0 => {
                let can_accept = worker.can_accept();
                let mut tasks = Vec::with_capacity(can_accept);
                for _ in 0..can_accept{
                    match self.tasks_queue.peek_mut().map(|x| (!x.has_assignments(), x)){
                        Some((not_assigned,mut task)) if not_assigned || scheduled_job.scheduled_tasks == scheduled_job.total_tasks => {
                            task.assignments.push(worker.id);
                            worker.assignments.push(task.task_data.id);
                            tasks.push(task.task_data.clone());
                            scheduled_job.scheduled_tasks += not_assigned as u128;
                        }
                        _ => {
                            self.need_schedule.set_false();
                            break
                        },
                    }
                }

                let need_replace = worker.need_replace();
                let mut replacements = Vec::with_capacity(need_replace);
                for i in 0..need_replace{
                    match self.tasks_queue.peek_mut().map(|x| (!x.has_assignments(), x)){
                        Some((not_assigned,mut task)) if not_assigned || scheduled_job.scheduled_tasks == scheduled_job.total_tasks => {
                            if let Some(task_id) = worker.replacements.pop(){
                                task.assignments.push(worker.id);
                                worker.assignments.push(task.task_data.id);
                                replacements.push((task_id,task.task_data.clone()));
                                scheduled_job.scheduled_tasks += not_assigned as u128;
                            }
                        }
                        _ => {
                            self.need_schedule.set_false();
                            break
                        },
                    }
                }

                if !tasks.is_empty() || !replacements.is_empty(){
                    info!(tasks = %tasks.len(), replacements = %replacements.len(), worker = worker.id, "Sending tasks to worker");

                    let job_data = if !worker.has_job_data {
                        worker.has_job_data = true;
                        (scheduled_job.job.data.get_mut(&worker.platform).cloned().map(|data| (scheduled_job.job.args.clone(), data)))
                    } else { None };

                    self.connection_manager.nodes.send(worker.id, WorkerRequest {
                        job: scheduled_job.id,
                        job_data,
                        replacements,
                        tasks,
                    }).await;
                }

            },
            _ => {
                self.need_schedule.set_false();
            }
        }
    }

    #[instrument(skip_all)]
    pub async fn process_node_manager(&mut self, response: NodeManagerEvent) -> Result<(), anyhow::Error> {
        match response {
            NodeManagerEvent::Message(NodeManagerResponse::Connect (workers)) => {
                info!(?workers, "Node manager sent nodes");
                for worker in workers{
                    self.connection_manager.nodes.connect_node(worker);
                }
            },
            NodeManagerEvent::Message(NodeManagerResponse::Disconnect(worker_id)) => {
                info!(%worker_id, "Node manager disconnect node");
                self.process_worker_disconnect(worker_id);
            },
            NodeManagerEvent::Message(NodeManagerResponse::RestoredSession) => {
                info!("Restored session with node manager");
                if !self.connection_manager.node_manager.connected{
                    self.connection_manager.node_manager.connected = true;
                }
            },
            NodeManagerEvent::Message(NodeManagerResponse::CreatedSession) => {
                info!("Create session with node manager");
                if !self.connection_manager.node_manager.connected{
                    self.connection_manager.node_manager.connected = true;
                    self.workers_queue.clear();
                    self.connection_manager.nodes.nodes.clear();
                    if let Some(scheduler_job) = &mut self.scheduled_job{
                        scheduler_job.scheduled_tasks = scheduler_job.completed_tasks;
                        self.tasks_queue.map(|task|{
                            task.assignments.clear();
                        });

                        self.connection_manager.node_manager.send(NodeManagerRequest::WorkersDemand {
                            required_workers: None,
                            supported_platforms: scheduler_job.job.data.keys().cloned().collect()
                        }).await;
                    }
                }
            }
            NodeManagerEvent::Disconnected => {
                info!("Node manager disconnected");
                self.connection_manager.connect_node_manager(self.config.node_manager_address);
            },
            NodeManagerEvent::Connected => {
                info!("Node manager connected");
            },
            NodeManagerEvent::ConnectionNotEstablished => {
                return Err(anyhow!("Failed to establish connection with node manager"))
            }
        }
        Ok(())
    }

    pub async fn process_command(&mut self, command: Command) {
        match command {
            Command::ScheduledJob(returning) => {
                if let Some(scheduler_job) = &self.scheduled_job{
                    returning(Some(scheduler_job.into()));
                } else {
                    returning(None);
                }
            },
            Command::Workers(limit, returning) => {
                let workers = self.workers_queue.iter().take(limit as usize).cloned().collect();

                returning(workers);
            },

            Command::Tasks(limit, returning) => {
                let tasks = self.tasks_queue.iter().take(limit as usize).cloned().collect();

                returning(tasks);
            }
        }
    }

    #[instrument(skip_all)]
    pub async fn process_workers(&mut self, worker_event: WorkerEvent) {
         match worker_event{
            WorkerEvent::Connected (worker_info, node) => {
                info!(?worker_info, "Worker connected");
                self.need_schedule.set_true();
                self.connection_manager.nodes.insert_connected_node(worker_info.worker_id, node);
                self.workers_queue.push(worker_info.worker_id, Worker{
                    subworkers: worker_info.subworkers,
                    platform: worker_info.platform,
                    id: worker_info.worker_id,
                    address: worker_info.address,
                    has_job_data: false,
                    assignments: Vec::new(),
                    replacements: Vec::new(),
                });
            }
            WorkerEvent::Disconnected(worker_id) => {
                info!(%worker_id, "Worker disconnected");
               self.process_worker_disconnect(worker_id)
            },

            WorkerEvent::Message(worker_id, WorkerResponse::Result(result)) => {
                let Some(scheduled_job) = &mut self.scheduled_job else { return };
                info!(%worker_id,task_id = %result.id, execution_time = ?result.execution_time, "Worker complete task");
                match self.tasks_queue.remove(result.id) {
                    None => {
                        match self.workers_queue.get_mut(worker_id) {
                            None => {}
                            Some(mut worker) => {
                                self.need_schedule.set_true();
                                worker.replacements
                                    .iter()
                                    .position(|x| x == &result.id)
                                    .map(|pos| worker.assignments.swap_remove(pos));
                            }
                        }
                    }
                    Some((task_id, mut task)) => {
                        self.need_schedule.set_true();
                        scheduled_job.avg_task_completion_time = Duration::from_nanos(((scheduled_job.avg_task_completion_time.as_nanos() * (scheduled_job.completed_tasks / (scheduled_job.completed_tasks + 1))) + (result.execution_time.as_nanos() / (scheduled_job.completed_tasks + 1))) as u64);
                        scheduled_job.completed_tasks +=1;
                        if let Some(batch) = self.results_buffer.push(result){
                            self.connection_manager
                                .task_manager
                                .send(SchedulerResponse::TaskResults(batch)).await;
                        }
                        task.assignments
                            .iter()
                            .position(|x| x == &worker_id)
                            .map(|pos| task.assignments.swap_remove(pos));
                        match self.workers_queue.get_mut(worker_id) {
                            None => {}
                            Some(mut worker) => {
                                worker.assignments
                                    .iter()
                                    .position(|x| x == &task_id)
                                    .map(|pos| worker.assignments.swap_remove(pos));
                            }
                        }

                        for id in task.assignments {
                            match self.workers_queue.get_mut(id) {
                                None => {}
                                Some(mut worker) => {
                                    worker.assignments
                                        .iter()
                                        .position(|x| x == &task_id)
                                        .map(|pos| worker.assignments.swap_remove(pos));
                                    worker.replacements.push(task_id);
                                }
                            }
                        }
                        if scheduled_job.completed_tasks == scheduled_job.total_tasks{
                            let batch = Vec::from(self.results_buffer.filled_slice());
                            self.results_buffer.clear();
                            self.connection_manager
                                .task_manager
                                .send(SchedulerResponse::JobComplete(batch, JobStats{
                                    total_tasks: scheduled_job.total_tasks,
                                    completed_tasks: scheduled_job.completed_tasks,
                                    failed_tasks: scheduled_job.failed_tasks,
                                    avg_task_completion_time: scheduled_job.avg_task_completion_time
                                })).await;
                            self.connection_manager.node_manager.send(NodeManagerRequest::ReleaseDemand).await;
                            self.reset_scheduler();
                        }
                    }
                }
            },
             WorkerEvent::Message(worker_id, WorkerResponse::Error(WorkerError::LibraryMissing{ replacements, tasks })) => {
                 let Some(scheduled_job) = &mut self.scheduled_job else { return };
                 info!(%worker_id, "Worker library missing");
                 if let Some(mut worker) = self.workers_queue.get_mut(worker_id){
                     self.need_schedule.set_true();
                     worker.has_job_data = false;
                     for task_id in tasks {
                         match self.tasks_queue.get_mut(task_id) {
                             None => {
                                 worker.replacements
                                     .iter()
                                     .position(|x| x == &task_id)
                                     .map(|pos| worker.assignments.swap_remove(pos));
                             },
                             Some(mut task) => {
                                 scheduled_job.scheduled_tasks -= !task.has_assignments() as u128;
                                 task.assignments
                                     .iter()
                                     .position(|x| x == &worker_id)
                                     .map(|pos| task.assignments.swap_remove(pos));
                                 worker.assignments
                                     .iter()
                                     .position(|x| x == &task_id)
                                     .map(|pos| worker.assignments.swap_remove(pos));
                             }
                         }
                     }
                     for (replace_task_id, task_id) in replacements {
                         match self.tasks_queue.get_mut(task_id) {
                             None => {
                                 worker.replacements
                                     .iter()
                                     .position(|x| x == &task_id)
                                     .map(|pos| worker.assignments.swap_remove(pos));
                             },
                             Some(mut task) => {
                                 scheduled_job.scheduled_tasks -= !task.has_assignments() as u128;
                                 task.assignments
                                     .iter()
                                     .position(|x| x == &worker_id)
                                     .map(|pos| task.assignments.swap_remove(pos));
                                 worker.assignments
                                     .iter()
                                     .position(|x| x == &task_id)
                                     .map(|pos| worker.assignments.swap_remove(pos));
                                 worker.replacements
                                     .push(replace_task_id);
                             }
                         }
                     }
                 }
             },
             WorkerEvent::Message(worker_id, WorkerResponse::Error(WorkerError::TaskExecutionError { task_id, error})) => {
                 let Some(scheduled_job) = &mut self.scheduled_job else { return };
                 info!(%worker_id,%task_id, %error, "Worker failed execute task");
                 if let Some(mut worker) = self.workers_queue.get_mut(worker_id){
                     self.need_schedule.set_true();
                     match self.tasks_queue.get_mut(task_id){
                         Some(mut task) => {
                             scheduled_job.scheduled_tasks -= !task.has_assignments() as u128;
                             task.assignments
                                 .iter()
                                 .position(|x| x == &worker_id)
                                 .map(|pos| task.assignments.swap_remove(pos));
                             worker.assignments
                                 .iter()
                                 .position(|x| x == &task_id)
                                 .map(|pos| worker.assignments.swap_remove(pos));
                             if task.failed_executions == self.config.failed_executions_limit {
                                 self.connection_manager
                                     .task_manager
                                     .send(SchedulerResponse::TaskExecutionError(error, JobStats{
                                         total_tasks: scheduled_job.total_tasks,
                                         completed_tasks: scheduled_job.completed_tasks,
                                         failed_tasks: scheduled_job.failed_tasks,
                                         avg_task_completion_time: scheduled_job.avg_task_completion_time
                                     }))
                                     .await;
                                 self.scheduled_job.take();
                             } else {
                                 task.failed_executions += 1;
                             }
                         },
                         None => {
                             worker.replacements
                                 .iter()
                                 .position(|x| x == &task_id)
                                 .map(|pos| worker.assignments.swap_remove(pos));
                         }
                     }
                 }
             },
             WorkerEvent::Message(worker_id, WorkerResponse::Error(WorkerError::NoFreeExecutors{ tasks })) => {
                 let Some(scheduled_job) = &mut self.scheduled_job else { return };
                 info!(%worker_id, "Worker not have free executors");
                 if let Some(mut worker) = self.workers_queue.get_mut(worker_id){
                     if worker.subworkers != 0 {
                         worker.subworkers -= tasks.len();
                         self.need_schedule.set_true();
                     }
                     for task_id in tasks {
                         match self.tasks_queue.get_mut(task_id) {
                             None => {
                                 worker.replacements
                                     .iter()
                                     .position(|x| x == &task_id)
                                     .map(|pos| worker.assignments.swap_remove(pos));
                             }
                             Some(mut task) => {
                                 scheduled_job.scheduled_tasks -= !task.has_assignments() as u128;
                                 task.assignments
                                     .iter()
                                     .position(|x| x == &worker_id)
                                     .map(|pos| task.assignments.swap_remove(pos));
                                 worker.assignments
                                     .iter()
                                     .position(|x| x == &task_id)
                                     .map(|pos| worker.assignments.swap_remove(pos));
                             }
                         }
                     }
                 }
             }
        }
    }


    #[instrument(skip_all)]
    pub async fn process_task_manager(&mut self, request: TaskManagerEvent) {
        match request {
            TaskManagerEvent::Message(SchedulerRequest::HelloMessage) => {},
            TaskManagerEvent::Message(SchedulerRequest::NewJob { job, id, total_tasks }) => {
                info!(%id, %total_tasks, "Task manager assigned new job");

                self.reset_scheduler();
                self.tasks_queue.reserve_exact(total_tasks as usize);
                if self.connection_manager.node_manager.connected {
                    self.connection_manager.node_manager.send(NodeManagerRequest::WorkersDemand {
                        required_workers: None,
                        supported_platforms: job.data.keys().cloned().collect(),
                    }).await;
                }
                self.scheduled_job = Some(ScheduledJob { id, total_tasks, job, scheduled_tasks: 0, received_tasks: 0, completed_tasks: 0, failed_tasks: 0, avg_task_completion_time: Default::default() });
            },
            TaskManagerEvent::Message(SchedulerRequest::Task(batch)) => {
                info!(tasks = %batch.len(), "Task manager sent tasks batch");
                let Some(scheduled_job) = &mut self.scheduled_job else { return };
                scheduled_job.received_tasks += batch.len() as u128;
                for task_data in batch {
                    self.tasks_queue.push(task_data.id, Task {
                        failed_executions: 0,
                        assignments: Vec::new(),
                        task_data,
                    });
                }
                self.need_schedule.set_true();
            },
            TaskManagerEvent::Disconnect => {
                if let Some(addr) = self.connection_manager.task_manager.connected{
                    info!(address = %addr, "Task manager disconnected. Stopping current job.");
                    self.scheduled_job.take();
                    self.connection_manager.task_manager.connected = None;
                    self.connection_manager.node_manager.send(NodeManagerRequest::ReleaseDemand).await;
                    self.reset_scheduler();
                }
            },
            TaskManagerEvent::Connected(addr) => {
                info!(address = %addr, "Task manager connected");
                self.connection_manager.task_manager.connected = Some(addr);
            }
        }
    }

    #[instrument(skip_all)]
    pub fn process_worker_disconnect(&mut self, worker_id: WorkerId){
        info!(%worker_id, "Disconnecting worker");
        self.connection_manager.nodes.remove_node(worker_id);
        match self.workers_queue.remove(worker_id) {
            Some((_, worker)) => {
                self.need_schedule.set_true();
                if let Some(scheduled_job) = &mut self.scheduled_job {
                    for task_id in worker.assignments {
                        match self.tasks_queue.get_mut(task_id) {
                            None => {}
                            Some(mut task) => {
                                task.assignments
                                    .iter()
                                    .position(|x| x == &worker_id)
                                    .map(|pos| task.assignments.swap_remove(pos));
                                scheduled_job.scheduled_tasks -= !task.has_assignments() as u128;
                            }
                        }
                    }
                }
            }
            None => {}
        }
    }

    pub fn reset_scheduler(&mut self){
        self.workers_queue.clear();
        self.tasks_queue.clear();
        self.results_buffer.clear();
        self.connection_manager.nodes.nodes.clear();
        self.need_schedule.set_false();
    }

}

#[derive(Eq,PartialEq, Serialize,Deserialize, Clone)]
pub struct Worker{
    id: usize,
    subworkers: usize,
    platform: Platform,
    has_job_data: bool,
    address: SocketAddr,
    replacements: Vec<TaskId>,
    assignments: Vec<TaskId>
}

impl Worker{
    pub fn priority(&self) -> usize{
        self.subworkers - self.assignments.len()
    }

    pub fn can_accept(&self) -> usize{
        self.subworkers - (self.assignments.len() + self.replacements.len())
    }
    pub fn need_replace(&self) -> usize{
        self.replacements.len()
    }
}

impl PartialOrd<Self> for Worker {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {

        match self.priority().partial_cmp(&other.priority())? {
            Ordering::Equal => self.has_job_data.partial_cmp(&other.has_job_data),
            x => Some(x)
        }
    }
}

impl Ord for Worker{
    fn cmp(&self, other: &Self) -> Ordering {
        match self.has_job_data.cmp(&other.has_job_data) {
            Ordering::Equal => self.priority().cmp(&other.priority()),
            x => x
        }
    }
}

#[derive(Eq,PartialEq, Serialize, Deserialize, Clone)]
pub struct Task {
    task_data: WorkerTask,
    failed_executions: u16,
    assignments: Vec<WorkerId>,
}

impl Task{
    pub fn priority(&self) -> usize{
        self.assignments.len()
    }

    pub fn has_assignments(&self) -> bool{
        !self.assignments.is_empty()
    }
}

impl PartialOrd<Self> for Task {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.priority().partial_cmp(&other.priority())
    }
}

impl Ord for Task{
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority().cmp(&other.priority())
    }
}




type Priority = usize;