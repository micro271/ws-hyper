pub mod error;
pub mod event_watcher;
pub mod pool_watcher;

use std::marker::PhantomData;

use error::WatcherErr;

use tokio::sync::mpsc::UnboundedSender;

pub trait WatcherOwn<T: Send,R: Send>: Send + Sync {
    fn run(self, tx: UnboundedSender<T>);
    fn task(self, tx: UnboundedSender<T>) -> impl std::future::Future<Output = ()>;
    fn get_send(&self) -> UnboundedSender<R>;
}

#[derive(Debug)]
pub struct Watcher<W, T, R> {
    tx: UnboundedSender<R>,
    state: WatcherState<W>,
    _phantom:PhantomData<T>
}

impl<W, T, R> Watcher<W, T, R>
where
    W: WatcherOwn<T, R>,
    R: Send,
    T: Send,
{
    pub fn new(watcher: W) -> Self {
        let tx = watcher.get_send();

        Self {
            tx,
            state: WatcherState::Pending(watcher),
            _phantom: PhantomData,
        }
    }

    pub fn get_sender(&self) -> UnboundedSender<R> {
        self.tx.clone()
    }

    pub fn task(&mut self) -> Result<W, WatcherErr> {
        if let WatcherState::Pending(task) =
            std::mem::replace(&mut self.state, WatcherState::Executing)
        {
            Ok(task)
        } else {
            Err(WatcherErr::new("The task already was executed"))
        }
    }
}

#[derive(Debug)]
enum WatcherState<W> {
    Pending(W),
    Executing,
}
