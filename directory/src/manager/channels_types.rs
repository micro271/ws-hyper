use tokio::sync::mpsc::{Receiver, UnboundedReceiver, UnboundedSender, error::SendError};

use crate::manager::utils::{AsyncRecv, OneshotSender};

impl<T: Send + Sync> AsyncRecv for Receiver<T> {
    type Item = T;

    fn recv(&mut self) -> impl Future<Output = Option<T>> {
        self.recv()
    }
}

impl<T: Send + 'static> OneshotSender for UnboundedSender<T> {
    type Item = T;
    type Output = Result<(), SendError<T>>;

    fn send(&self, item: Self::Item) -> Self::Output {
        Self::send(self, item)
    }
}

impl<T: Send + Sync> AsyncRecv for UnboundedReceiver<T> {
    type Item = T;

    fn recv(&mut self) -> impl Future<Output = Option<T>> {
        self.recv()
    }
}
