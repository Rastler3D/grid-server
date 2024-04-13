use shared::platform::Platform;
use shared::worker::{WorkerError, WorkerRequest, WorkerResponse};

pub trait Worker{
    const PLATFORM: Platform;
    async fn process(&mut self, request: WorkerRequest) -> Result<(),WorkerError>;
    async fn recv(&mut self) -> WorkerResponse;
    async fn reset(&mut self);
}