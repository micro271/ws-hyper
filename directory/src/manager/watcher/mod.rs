pub mod error;
pub mod event_watcher;
mod for_dir;
pub mod parser;
pub mod pool_watcher;
use std::marker::PhantomData;

use tokio::sync::mpsc::UnboundedSender;

use crate::manager::utils::{Executing, OneshotSender, TakeOwn, Task};

pub trait WatcherOwn<T, R, SI, SO>: Send + Sync
where
    Self: Sized,
    T: OneshotSender<Item = SI, Output = SO>,
    R: Send + 'static,
    SI: Send + 'static,
    SO: Send + 'static,
{
    fn run(self, tx: T);
    fn task(self, tx: T) -> impl std::future::Future<Output = ()>;
    fn tx(&self) -> UnboundedSender<R>;
}

#[derive(Debug, Clone)]
pub struct Watcher<S, W, T, R, SI, SO> {
    tx: UnboundedSender<R>,
    watcher: S,
    _phantom: PhantomData<(T, W, SI, SO)>,
}

impl<S, W, T, R, SI, SO> Watcher<S, W, T, R, SI, SO>
where
    W: WatcherOwn<T, R, SI, SO>,
    R: Send + 'static,
    T: OneshotSender<Item = SI, Output = SO>,
    S: Send + 'static,
    SI: Send + 'static,
    SO: Send + 'static,
{
    fn _tx(&self) -> UnboundedSender<R> {
        self.tx.clone()
    }
}

impl<W, T, R, SI, SO> Watcher<Task<W>, W, T, R, SI, SO>
where
    W: WatcherOwn<T, R, SI, SO> + 'static,
    T: OneshotSender<Item = SI, Output = SO>,
    SI: Send + 'static,
    SO: Send + 'static,
    R: Send + 'static,
{
    pub fn new(watcher: W) -> Self {
        let tx = watcher.tx();
        Self {
            tx,
            watcher: Task::new(watcher),
            _phantom: PhantomData,
        }
    }

    pub fn task(self) -> (Watcher<Executing, W, T, R, SI, SO>, W) {
        (
            Watcher {
                tx: self.tx,
                watcher: Executing,
                _phantom: self._phantom,
            },
            self.watcher.take(),
        )
    }
}
