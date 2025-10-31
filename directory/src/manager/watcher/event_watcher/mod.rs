mod builder;
mod rename_control;

use super::super::Change;
pub use builder::*;
use notify::{
    INotifyWatcher, RecursiveMode, Watcher,
    event::{CreateKind, ModifyKind, RenameMode},
};
pub use rename_control::*;
use std::{marker::PhantomData, path::{Path, PathBuf}};
use tokio::sync::mpsc::unbounded_channel;

use crate::{bucket::{Bucket, key::Key, object::Object}, manager::{
    utils::{AsyncRecv, OneshotSender},
    watcher::{WatcherOwn, error::WatcherErr},
}};

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

                    if let Some(path) = path.parent().and_then(|x| x.to_str()).filter(|x| root.eq(*x)).and_then(|x| x.strip_prefix(root)) {
                        let bucket = Bucket::new_unchk_from_path(path);
                        if let Err(err) = tx.send(Change::NewBucket { bucket }) {
                            tracing::error!("New directory nofity error: {err}");
                        }
                    } else {
                        let tmp = path.strip_prefix(root).ok().and_then(|x| x.to_str()).unwrap();
                        let mut iter = tmp.split('/');
                        let bucket = iter.nth(0);
                        let key = iter.collect::<Vec<&str>>().join("/");

                        let bucket = Bucket::new_unchk(bucket.unwrap().to_string());

                        if let Err(err) = tx.send(Change::NewKey { bucket, key: Key::new(key) } ) {
                            tracing::error!("New directory nofity error: {err}");
                        }
                    }
                }
                notify::EventKind::Create(action) => {
                    tracing::trace!("Event: {event:?}");
                    tracing::trace!("Object Type: {action:?}");
                    let mut path = event.paths;
                    let path = path.pop().unwrap();

                    if path.parent().is_some_and(|x| x == Path::new(root)) {
                        tracing::error!("Objects aren't allowed in the root path");
                        continue;
                    }
                    let object = Object::from(&path);
                    let mut iter = path.strip_prefix(root).ok().and_then(|x| x.to_str()).unwrap().split("/");
                    let bucket = Bucket::new_unchk(iter.next().unwrap());
                    let key = Key::new(iter.collect::<Vec<_>>().join("/"));

                    if let Err(err) = tx.send(Change::NewObject { bucket, key, object } ) {
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

                    if from.is_dir()  {
                        if from.parent().is_some_and(|x| x == Path::new(root)) {
                            let from = Bucket::new_unchk_from_path(from.file_name().unwrap());
                            let to = Bucket::new_unchk_from_path(to.file_name().unwrap());
                            if let Err(er) = tx.send(Change::NameBucket { from , to }) {
                                tracing::error!("{er}");
                            }
                        } else {
                            let bucket = Bucket::new_unchk(to.strip_prefix(root).ok().and_then(|x| x.to_str()).and_then(|x| x.split("/").nth(0)).unwrap());
                            let from = Key::new(from.file_name().and_then(|x| x.to_str()).unwrap());
                            let to = Key::new(to.file_name().and_then(|x| x.to_str()).unwrap());
                            if let Err(er) = tx.send(Change::NameKey { bucket, from, to }) {
                                tracing::error!("{er}");
                            }
                        }
                    } else  {
                        let Some(parent) = to.parent().filter(|x| *x != Path::new(root)).and_then(|x| x.to_str()) else {
                            tracing::error!("Object aren't allowed in the root path");
                            continue;
                        };
                        let mut iter = parent.strip_prefix(root).map(|x| x.split("/")).unwrap();
                        let bucket = Bucket::new_unchk(iter.nth(0).unwrap());
                        let key = Key::new(iter.collect::<Vec<_>>().join("/"));
                        let from = Object::from(&from);
                        let to = Object::from(&to);

                        if let Err(er) = tx.send(Change::NameObject { bucket, key , from, to }) {
                            tracing::error!("{er}");
                        }
                    }
                }
                notify::EventKind::Remove(_) => { todo!() }
                _ => {}
            }
        }
    }

    fn tx(&self) -> TxInner {
        self.tx.clone()
    }
}
