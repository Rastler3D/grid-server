use std::pin::{pin, Pin};
use std::task::{Context, Poll, ready};
use interprocess::local_socket::tokio::{OwnedReadHalf, OwnedWriteHalf};
use shared::executor::{ExecutorJob, ExecutorResult};
use shared::utils::bin_codec::BinCodec;
use tokio::process::{Child};
use tokio_stream::Stream;
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::compat::Compat;
use std::future::Future;
use futures_util::sink::SinkExt;
use tokio::io::AsyncReadExt;

pub struct DllExecutorStream {
    pub(crate) process: Child,
    pub(crate) stream: FramedRead<Compat<OwnedReadHalf>, BinCodec<ExecutorResult>>,
}

pub enum DllExecutorEvent{
    Terminated(String),
    Result(ExecutorResult)
}

impl Stream for DllExecutorStream {
    type Item = DllExecutorEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let wait = { pin!(self.process.wait()).poll(cx) };
        if let Poll::Ready(terminated) = wait  {
            let mut error = String::new();
            let read = self.process
                .stderr
                .as_mut()
                .unwrap()
                .read_to_string(&mut error);

            ready!(pin!(read).poll(cx));
            return Poll::Ready(Some(DllExecutorEvent::Terminated(error)))
        };
        match Pin::new(&mut self.stream).poll_next(cx) {
            Poll::Ready(res) => {
                Poll::Ready(res
                    .and_then(|x| x.ok())
                    .map(|result| DllExecutorEvent::Result(result))
                )
            },
            Poll::Pending => Poll::Pending
        }
    }
}

impl Drop for DllExecutorStream {
    fn drop(&mut self) {
        self.process.start_kill();
    }
}

pub struct DllExecutor {
    pub(crate) sink: FramedWrite<Compat<OwnedWriteHalf>, BinCodec<ExecutorJob>>
}

impl DllExecutor {
    pub async fn start_job(&mut self, job: ExecutorJob){
        self.sink.send(job).await.expect("start job failed")
    }
}