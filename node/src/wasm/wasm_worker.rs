use std::collections::HashMap;
use std::mem::{replace, take};
use std::num::NonZeroUsize;
use std::sync::Arc;
use lru::LruCache;
use shared::worker::{TaskId, WorkerError, WorkerRequest, WorkerResponse, WorkerResult, WorkerTask};
use slab::Slab;
use tokio::sync::RwLock;
use tokio_stream::{StreamExt, StreamMap};
use futures_sink::Sink;
use tracing::{debug, info};
use shared::executor::{ExecutorJob, ExecutorResult};
use shared::platform::{Platform, WASM_PLATFORM};
use uuid::Uuid;
use wasmtime::component::__internal::anyhow;
use wasmtime::Config;
use crate::wasm::wasm_executor::{WasmExecutor, WasmExecutorStream};

use crate::worker::Worker;

pub struct WasmWorker {
    free_executors: Vec<usize>,
    assignments: HashMap<(Uuid,TaskId), usize>,
    executors: Slab<WasmExecutor>,
    message: StreamMap<usize,WasmExecutorStream>,
    job_cache: Arc<RwLock<LruCache<Uuid, WasmModule>>>,
    job_args: LruCache<Uuid, Vec<u8>>,
    config: Config
}

impl Worker for WasmWorker{
    const PLATFORM: Platform = WASM_PLATFORM;

    async fn process(&mut self, request: WorkerRequest) -> Result<(),WorkerError> {
        self.process_request(request).await
    }

    async fn recv(&mut self) -> WorkerResponse {
        loop {
            if let Some(response) = self.process_message().await{
                return response;
            }
        }
    }

    async fn reset(&mut self) {
        debug!("Resetting worker");
        let mut assignments = replace(&mut self.assignments, HashMap::new());
        for (_, id) in assignments.drain(){
            self.abort_executor(id);
            self.free_executors.push(id);
        }
        self.assignments = assignments;
    }
}

impl WasmWorker {
    pub async fn new() -> anyhow::Result<Self>{

        let mut config = Config::new();
        config
            .epoch_interruption(true)
            .wasm_component_model(true)
            .async_support(true);

        let executors_num = std::thread::available_parallelism()
            .map(NonZeroUsize::get)
            .unwrap_or(1);

        let mut node = Self{
            assignments: HashMap::with_capacity(executors_num),
            executors: Slab::with_capacity(executors_num),
            job_cache: Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(10).unwrap()))),
            job_args: LruCache::new(NonZeroUsize::new(10).unwrap()),
            message: StreamMap::with_capacity(executors_num),
            free_executors: Vec::with_capacity(executors_num),
            config: config,
        };


        for _ in 0..executors_num{
            let executor_id = node.start_executor().await?;
            node.free_executors.push(executor_id);
        }

        Ok(node)
    }

    async fn process_message(&mut self) -> Option<WorkerResponse>{
        let Some((_, ExecutorResult{ job_id,task_id,result, execution_time})) = self.message.next().await else {
            return None;
        };
        if let Some(executor_id) = self.assignments.remove(&(job_id,task_id)){
            self.free_executors.push(executor_id);

            return match result {
                Ok(data) => {
                    debug!(%executor_id, %job_id, %task_id, ?execution_time, "Executor complete task");
                    Some(WorkerResponse::Result(WorkerResult {
                        id: task_id,
                        data: data,
                        execution_time,
                    }))
                }
                Err(error) => {
                    debug!(%executor_id, %job_id, %task_id, ?execution_time, %error, "Executor failed task");
                    Some(WorkerResponse::Error(WorkerError::TaskExecutionError{
                        task_id,
                        error: error.to_string()
                    }))
                }
            }
        }
        None
    }
    async fn process_request(&mut self, mut request: WorkerRequest) -> Result<(),WorkerError>{
        self.process_module(&mut request).await?;
        self.process_task_replacement(request.job, request.replacements).await;
        self.process_tasks_assignment(request.job, request.tasks).await?;

        Ok(())
    }

    async fn process_tasks_assignment(&mut self, job_id: Uuid, mut tasks: Vec<WorkerTask>) -> Result<(), WorkerError>{
        while let Some(task) = tasks.pop(){
            match self.free_executors.pop() {
                Some(executor_id) => {
                    debug!(%executor_id, %job_id, task_id = task.id, "Assigning task to executor");
                    self.assign_task(executor_id, job_id,task, false).await;
                }
                None => {
                    debug!(%job_id, task_id = task.id, "No free executor to assign task");
                    tasks.push(task);
                    return Err(WorkerError::NoFreeExecutors{
                        tasks: tasks.iter().map(|x| x.id).collect(),
                    })
                }
            }
        }


        Ok(())
    }
    async fn process_task_replacement(&mut self, job_id: Uuid,  replacements: Vec<(TaskId, WorkerTask)>){
        for (replace_task_id,task) in replacements{
            if let Some(executor_id) = self.assignments.remove(&(job_id,replace_task_id)){
                debug!(%executor_id, %replace_task_id, new_task_id = task.id, "Replacing executor task");
                self.assign_task(executor_id, job_id,task, true).await;

            }
        }
    }
    async fn process_module(&mut self, request: &mut WorkerRequest) -> Result<(), WorkerError>{
        let contains = self.job_cache.read().await.contains(&request.job);
        if !contains  {
            match take(&mut request.job_data) {
                None => {
                    debug!(job_id = %request.job, "Library missing");
                    return Err(WorkerError::LibraryMissing{
                        tasks: request.tasks.iter().map(|x| x.id).collect(),
                        replacements: request.replacements.iter().map(|(id, task)| (*id, task.id)).collect()
                    })
                },
                Some((args,data)) => {
                    self.job_cache.write().await.push(request.job, WasmModule::Source(data));
                    self.job_args.push(request.job, args);
                    debug!(job_id = %request.job ,"Job data cached");
                }
            }
        }

        Ok(())
    }


    async fn assign_task(&mut self, executor_id: usize, job: Uuid, task: WorkerTask, need_abort: bool){
        if let Some(executor) = self.executors.get_mut(executor_id){
            if need_abort{
                executor.abort();
            }
            self.assignments.insert((job,task.id), executor_id);
            if let Some(args) = self.job_args.get(&job){
                debug!(%executor_id, task_id = %task.id, job_id = %job, "Task assigned to executor");
                executor.start_job(ExecutorJob{
                    job_id: job,
                    job_data: args.clone(),
                    task_id: task.id,
                    task_data: task.data,
                }).await;
            }
        }
    }

    fn abort_executor(&mut self, executor_id: usize){
        if let Some(executor) = self.executors.get_mut(executor_id){
            debug!(%executor_id, "Aborted task execution");
            executor.abort();
        }
    }

    async fn restart_executor(&mut self, executor_id: usize) -> wasmtime::Result<usize>{
        debug!(%executor_id, "Restarting executor");
        self.stop_executor(executor_id).await;
        self.start_executor().await
    }
    async fn stop_executor(&mut self, executor_id: usize){
        debug!(%executor_id, "Stopping executor");
        if let Some(_) = self.executors.try_remove(executor_id) {
            self.free_executors.retain(|&id| id != executor_id);
            self.message.remove(&executor_id);
        }
    }

    async fn start_executor(&mut self) -> wasmtime::Result<usize>{
        let cache = self.job_cache.clone();
        let (executor, stream) = WasmExecutor::new(&self.config, cache).await?;

        let executor_id = self.executors.insert(executor);
        self.message.insert(executor_id, stream);
        debug!(%executor_id, "Starting executor");

        Ok(executor_id)
    }
}

pub enum WasmModule{
    Compiled(Vec<u8>),
    Source(Vec<u8>)
}

impl WasmModule{
    pub fn is_compiled(&self) -> bool{
        matches!(self, WasmModule::Compiled(_))
    }
    pub fn is_source(&self) -> bool{
        matches!(self, WasmModule::Source(_))
    }
}