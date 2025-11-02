mod builder;
mod rename_control;

use super::super::Change;
pub use builder::*;
use notify::{
    INotifyWatcher, RecursiveMode, Watcher,
    event::{CreateKind, ModifyKind, RenameMode},
};
pub use rename_control::*;
use std::{marker::PhantomData, path::PathBuf};
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    bucket::{
        Bucket,
        key::Key,
        object::{Object, ObjectName},
    },
    manager::{
        utils::{AsyncRecv, OneshotSender},
        watcher::{WatcherOwn, error::WatcherErr},
    },
};

pub struct EventWatcher<Tx, TxInner, RxInner> {
    rename_control: RenameControl,
    _notify_watcher: INotifyWatcher,
    tx: TxInner,
    rx: RxInner,
    _pantom: PhantomData<Tx>,
    path: String,
}

impl<T, TxInner, RxInner> WatcherOwn<T, TxInner> for EventWatcher<T, TxInner, RxInner>
where
    T: OneshotSender<Item = Change> + Send + Clone + 'static,
    TxInner: OneshotSender<Item = Result<notify::Event, notify::Error>> + Send + 'static + Clone,
    RxInner: AsyncRecv<Item = Result<notify::Event, notify::Error>> + Send + 'static,
{
    fn run(self, tx: T) {
        tokio::task::spawn(self.task(tx));
    }

    async fn task(mut self, tx: T) {
        tracing::debug!("Watcher notify manage init");

        let root = &self.path[..];
        let tx_rename = self.rename_control.sender();
        while let Some(Ok(event)) = self.rx.recv().await {
            match event.kind {
                notify::EventKind::Create(CreateKind::Folder) => {
                    tracing::trace!("{event:?}");
                    let mut path = event.paths;
                    let path = path.pop().unwrap();

                    if path.parent().filter(|x| root == *x).is_some() {
                        let bucket = Bucket::new_unchk_from_path(path.file_name().unwrap());
                        if let Err(err) = tx.send(Change::NewBucket { bucket }) {
                            tracing::error!("New directory nofity error: {err}");
                        }
                    } else {
                        let tmp = path
                            .strip_prefix(root)
                            .ok()
                            .and_then(|x| x.to_str())
                            .unwrap();
                        let mut iter = tmp.split('/');
                        let bucket = iter.nth(0);
                        let key = iter.collect::<Vec<&str>>().join("/");

                        let bucket = Bucket::new_unchk(bucket.unwrap().to_string());

                        if let Err(err) = tx.send(Change::NewKey {
                            bucket,
                            key: Key::new(key),
                        }) {
                            tracing::error!("New directory nofity error: {err}");
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
                    let bucket = Bucket::new_unchk(iter.next().unwrap());
                    let key = Key::new(iter.collect::<Vec<_>>().join("/"));

                    if let Err(err) = tx.send(Change::NewObject {
                        bucket,
                        key,
                        object,
                    }) {
                        tracing::error!("New directory nofity error: {err}");
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
                            let from = Bucket::new_unchk_from_path(
                                from.file_name().unwrap().to_string_lossy().into_owned(),
                            );
                            let to = Bucket::new_unchk_from_path(
                                to.file_name().unwrap().to_string_lossy().into_owned(),
                            );
                            if let Err(er) = tx.send(Change::NameBucket { from, to }) {
                                tracing::error!("{er}");
                            }
                        } else {
                            let path = to
                                .strip_prefix(root)
                                .unwrap()
                                .to_string_lossy()
                                .into_owned();
                            let mut path = path.split("/");
                            let bucket = Bucket::new_unchk(path.next().unwrap());
                            let from = from.to_string_lossy().into_owned();
                            let bucket_ = format!("{}/", bucket.as_ref());
                            let mut from = from.split(&bucket_[..]);

                            let from = Key::new(from.nth(1).unwrap());
                            let to = to.to_string_lossy().into_owned();
                            let mut to = to.split(&bucket_[..]);
                            let to = Key::new(to.nth(1).unwrap());
                            if let Err(er) = tx.send(Change::NameKey { bucket, from, to }) {
                                tracing::error!("{er}");
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
                        let bucket = Bucket::new_unchk(iter.next().unwrap());
                        let key = Key::new(iter.collect::<Vec<_>>().join("/"));
                        if let Err(er) = tx.send(Change::NameObject {
                            bucket,
                            key,
                            from,
                            to,
                        }) {
                            tracing::error!("{er}");
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

    fn tx(&self) -> TxInner {
        self.tx.clone()
    }
}
