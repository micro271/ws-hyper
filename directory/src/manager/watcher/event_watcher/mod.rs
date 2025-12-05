mod builder;
mod rename_control;

use super::super::Change;
pub use builder::*;
use notify::{
    INotifyWatcher, RecursiveMode, Watcher,
    event::{CreateKind, ModifyKind, RenameMode},
};
pub use rename_control::*;
use std::path::PathBuf;
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    bucket::{Bucket, key::Key},
    manager::{
        utils::{
            AsyncRecv, OneshotSender, REGEX_OBJECT_NAME, SplitTask, Task, ToSkip,
            hd_new_bucket_or_key_watcher, hd_new_object_watcher, hd_rename_object, hd_rename_path,
        },
        watcher::{NotifyChType, error::WatcherErr},
    },
};

#[derive(Debug, Clone)]
pub struct EventWatcherCh<Tx>(Tx);

pub struct EventWatcher<Tx, Rx, TxChange> {
    _notify_watcher: INotifyWatcher,
    tx: Tx,
    rx: Rx,
    change_notify: TxChange,
    path: PathBuf,
    rename_control_sender: RenameControlCh,
    ignore_rename_prefix: String,
}

impl<Tx, Rx, TxChange> Task for EventWatcher<Tx, Rx, TxChange>
where
    TxChange: OneshotSender<Item = Change>,
    Tx: OneshotSender<Item = NotifyChType> + Clone + Send + 'static,
    Rx: AsyncRecv<Item = NotifyChType> + Send + 'static,
{
    async fn task(mut self) {
        tracing::warn!("Watcher notify manage init");

        let root = self.path.as_path();
        let tx_rename = self.rename_control_sender.inner();
        let mut rename_skip = ToSkip::default();
        while let Some(Ok(event)) = self.rx.recv().await {
            match event.kind {
                notify::EventKind::Create(CreateKind::Folder) => {
                    let mut paths = event.paths;

                    let Some(path) = paths.pop() else {
                        continue;
                    };

                    match hd_new_bucket_or_key_watcher(path, root, &mut rename_skip).await {
                        Ok(ch) => {
                            if let Err(err) = self.change_notify.send(ch) {
                                tracing::error!("[Event Wtcher] Sender error: {err}");
                            }
                        }
                        Err(()) => {
                            tracing::error!("[ CreateKinfFolder ] Error")
                        }
                    }
                }
                notify::EventKind::Create(action) => {
                    tracing::trace!("Event: {event:?}");
                    tracing::trace!("Action: {action:?}");
                    let mut path = event.paths;

                    let Some(path) = path.pop() else {
                        tracing::error!(
                            "[Event Watcher] {{ Create file skip }} Path is not present in action.path"
                        );
                        continue;
                    };

                    match hd_new_object_watcher(path, root, &self.ignore_rename_prefix).await {
                        Ok(ch) => {
                            if let Err(er) = self.change_notify.send(ch) {
                                tracing::error!("[Event Watcher] {{ Modify Name Object }} {er}");
                                continue;
                            }
                        }
                        Err(()) => {
                            tracing::error!("[ CreateKinfOther ] Error")
                        }
                    }
                }
                notify::EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                    let mut path = event.paths;
                    let path = path.pop().unwrap();

                    tracing::debug!(
                        "[Watcher] {{ ModifyKind::Name(RenameMode::From) }} {path:?} (Maybe Delete)"
                    );

                    if let Err(err) = tx_rename.send(Rename::From(RenameFrom::new(path))) {
                        tracing::error!("{err}");
                    }
                }
                notify::EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                    tracing::trace!("Modify both {:?}", event.paths);
                    let mut paths = event.paths;

                    let (Some(to), Some(from)) = (paths.pop(), paths.pop()) else {
                        continue;
                    };

                    if let Err(err) = tx_rename.send(Rename::Decline(from.clone())) {
                        tracing::error!("{err}");
                    }

                    let ch = if to.is_dir() {
                        hd_rename_path(root, from, to, &mut rename_skip).await
                    } else {
                        hd_rename_object(root, from, to).await
                    };

                    match ch {
                        Ok(ch) => {
                            if let Err(err) = self.change_notify.send(ch) {
                                tracing::error!("{err}");
                            }
                        }
                        Err(()) => {
                            tracing::error!("[ ModifyKind::Rename ] Error")
                        }
                    }
                }
                notify::EventKind::Remove(er) => {
                    tracing::trace!("{er:?}");
                    let mut path = event.paths;
                    let Some(path) = path.pop() else {
                        continue;
                    };

                    let Some(name) = path.file_name().and_then(|x| x.to_str()) else { continue ;};

                    let change = if REGEX_OBJECT_NAME.is_match(name)
                        && let Some(bucket) = Bucket::find_bucket(root, &path)
                        && let Some(key) = Key::from_bucket(&bucket, &path)
                    {
                        Change::DeleteObject { bucket, key, file_name: name.to_string() }
                    } else if let Some(path) = path.parent() && path == root {
                            let bucket = Bucket::from(name);
                            Change::DeleteBucket { bucket }
                        } else if let Some(bucket) = Bucket::find_bucket(root, &path) && let Some(key) = Key::from_bucket(&bucket, &path) {
                            Change::DeleteKey { bucket, key }
                        } else {
                            tracing::error!("[ Event Watcher ] {{ Error to delete path }} {path:?}");
                            continue;
                        };
                    
                    if let Err(er) = self.change_notify.send(change) {
                        tracing::error!("[ EventWatcher ] {{ Remove file }} Error: {er}");
                    }

                }
                _ => {}
            }
        }
    }
}

impl<Tx> std::ops::Deref for EventWatcherCh<Tx> {
    type Target = Tx;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Tx: Clone> EventWatcherCh<Tx> {
    fn new(tx: Tx) -> Self {
        Self(tx)
    }
}

impl<Tx, Rx, TxChange> SplitTask for EventWatcher<Tx, Rx, TxChange>
where
    TxChange: OneshotSender<Item = Change>,
    Tx: OneshotSender<Item = NotifyChType> + Clone + Send + 'static,
    Rx: AsyncRecv<Item = NotifyChType> + Send + 'static,
{
    type Output = EventWatcherCh<Tx>;

    fn split(self) -> (<Self as SplitTask>::Output, impl crate::manager::utils::Run) {
        (EventWatcherCh::new(self.tx.clone()), self)
    }
}
