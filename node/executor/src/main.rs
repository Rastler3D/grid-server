#![feature(try_blocks)]

use std::ffi::OsStr;
use clap::Arg;
use std::process::ExitCode;
use std::io::{Read, Write};
use std::mem::size_of;
use std::num::NonZeroUsize;
use std::panic::catch_unwind;
use std::path::{PathBuf};
use std::time::{Duration, Instant};
use anyhow::Context;
use bincode::config;
use interprocess::local_socket::LocalSocketStream;
use libloading::{Library, library_filename, Symbol};
use lru::LruCache;
use shared::executor::{ExecutorJob, ExecutorResult};
use uuid::Uuid;


const EXECUTOR: usize = 1;
const SOCKET: usize = 2;

type ExecutorFn = unsafe extern fn(Vec<u8>, Vec<u8>) -> Result<Vec<u8>, String>;

fn main() -> ExitCode {
    let command = clap::Command::new("executor")
        .arg(Arg::new("id").required(true).num_args(1))
        .get_matches();
    let id = command.get_one::<String>("id").expect("ID is required");

    let stream = match LocalSocketStream::connect(id.clone()) {
        Ok(channel) => channel,
        Err(err) => {
            eprintln!("{:?}", err);
            return ExitCode::FAILURE;
        }
    };

    if let Err(err) = unsafe { Executor::new(stream).start() }{
        eprintln!("{:?}", err.to_string());
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

struct Executor{
    stream: LocalSocketStream,
    lru_cache: LruCache<Uuid,Library>,
}
impl Executor {
    pub fn new(stream: LocalSocketStream) -> Self {
        Self {
            stream,
            lru_cache: LruCache::new(NonZeroUsize::new(5).unwrap())
        }
    }

    pub unsafe fn start(&mut self) -> Result<(), anyhow::Error>{
        loop {
            let job = self.recv_job()?;
            let mut now = Instant::now();
            let result = try {
                let func = self.get_func(job.job_id).map_err(|x| x.to_string())?;
                now = Instant::now();
                let result = catch_unwind(|| func(job.job_data, job.task_data))
                    .map_err(|x| "Failed to execute function".to_string())
                    .and_then(|x| x)?;
                result
            };
            let execution_time = now.elapsed();
            let executor_result = ExecutorResult {
                job_id: job.job_id,
                task_id: job.task_id,
                result,
                execution_time
            };

            self.send_result(executor_result)?
        }
    }
    pub fn send_result(&mut self, executor_result: ExecutorResult) -> Result<(), anyhow::Error>{
        let data = bincode::serde::encode_to_vec(executor_result, config::standard()).expect("Unable to serialize result");
        let len = (data.len() as u64).to_be_bytes();
        self.stream.write_all(&len)?;
        self.stream.write_all(&data)?;
        Ok(())
    }

    pub fn recv_job(&mut self) -> Result<ExecutorJob, anyhow::Error> {
        let mut len = [0;size_of::<u64>()];
        self.stream.read_exact(&mut len).context("Unable to read size-prefix")?;
        let mut vec = vec![0; u64::from_be_bytes(len) as usize];
        self.stream.read_exact(&mut vec).context("Unable to read data")?;
        bincode::serde::decode_from_slice(&vec, bincode::config::standard())
            .context("Unable to deserialize Job")
            .map(|x| x.0)
    }

    pub unsafe fn get_func(&mut self, uuid: Uuid) -> Result<Symbol<ExecutorFn>, anyhow::Error> {
        let library = self.get_library(uuid)?;
        library.get::<ExecutorFn>(b"execute_job\0")
            .context("Unable to find executor function")
    }

    pub unsafe fn get_library(&mut self, uuid: Uuid) -> Result<&Library, anyhow::Error>{
        self.lru_cache.try_get_or_insert(uuid, || {
            Library::new(Self::library_path(uuid))
                .context("Unable to open library")
        })
    }

    pub fn library_path(uuid: Uuid) -> PathBuf{
        let uuid = uuid.to_string();

        [OsStr::new("library"),&library_filename(uuid)].iter().collect()
    }
}








