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
    bucket::{
        Bucket,
        key::Key,
        object::{Object, ObjectName}, utils::{NormalizeFileUtf8, find_bucket},
    },
    manager::{
        utils::{AsyncRecv, OneshotSender, SplitTask, Task},
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
        while let Some(Ok(event)) = self.rx.recv().await {
            match event.kind {
                notify::EventKind::Create(CreateKind::Folder) => {
                    let mut paths = event.paths;
                    let Some(path) = paths.pop() else {
                        continue;
                    };

                    let Some(file_name) = NormalizeFileUtf8::run(&path).await else {
                        continue;
                    };
                    
                    if path.parent().is_some_and(|x| x == path) {
                        let bucket = Bucket::from(file_name);
                        if let Err(err) = self.change_notify.send(Change::NewBucket { bucket }) {
                            tracing::error!(
                                "[Event Watcher] {{ New Bucket }} nofity error: {err} - path: {:?}",
                                path
                            );
                        }
                    } else {
                        if let Some(bucket) = find_bucket(root, &path) && let Some(key) = Key::from_bucket(&bucket, &path) {
                            if let Err(err) = self.change_notify.send(Change::NewKey { bucket, key  }) {
                                tracing::error!(
                                    "[Event Watcher] {{ New Key }} nofity error: {err} - path: {:?}",
                                    path
                                );
                            }
                        } else {
                            tracing::error!("Bucket not found {path:?}");
                        }
                        
                    }
                }
                notify::EventKind::Create(action) => {
                    tracing::trace!("Event: {event:?}");
                    tracing::trace!("Object Type: {action:?}");
                    let mut path = event.paths;
                    let path = path.pop().unwrap();

                    if path.parent().is_some_and(|x| x == root) {
                        tracing::error!("Objects aren't allowed in the root path");
                        continue;
                    }
                    let object = Object::from(&path);
                    let iter = path.parent().unwrap();
                    let iter = iter
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    let mut iter = iter.split("/");
                    let bucket = Bucket::from(iter.next().unwrap());
                    let key = Key::new(iter.collect::<Vec<_>>().join("/"));

                    if let Err(err) = self.change_notify.send(Change::NewObject {
                        bucket,
                        key,
                        object,
                    }) {
                        tracing::error!(
                            "[Event Watcher] {{ Create Object }} New directory nofity error: {err} - {:?}",
                            path
                        );
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
                    let to = paths.pop().unwrap();
                    let from = paths.pop().unwrap();

                    if let Err(err) = tx_rename.send(Rename::Decline(from.clone())) {
                        tracing::error!("{err}");
                    }

                    if to.is_dir() {
                        if from.parent().is_some_and(|x| x == root) {
                            let from = Bucket::new_or_rename(&from).await;
                            let to = Bucket::new_or_rename(&to).await;
                            if let Err(er) =
                                self.change_notify.send(Change::NameBucket { from, to })
                            {
                                tracing::error!(
                                    "[Event Watcher] {{ Modify Name Bucket }} {er}"
                                );
                            }
                        } else {
                            let path = to
                                .strip_prefix(root)
                                .unwrap()
                                .to_string_lossy()
                                .into_owned();
                            let mut path = path.split("/");
                            let bucket = Bucket::from(path.next().unwrap());
                            let from = from.to_string_lossy().into_owned();
                            let bucket_ = format!("{}/", bucket);
                            let mut from = from.split(&bucket_[..]);

                            let from = Key::new(from.nth(1).unwrap());
                            let to = to.to_string_lossy().into_owned();
                            let mut to = to.split(&bucket_[..]);
                            let to = Key::new(to.nth(1).unwrap());
                            if let Err(er) =
                                self.change_notify
                                    .send(Change::NameKey { bucket, from, to })
                            {
                                tracing::error!(
                                    "[Event Watcher] {{ Modify Name Key }} {er} - {:?}",
                                    path
                                );
                            }
                        }
                    } else {
                        let Some(parent) = to.parent().filter(|x| *x != root) else {
                            tracing::error!("Object aren't allowed in the root path");
                            continue;
                        };
                        let from = ObjectName::from(&from);
                        let to = Object::from(&to);
                        let iter = parent
                            .strip_prefix(root)
                            .map(|x| x.to_string_lossy().into_owned())
                            .unwrap();
                        let mut iter = iter.split("/");
                        let bucket = Bucket::from(iter.next().unwrap());
                        let key = Key::new(iter.collect::<Vec<_>>().join("/"));
                        if let Err(er) = self.change_notify.send(Change::NameObject {
                            bucket,
                            key,
                            from,
                            to,
                        }) {
                            tracing::error!("[Event Watcher] {{ Modify Name Object }} {er}");
                        }
                    }
                }
                notify::EventKind::Remove(_) => {
                    todo!()
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
