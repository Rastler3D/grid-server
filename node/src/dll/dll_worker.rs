use std::collections::HashMap;
use std::ffi::{OsStr};
use std::mem::{replace, take};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process;
use std::process::Stdio;
use futures_sink::Sink;
use interprocess::local_socket::tokio::LocalSocketListener;
use libloading::library_filename;
use lru::LruCache;
use shared::executor::{ExecutorJob, ExecutorResult};
use shared::platform::{CURRENT_PLATFORM, Platform};
use shared::utils::bin_codec::BinCodec;
use shared::worker::{TaskId, WorkerError, WorkerRequest, WorkerResponse, WorkerResult, WorkerTask};
use tokio::{fs};
use tokio::process::Command;
use tokio_stream::{StreamExt, StreamMap};
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::compat::{FuturesAsyncReadCompatExt, FuturesAsyncWriteCompatExt};
use tracing::info;
use uuid::Uuid;
use wasmtime::component::__internal::anyhow;
use crate::dll::dll_executor::{DllExecutor, DllExecutorEvent, DllExecutorStream};
use crate::worker::Worker;

pub struct DllWorker {
    free_executors: Vec<u32>,
    assignments: HashMap<(Uuid,TaskId), u32>,
    executors: HashMap<u32, DllExecutor>,
    listener: LocalSocketListener,
    message: StreamMap<u32, DllExecutorStream>,
    job_cache: LruCache<Uuid, ()>,
    job_args: LruCache<Uuid, Vec<u8>>
}

impl Worker for DllWorker{
    const PLATFORM: Platform = CURRENT_PLATFORM;

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
        info!("Resetting worker");
        let mut assignments = replace(&mut self.assignments, HashMap::new());
        for (_, executor_id) in assignments.drain(){
            let executor_id = self.restart_executor(executor_id).await;
            self.free_executors.push(executor_id);
        }
        self.assignments = assignments;
    }
}

impl DllWorker {
    pub async fn new() -> anyhow::Result<Self>{
        let listener = LocalSocketListener::bind(process::id().to_string())?;
        let executor_num = std::thread::available_parallelism()
            .map(NonZeroUsize::get)
            .unwrap_or(1);

        let mut node = Self{
            listener: listener,
            assignments: HashMap::with_capacity(executor_num),
            executors: HashMap::with_capacity(executor_num),
            job_cache: LruCache::new(NonZeroUsize::new(10).unwrap()),
            message: StreamMap::with_capacity(executor_num),
            free_executors: Vec::with_capacity(executor_num),
            job_args: LruCache::new(NonZeroUsize::new(10).unwrap()),
        };

        for _ in 0..executor_num{
            let executor_id = node.start_executor().await;
            node.free_executors.push(executor_id);
        }

        Ok(node)
    }
    pub async fn process_message(&mut self) -> Option<WorkerResponse>{

        let Some((executor_id, result)) = self.message.next().await else {
            return None;
        };
        match result {
            DllExecutorEvent::Terminated(error) => {
                info!(%executor_id, %error, "Executor terminated");
                let executor_id = self.restart_executor(executor_id).await;
                self.free_executors.push(executor_id);
                if let Some(((_, task_id), _)) = self.assignments.extract_if(|k, &mut v| v == executor_id).next() {
                    return Some(WorkerResponse::Error(WorkerError::TaskExecutionError {
                        task_id,
                        error
                    }))
                }
            },
            DllExecutorEvent::Result(ExecutorResult{ job_id, task_id, result, execution_time }) => {
                if let Some(executor_id) = self.assignments.remove(&(job_id,task_id)){
                    self.free_executors.push(executor_id);

                    return match result {
                        Ok(data) => {
                            info!(%executor_id, %job_id, %task_id, ?execution_time, "Executor complete task");
                            Some(WorkerResponse::Result(WorkerResult {
                                id: task_id,
                                data: data,
                                execution_time,
                            }))
                        }
                        Err(error) => {
                            info!(%executor_id, %job_id, %task_id, ?execution_time, %error, "Executor failed task");
                            Some(WorkerResponse::Error(WorkerError::TaskExecutionError{
                                task_id,
                                error: error.to_string()
                            }))
                        }
                    }
                }
            }
        }

        None
    }
    pub async fn process_request(&mut self, mut request: WorkerRequest) -> Result<(), WorkerError>{
        self.process_library(&mut request).await?;
        self.process_task_replacement(request.job, request.replacements).await;
        self.process_tasks_assignment(request.job, request.tasks).await?;

        Ok(())
    }

    pub async fn process_tasks_assignment(&mut self, job_id: Uuid, mut tasks: Vec<WorkerTask>) -> Result<(), WorkerError>{
        while let Some(task) = tasks.pop(){
            match self.free_executors.pop() {
                Some(executor_id) => {
                    info!(%executor_id, task_id = task.id, "Assigning task to executor");
                    self.assign_task(executor_id,job_id,task).await
                }
                None => {
                    info!(%job_id, task_id = task.id, "No free executor to assign task");
                    tasks.push(task);
                    return Err(WorkerError::NoFreeExecutors {
                        tasks: tasks.iter().map(|x| x.id).collect(),
                    })
                }
            }
        }

        Ok(())
    }
    pub async fn process_task_replacement(&mut self, job_id: Uuid,  replacements: Vec<(TaskId, WorkerTask)>){
        for (replace_task_id,task) in replacements{
            if let Some(executor_id) = self.assignments.remove(&(job_id,replace_task_id)){
                let executor_id = self.restart_executor(executor_id).await;
                info!(%executor_id, %replace_task_id, new_task_id = task.id, "Replacing executor task");
                self.assign_task(executor_id,job_id,task).await;
            }
        }
    }
    pub async fn process_library(&mut self, request:&mut WorkerRequest) -> Result<(), WorkerError>{
        if let None = self.job_cache.get(&request.job) {
            match take(&mut request.job_data) {
                None => {
                    info!(job_id = %request.job, "Library missing");
                    return Err(WorkerError::LibraryMissing{
                        tasks: request.tasks.iter().map(|x| x.id).collect(),
                        replacements: request.replacements.iter().map(|(id, task)| (*id, task.id)).collect()
                    })
                },
                Some((args, data)) => {
                    info!(job_id = %request.job ,"Caching job data");
                    Self::add_library(request.job, data).await;
                    self.job_args.push(request.job, args);
                    if let Some((uuid,_)) = self.job_cache.push(request.job,()) {
                        Self::remove_library(uuid).await;
                    }
                }
            }
        }

        Ok(())
    }
    pub async fn add_library(uuid: Uuid, data: Vec<u8>){
        let file_path = [OsStr::new("library"),&library_filename(uuid.to_string())].iter().collect::<PathBuf>();
        fs::create_dir_all("library").await.unwrap();
        fs::write(file_path, data).await.unwrap();
    }

    pub async fn remove_library(uuid: Uuid){
        let file_path = [OsStr::new("library"),&library_filename(uuid.to_string())].iter().collect::<PathBuf>();
        fs::remove_file(file_path).await.unwrap();
    }

    pub async fn assign_task(&mut self, executor_id: u32, job_id: Uuid, task: WorkerTask ){
        if let Some(executor) = self.executors.get_mut(&executor_id) {
            self.assignments.insert((job_id,task.id), executor_id);
            if let Some(args) = self.job_args.get(&job_id) {
                info!(%executor_id, %job_id, task_id = %task.id, "Task assigned to executor");
                executor.start_job(ExecutorJob {
                    job_id: job_id,
                    job_data: args.clone(),
                    task_id: task.id,
                    task_data: task.data
                }).await;
            }
        }
    }

    pub async fn restart_executor(&mut self, executor_id: u32) -> u32{
        info!(%executor_id, "Restarting executor");
        self.stop_executor(executor_id).await;
        self.start_executor().await
    }
    pub async fn stop_executor(&mut self, executor_id: u32){
        info!(%executor_id, "Stopping executor");
        if let Some(_) = self.executors.remove(&executor_id) {
            self.free_executors.retain(|&id| id != executor_id);
            self.message.remove(&executor_id);
        }
    }

    pub async fn start_executor(&mut self) -> u32{
        let process = Command::new("executor")
            .arg(process::id().to_string())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .current_dir(".")
            .spawn()
            .unwrap();
        let process_id = process.id().unwrap();
        let socket = self.listener
            .accept()
            .await
            .unwrap();

        let (read,write) = socket.into_split();
        let sink = FramedWrite::new(write.compat_write(), BinCodec::new());
        let stream = FramedRead::new(read.compat(), BinCodec::new());
        self.executors.insert(process_id, DllExecutor {
            sink,
        });
        self.message.insert(process_id, DllExecutorStream {
            stream,
            process,
        });
        info!(executor_id = %process_id, "Starting executor");

        process_id
    }
}


impl Drop for DllWorker{
    fn drop(&mut self) {
        std::fs::remove_dir_all("library");
    }
}