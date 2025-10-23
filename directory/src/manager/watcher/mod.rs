pub mod error;
pub mod event_watcher;
pub mod pool_watcher;
pub mod parser;
use std::marker::PhantomData;

use tokio::sync::mpsc::UnboundedSender;

pub trait WatcherOwn<T: Send,R: Send>: Send + Sync {
    fn run(self, tx: UnboundedSender<T>);
    fn task(self, tx: UnboundedSender<T>) -> impl std::future::Future<Output = ()>;
    fn tx(&self) -> UnboundedSender<R>;
}

#[derive(Debug, Clone)]
pub struct Watcher<S, W, T, R> {
    tx: UnboundedSender<R>,
    watcher: S,
    _phantom:PhantomData<(T, W)>,
}

impl<S, W, T, R> Watcher<S, W, T, R> 
where 
    W: WatcherOwn<T, R>,
    R: Send + Sync + 'static,
    T: Send + Sync + 'static,
    S: Send + Sync + 'static,
{
    fn tx(&self) -> UnboundedSender<R> {
        self.tx.clone()
    }
}

impl<W, T, R> Watcher<Task<W>, W, T, R>
where
    W: WatcherOwn<T, R> + Sync + 'static,
    R: Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    pub fn new(watcher: W) -> Self {
        let tx = watcher.tx();
        Self {
            tx,
            watcher: Task(watcher),
            _phantom: PhantomData,
        }
    }

    pub fn task(self) -> (Watcher<Executing, W, T, R>, W) {
        (Watcher {
            tx: self.tx,
            watcher: Executing,
            _phantom: self._phantom,
        }, self.watcher.take())
    }
}

#[derive(Debug, Clone)]
pub struct Task<W: Send>(W);

#[derive(Debug, Clone)]
pub struct Executing;

impl<W: Send> Task<W> {
    fn take(self) -> W {
        self.0
    }
}