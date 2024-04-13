use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::thread;
use std::time::{Duration, Instant};
use interprocess::local_socket::tokio::{OwnedReadHalf, OwnedWriteHalf};
use lru::LruCache;
use shared::executor::{ExecutorJob, ExecutorResult};
use shared::utils::bin_codec::BinCodec;
use shared::worker::TaskId;
use tokio::process::Child;
use tokio::{runtime, select};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::{Mutex, Notify, RwLock};
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::sync::CancellationToken;
use tracing::info;
use uuid::Uuid;
use wasmtime::{Config, Engine, Store};
use wasmtime::component::{bindgen, Linker};
use wasmtime_wasi::{WasiCtxBuilder, WasiP1Ctx};
use wasmtime::component::Component;
use crate::wasm::wasm_worker::WasmModule;

bindgen!({
   world: "job",
   path: "wit/world.wit",
   async: true,
});

pub type  WasmExecutorStream = ReceiverStream<ExecutorResult>;

pub struct WasmExecutor{
    pub(crate) engine: Engine,
    pub(crate) cancellation_token: CancellationToken,
    pub(crate) interruption: Arc<Notify>,
    pub(crate) sink: Sender<ExecutorJob>
}

impl WasmExecutor {

    pub async fn new(config: &Config, cache: Arc<RwLock<LruCache<Uuid, WasmModule>>>) -> wasmtime::Result<(WasmExecutor, WasmExecutorStream)>{
        let engine = Engine::new(config)?;
        let engine1 = engine.clone();
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::command::add_to_linker(&mut linker)?;
        let (send_job , mut recv_job) = channel(10);
        let (send_result, recv_result) = channel(10);
        let cancellation_token = CancellationToken::new();
        let cancellation = cancellation_token.clone();
        let interruption = Arc::new(Notify::new());
        let wasm_executor = WasmExecutor{
            cancellation_token,
            interruption: interruption.clone(),
            engine: engine1,
            sink: send_job,
        };

        thread::spawn(move ||{
            let mut state = ExecutorState{ linker,  recv_job,  engine, send_result, cache };
            let runtime = runtime::Builder::new_current_thread().enable_all().build().unwrap();

            runtime.block_on(async {
                loop {
                    select! {
                        _ = cancellation.cancelled() => break,
                        _ = interruption.notified() => continue,
                        _ = state.execute() => continue,
                    }
                }
            })
        });



        Ok((wasm_executor, ReceiverStream::new(recv_result)))
    }
    pub fn abort(&mut self){
        self.engine.increment_epoch();
        self.interruption.notify_one();
    }
    pub async fn start_job(&mut self, job: ExecutorJob){
        self.sink.send(job).await.expect("start job failed")
    }
}

impl Drop for WasmExecutor{
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}

struct ExecutorState{
    engine: Engine,
    linker: Linker<WasiP1Ctx>,
    cache: Arc<RwLock<LruCache<Uuid,WasmModule>>>,
    recv_job: Receiver<ExecutorJob>,
    send_result: Sender<ExecutorResult>
}

impl ExecutorState {

    async fn execute(&mut self){
        let Some(executor_job) = self.recv_job.recv().await else { return };
        let job_id = executor_job.job_id;
        let task_id = executor_job.task_id;
        let mut execution_time = Duration::new(0, 0);
        let result: Result<Result<Vec<u8>, String>, wasmtime::Error> = try {
            let mut lock = self.cache.read().await;
            let component = match lock.peek(&job_id).unwrap(){
                WasmModule::Source( ref bytes) => {
                    drop(lock);
                    let mut lock = self.cache.write().await;
                    let module = lock.peek_mut(&job_id).unwrap();
                    match module{
                        WasmModule::Compiled(bytes) => {
                            unsafe { Component::deserialize(&self.engine, bytes)? }
                        }
                        WasmModule::Source(bytes) => {
                            let component = Component::new(&self.engine, bytes)?;
                            let precompiled_bytes = component.serialize()?;
                            *module = WasmModule::Compiled(precompiled_bytes);
                            component
                        }
                    }
                },
                WasmModule::Compiled( ref precompiled_bytes) => {
                    let module = unsafe { Component::deserialize(&self.engine, precompiled_bytes)? };
                    drop(lock);

                    module
                }
            };

            let wasi = WasiP1Ctx::new(WasiCtxBuilder::new().inherit_stdio().build());
            let mut store = Store::new(&self.engine, wasi);
            store.set_epoch_deadline(1);
            store.epoch_deadline_async_yield_and_update(1);
            let (job, _) = Job::instantiate_async(&mut store, &component, &self.linker).await?;
            let now = Instant::now();
            let result = job.call_execute_job(&mut store, &executor_job.job_data, &executor_job.task_data).await?;
            execution_time = now.elapsed();

            result
        };
        let result = result.map_err(|err| err.to_string()).and_then(|x| x);
        self.send_result.send(ExecutorResult { job_id,task_id, execution_time, result }).await;
    }
}


