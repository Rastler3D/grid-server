use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::{Pin, pin};
use std::sync::Arc;
use std::task::{Context, Poll, ready};
use futures_util::{SinkExt, StreamExt};
use quinn::{Endpoint, RecvStream, SendStream};
use shared::scheduler::{SchedulerRequest, SchedulerResponse};
use shared::utils::bin_codec::BinCodec;
use shared::utils::channel::stream_channel;
use slab::Slab;
use tokio::sync::futures::Notified;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::{Mutex, Notify};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::codec::{FramedRead, FramedWrite};
use tokio_util::sync::{CancellationToken, PollSender};
use crate::connection::accept_incoming;

#[derive(Debug)]
pub enum TaskManagerEvent{
    Connected(SocketAddr),
    Disconnect,
    Message(SchedulerRequest)
}

pub struct TaskManagerConnection {
    pub connected: Option<SocketAddr>,
    pub sink: PollSender<SchedulerResponse>,
    pub stream: ReceiverStream<TaskManagerEvent>,
    pub cancellation_token: CancellationToken
}

impl TaskManagerConnection {

    pub fn start_accepting(endpoint: Endpoint) -> Self{
        let (stream, request_sink,) = stream_channel(1024);
        let (response_stream, sink,) = stream_channel(1024);
        let lock = Arc::new(Mutex::new((response_stream,request_sink)));
        let cancellation_token = CancellationToken::new();
        tokio::spawn(accept_incoming(endpoint.clone(), cancellation_token.clone(), lock));

        Self{
            connected: None,
            stream,
            sink,
            cancellation_token,
        }
    }
    pub async fn send(&mut self, request: SchedulerResponse){
        self.sink.send(request).await;
    }

    pub async fn recv(&mut self) -> Option<TaskManagerEvent>{
        self.stream.next().await
    }
}


// pub struct TaskManagersQueue{
//     notify: Notify,
//     sender: Sender<TaskManagerConnection>,
//     receiver: Mutex<Receiver<TaskManagerConnection>>
// }
//
// impl TaskManagersQueue{
//     pub fn new() -> Self{
//         let (sender,receiver) = channel(1);
//         Self{
//             receiver: Mutex::new(receiver),
//             notify: Notify::new(),
//             sender,
//         }
//     }
//     pub fn seize(&self) -> Seize{
//         let mut notified = self.notify.notified();
//
//         let pin = Pin{
//             pointer: &mut notified
//         }.enable();
//
//         Seize{
//             notified
//         }
//     }
//
//     pub async fn push(&mut self, _: Token, task_manager: TaskManagerConnection){
//         self.sender.send(task_manager).await;
//     }
//
//     pub async fn pop(&mut self) -> Option<TaskManagerConnection> {
//         self.notify.notify_one();
//         self.receiver.lock().await.recv()
//     }
// }
//
//
// pub struct Seize<'a>{
//     notified: Notified<'a>
// }
//
// impl Future for Seize{
//     type Output = Token;
//
//     fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//         ready!(Pin::new(&mut self.notified).poll(cx));
//
//         Poll::Ready(Token(()))
//     }
// }
// pub struct Token(());