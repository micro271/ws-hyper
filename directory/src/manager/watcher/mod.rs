pub mod error;
pub mod event_watcher;
mod for_dir;
pub mod pool_watcher;
use std::marker::PhantomData;

use crate::manager::utils::{Executing, OneshotSender, TakeOwn, Task};

pub trait WatcherOwn<T, TxInner>: Send + 'static
where
    Self: Sized,
    T: OneshotSender + Send,
    TxInner: OneshotSender + Clone + Send + 'static,
{
    fn run(self, tx: T);
    fn task(self, tx: T) -> impl std::future::Future<Output = ()>;
    fn tx(&self) -> TxInner;
}

#[derive(Debug, Clone)]
pub struct Watcher<S, W, Tx, TxInner> {
    tx: TxInner,
    watcher: S,
    _phantom: PhantomData<(W, Tx)>,
}

impl<S, W, Tx, TxInner> Watcher<S, W, Tx, TxInner>
where
    W: WatcherOwn<Tx, TxInner>,
    Tx: OneshotSender + Clone + Send,
    TxInner: OneshotSender + Clone + Send,
    S: Send + 'static,
{
    fn _tx(&self) -> TxInner {
        self.tx.clone()
    }
}

impl<W, T, TxInner> Watcher<Task<W>, W, T, TxInner>
where
    W: WatcherOwn<T, TxInner> + Send + 'static,
    T: OneshotSender + Send + Clone + 'static,
    TxInner: OneshotSender + Clone + Send + 'static,
{
    pub fn new(watcher: W) -> Self {
        let tx = watcher.tx();

        Self {
            tx,
            watcher: Task::new(watcher),
            _phantom: PhantomData,
        }
    }

    pub fn task(self) -> (Watcher<Executing, W, T, TxInner>, W) {
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
