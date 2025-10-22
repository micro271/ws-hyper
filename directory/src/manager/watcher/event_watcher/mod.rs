mod rename_control;

use std::{path::PathBuf, sync::Arc};

use notify::{
    INotifyWatcher, RecursiveMode, Watcher,
    event::{CreateKind, ModifyKind, RenameMode},
};
use tokio::sync::{
    RwLock,
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};

pub use rename_control::*;
use super::super::Change;

use crate::{
    directory::{Directory, WithPrefixRoot, file::File, tree_dir::TreeDir},
    manager::{
        watcher::{WatcherOwn, error::WatcherErr},
    },
};

#[derive(Debug, Default)]
pub struct EventWatcherBuilder {
    path: Option<PathBuf>,
    r#await: Option<u64>,
    state: Option<Arc<RwLock<TreeDir>>>,
}

impl EventWatcherBuilder {
    pub fn rename_control_await(mut self, r#await: u64) -> Self {
        self.r#await = Some(r#await);
        self
    }

    pub fn path(mut self, mut path: PathBuf) -> Result<Self, WatcherErr> {
        if path.is_relative() {
            path = path.canonicalize().map_err(|x| WatcherErr::new(x.to_string()))?;
        }

        self.path = Some(path);

        Ok(self)
    }

    pub fn state(mut self, state: Arc<RwLock<TreeDir>>) -> Self {
        self.state = Some(state);
        self
    }

    pub fn build(self) -> Result<EventWatcher, WatcherErr> {
        let Some(path) = self.path else {
            return Err(WatcherErr::new("Path not defined"));
        };
        let r#await = self.r#await.unwrap_or(2000);

        let Some(state) = self.state else {
            return Err(WatcherErr::new("State not defined"));
        };

        if !path.exists() {
            return Err(WatcherErr::new(format!("Path {path:?} not exists")));
        }

        let (tx, rx) = unbounded_channel();
        let tx_cp = tx.clone();
        let mut notify_watcher = notify::recommended_watcher(move |event| _ = tx_cp.send(event))
            .map_err(|x| WatcherErr::new(x.to_string()))?;

        notify_watcher
            .watch(&path, RecursiveMode::Recursive)
            .map_err(|x| WatcherErr::new(x.to_string()))?;

        let rename_control = RenameControl::new(tx.clone(), self.r#await.unwrap_or(r#await));
        let path = path.to_str().map(ToString::to_string).ok_or(WatcherErr::new(format!("Error to parse from {path:?} to String")))?;

        Ok(EventWatcher {
            _notify_watcher: notify_watcher,
            rename_control,
            tx,
            rx,
            state,
            path,
        })
    }
}

pub struct EventWatcher {
    rename_control: RenameControl,
    _notify_watcher: INotifyWatcher,
    tx: UnboundedSender<Result<notify::Event, notify::Error>>,
    rx: UnboundedReceiver<Result<notify::Event, notify::Error>>,
    state: Arc<RwLock<TreeDir>>,
    path: String,
}

impl WatcherOwn<Change, Result<notify::Event, notify::Error>> for EventWatcher {
    fn run(self, tx: UnboundedSender<Change>)
    where
        Self: 'static,
    {
        tokio::task::spawn(self.task(tx));
    }

    async fn task(mut self, tx: UnboundedSender<Change>) {
        tracing::debug!("Watcher notify manage init");
        let tx_rename = self.rename_control.sender();
        let real_path = self.path.as_ref();
        while let Some(Ok(event)) = self.rx.recv().await {
            match event.kind {
                notify::EventKind::Create(CreateKind::Folder) => {
                    tracing::trace!("{event:?}");
                    let mut path = event.paths;
                    let path = path.pop().unwrap();
                    let reader = self.state.read().await;
                    let dir = Directory::from(WithPrefixRoot::new(
                        path.parent().unwrap(),
                        real_path,
                        reader.root(),
                    ));

                    if let Err(err) = tx.send(Change::New {
                        dir,
                        file: File::from(&path),
                    }) {
                        tracing::error!("New directory nofity error: {err}");
                    }
                }
                notify::EventKind::Create(action) => {
                    tracing::trace!("Event: {event:?}");
                    tracing::trace!("File Type: {action:?}");
                    let mut path = event.paths;
                    let path = path.pop().unwrap();
                    let reader = self.state.read().await;
                    if let Err(err) = tx.send(Change::New {
                        dir: Directory::from(WithPrefixRoot::new(
                            path.parent().unwrap(),
                            real_path,
                            reader.root(),
                        )),
                        file: File::from(&path),
                    }) {
                        tracing::error!("New file nofity error: {err}");
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
                    let reader = self.state.read().await;
                    let path = to.parent().unwrap();
                    let dir = Directory::from(WithPrefixRoot::new(
                        path,
                        real_path,
                        reader.root(),
                    ));

                    let from_file_name = from.file_name().and_then(|x| x.to_str()).unwrap();
                    if let Err(err) = tx.send(Change::Name {
                        dir,
                        from: from_file_name.to_string(),
                        to: File::from(&to),
                    }) {
                        tracing::error!("tx_watcher error: {err}");
                    }
                }
                notify::EventKind::Remove(_) => {
                    let mut path = event.paths;
                    let path = path.pop().unwrap();
                    let file_name = path
                        .file_name()
                        .and_then(|x| x.to_str().map(ToString::to_string))
                        .unwrap();
                    let reader = self.state.read().await;
                    let parent = path.parent().unwrap();
                    let parent = Directory::from(WithPrefixRoot::new(
                        parent,
                        real_path,
                        reader.root(),
                    ));
                    tracing::trace!("[REMOVE] Directory: {parent:?}, file name: {file_name}");
                    if let Err(e) = tx.send(Change::Delete { parent, file_name }) {
                        tracing::error!("{e}");
                    }
                }
                _ => {}
            }
        }
    }

    fn get_send(&self) -> UnboundedSender<Result<notify::Event, notify::Error>> {
        self.tx.clone()
    }
}
