use std::marker::Tuple;
use tokio::sync::{oneshot, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::PollSender;
use tokio::sync::mpsc::error::{SendError, TryRecvError};
use tokio::sync::mpsc::{channel, Receiver, Sender};

pub fn stream_channel<T: Send>(buffer: usize) -> (ReceiverStream<T>, PollSender<T>) {
    let (sender,receiver) = mpsc::channel(buffer);
    (ReceiverStream::new(receiver), PollSender::new(sender))
}

pub fn return_channel<T>() -> (oneshot::Receiver<T>, Return<T>) {
    let (sender, receiver) = oneshot::channel();
    (receiver, Return { sender })
}

#[derive(Debug)]
pub struct Return<T> {
    sender: oneshot::Sender<T>,
}

impl<T> FnOnce<(T,)> for Return<T> {
    type Output = ();

    extern "rust-call" fn call_once(self, return_value: (T,)) -> Self::Output {
        self.sender.send(return_value.0).unwrap_or_default()
    }
}

impl<T: Tuple> FnOnce<T> for Return<T> {
    type Output = ();

    extern "rust-call" fn call_once(self, return_value: T) -> Self::Output {
        self.sender.send(return_value).unwrap_or_default()
    }
}


pub struct DuplexChannel<IN, OUT> {
    sender: Sender<OUT>,
    receiver: Receiver<IN>,
}

impl<IN, OUT> DuplexChannel<IN, OUT> {

    fn new(sender: Sender<OUT>, receiver: Receiver<IN>) -> DuplexChannel<IN,OUT>{
        DuplexChannel{
            sender,
            receiver
        }
    }
    pub async fn send_async(&self, item: OUT) -> Result<(), SendError<OUT>> {
        self.sender.send(item).await
    }

    pub fn send(&self, item: OUT) -> Result<(), SendError<OUT>> {
        self.sender.blocking_send(item)
    }

    pub fn recv(&mut self) -> Option<IN> {
        self.receiver.blocking_recv()
    }

    pub async fn recv_async(&mut self) -> Option<IN> {
        self.receiver.recv().await
    }

    pub fn try_recv(&mut self) -> Result<IN, TryRecvError> {
        self.receiver.try_recv()
    }
}

pub fn duplex_channel<IN, OUT>(buffer: usize) -> (DuplexChannel<IN, OUT>, DuplexChannel<OUT, IN>) {
    let (tx1, rx1) = channel(buffer);
    let (tx2, rx2) = channel(buffer);

    ( DuplexChannel::new(tx1,rx2), DuplexChannel::new(tx2,rx1) )
}