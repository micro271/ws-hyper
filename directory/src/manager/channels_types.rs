use tokio::sync::mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender, error::SendError};

use crate::manager::utils::{AsyncRecv, AsyncSender, OneshotSender};

impl<T: Send + Sync> AsyncRecv for Receiver<T> {
    type Item = T;

    fn recv(&mut self) -> impl Future<Output = Option<T>> {
        self.recv()
    }
}

impl<T: Send + 'static> OneshotSender for UnboundedSender<T> {
    type Item = T;

    fn send(&self, item: Self::Item) -> Result<(), SendError<T>> {
        Self::send(self, item)
    }
}

impl<T: Send + 'static> AsyncSender for Sender<T> {
    type Item = T;

    fn send(&mut self, item: Self::Item) -> impl Future<Output = Result<(), SendError<T>>> {
        Sender::send(self, item)
    }
}

impl<T: Send + Sync> AsyncRecv for UnboundedReceiver<T> {
    type Item = T;

    fn recv(&mut self) -> impl Future<Output = Option<T>> {
        self.recv()
    }
}
