use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use notify::{Event, event::RemoveKind};
use tokio::sync::{
    Mutex,
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};

use crate::manager::utils::{SplitTask, Task};

pub struct RenameControlCh(UnboundedSender<Rename>);

impl RenameControlCh {
    pub fn inner(self) -> UnboundedSender<Rename> {
        self.0
    }
}

#[derive(Debug)]
pub struct RenameControl {
    tx: UnboundedSender<Rename>,
    rx: UnboundedReceiver<Rename>,
    r#await: u64,
    sender_watcher: UnboundedSender<Result<notify::Event, notify::Error>>,
}

impl RenameControl {
    pub fn new(
        sender_watcher: UnboundedSender<Result<notify::Event, notify::Error>>,
        r#await: u64,
    ) -> Self {
        let (tx, rx) = unbounded_channel();

        Self {
            tx,
            rx,
            sender_watcher,
            r#await,
        }
    }

    pub fn sender(&self) -> UnboundedSender<Rename> {
        self.tx.clone()
    }
}

impl SplitTask for RenameControl {
    type Output = RenameControlCh;
    fn split(self) -> (<Self as SplitTask>::Output, impl crate::manager::utils::Run) {
        (RenameControlCh(self.tx.clone()), self)
    }
}

impl Task for RenameControl {
    async fn task(self)
    where
        Self: Sized,
    {
        let RenameControl {
            tx: _,
            mut rx,
            r#await,
            sender_watcher,
        } = self;

        let duration = Duration::from_millis(r#await);
        let files = Arc::new(Mutex::new(
            HashMap::<PathBuf, UnboundedSender<DropDelete>>::new(),
        ));

        loop {
            let files_inner = files.clone();
            match rx.recv().await {
                Some(Rename::From(RenameFrom(from))) => {
                    let (tx_inner, mut rx_inner) = unbounded_channel::<DropDelete>();
                    files_inner.lock().await.insert(from.clone(), tx_inner);
                    let sender_watcher = sender_watcher.clone();
                    tokio::spawn(async move {
                        tokio::select! {
                            () = tokio::time::sleep(duration) => {
                                if files_inner.lock().await.remove(&from).is_some() {
                                    tracing::trace!("[RenameControl] {{ Time expired }} Delete {from:?}");
                                    
                                    let event = Event::new(notify::EventKind::Remove(RemoveKind::Any)).add_path(from);
                                    if let Err(err) = sender_watcher.send(Ok(event)) {
                                        tracing::error!("[RenameControl] From tx_watcher {err}");
                                    }
                                }
                            }
                            resp = rx_inner.recv() => {
                                tracing::trace!("[RenameControl Inner task] Decline {from:?}");
                                if resp.is_none() {
                                    tracing::error!("tx_inner of the RenameControl closed");
                                }
                            }
                        };
                    });
                }
                Some(Rename::Decline(path)) => {
                    if let Some(sender) = files.lock().await.remove(&path) {
                        tracing::trace!("[RenameControl] Decline from Watcher, path: {path:?}");
                        if let Err(err) = sender.send(DropDelete) {
                            tracing::error!("{err}");
                        }
                    }
                }
                _ => {
                    tracing::error!("[RenameControl] Sender was close");
                    break;
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct DropDelete;

#[derive(Debug)]
pub struct RenameFrom(PathBuf);

impl RenameFrom {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }
}

pub enum Rename {
    From(RenameFrom),
    Decline(PathBuf),
}
