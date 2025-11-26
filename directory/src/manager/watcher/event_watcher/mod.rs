mod builder;
mod rename_control;

use super::super::Change;
pub use builder::*;
use notify::{
    INotifyWatcher, RecursiveMode, Watcher,
    event::{CreateKind, ModifyKind, RenameMode},
};
pub use rename_control::*;
use std::{collections::HashMap, path::{Path, PathBuf}, sync::Arc};
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    bucket::{
        Bucket,
        key::Key,
        object::Object, utils::{FileNameUtf8, Renamed, find_bucket},
    },
    manager::{
        utils::{AsyncRecv, OneshotSender, SplitTask, Task},
        watcher::{NotifyChType, error::WatcherErr},
    }, state::local_storage::LocalStorage,
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
    obj_ls: Arc<LocalStorage>,
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
        let mut rename_tracking = HashMap::new();
        let mut rename_skip = Vec::new();
        while let Some(Ok(event)) = self.rx.recv().await {
            match event.kind {
                notify::EventKind::Create(CreateKind::Folder) => {
                    let mut paths = event.paths;
                    let Some(path) = paths.pop() else {
                        continue;
                    };

                    let Some(file_name) = FileNameUtf8::run(&path).await.ok() else {
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

                    let Some(path) = path.pop() else {
                        tracing::error!("[Event Watcher] {{ Create file skip }} Path is not present in action.path");
                        continue;
                    };

                    if path.parent().is_some_and(|x| x == root) {
                        tracing::error!("[Event Watcher] Objects aren't allowed in the root path");
                        continue;
                    }

                    let bucket = find_bucket(root, &path).unwrap();
                    tracing::trace!("[Event Watcher] bucket: {bucket}");

                    let key = Key::from_bucket(&bucket, &path).unwrap();
                    tracing::trace!("[Event Watcher] key: {key:?}");

                    let object = Object::new(&path).await;

                    tracing::trace!("[Event Watcher] object: {object:?}");
                    let file_name = object.file_name.clone();

                    if let Err(er) = self.change_notify.send(Change::NewObject { bucket, key, object }) {
                        tracing::error!("[Event Watcher] {{ Modify Name Object }} {er}");
                        continue;
                    }

                    rename_skip.push(file_name);
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

                    let Some(mut to) = paths.pop() else {
                        continue;
                    };

                    let Some(mut from) = paths.pop() else {
                        continue;
                    };

                    if let Err(err) = tx_rename.send(Rename::Decline(from.clone())) {
                        tracing::error!("{err}");
                    }

                    let file_name = match FileNameUtf8::run(&to).await {
                        Renamed::Yes(name) => {
                            tracing::debug!("[Event Watcher] {{ Rename tracking }} new name: {name:?} - from: {from:?}");
                            rename_tracking.insert(name, from);
                            continue;
                        },
                        Renamed::Not(name) => {
                            if let Some(from_) = rename_tracking.remove(&name) {
                                from = from_;
                            } else if rename_skip.pop_if(|x| *x == name).is_some() {
                                continue;
                            }
                            name
                        },
                        Renamed::Fail(error) => {
                            tracing::error!("{error}");
                            continue;
                        },
                    };

                    if to.is_dir() {
                        if from.parent().is_some_and(|x| x == root) {
                            let from = Bucket::new_unchecked(file_name);
                            let to = Bucket::new(&to);

                            if let Err(er) =
                                self.change_notify.send(Change::NameBucket { from, to })
                            {
                                tracing::error!(
                                    "[Event Watcher] {{ Modify Name Bucket }} {er}"
                                );
                            }
                        } else {
                            let Some(bucket) = find_bucket(root, &from) else { continue; };

                            if let Some(_from) = Key::from_bucket(&bucket, &from) && let Some(to) = Key::from_bucket(&bucket, &to) && let Err(er) =
                                    self.change_notify
                                        .send(Change::NameKey { bucket, from: _from, to })
                            {
                                tracing::error!("[Event Watcher] {{ Sender error }} {er} - {:?}", from);
                            } else {
                                tracing::error!("[Event Watcher] {{ Fail to obtain the key from: {from:?} - to: {to:?} }}");
                            }
                        }
                    } else {
                        let Some(bucket) = find_bucket(root, &from) else { 
                            tracing::error!("[Event Watcher] {{ find_bucket }} Bucket {from:?} not found");
                            continue; 
                        };

                        let key = Key::from_bucket(&bucket, from.parent().unwrap()).unwrap();
                        to.pop();
                        to.push(&file_name);

                        let Some(object) = self.obj_ls.get_object(&key, from.file_name().and_then(|x| x.to_str()).unwrap()).await else { 
                            tracing::error!("[Event Watcher] {{ LocalStorage }} object {file_name:?} not found");
                            todo!()
                        };
                        tracing::error!("{:#?}", object);
                        let mut new_obj = Object::new_from_object(&to, &object).await;

                        new_obj.name = file_name;
                        tracing::error!("{to:?} - {from:?}");

                        if let Err(er) = tokio::fs::rename(to, from).await {
                            tracing::error!("[Event Watcher] {{ Restore Name Error }} {er}");
                            continue;
                        }

                        rename_skip.push(new_obj.file_name.clone());

                        if let Err(er) = self.change_notify.send(Change::NameObject {
                            bucket,
                            key,
                            from: object.name,
                            to: new_obj,
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
