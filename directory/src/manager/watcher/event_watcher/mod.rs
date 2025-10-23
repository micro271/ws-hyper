mod rename_control;
mod builder;

use std::path::PathBuf;
use notify::{
    INotifyWatcher, RecursiveMode, Watcher,
    event::{CreateKind, ModifyKind, RenameMode},
};
use tokio::sync::
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
pub use rename_control::*;
use super::super::Change;
pub use builder::*;

use crate::{
    directory::file::File,
    manager::{utils::ForDir, watcher::{error::WatcherErr, WatcherOwn}},
};

pub struct EventWatcher {
    rename_control: RenameControl,
    _notify_watcher: INotifyWatcher,
    tx: UnboundedSender<Result<notify::Event, notify::Error>>,
    rx: UnboundedReceiver<Result<notify::Event, notify::Error>>,
    for_dir: ForDir<String>,
}

impl WatcherOwn<Change, Result<notify::Event, notify::Error>> for EventWatcher {
    fn run(self, tx: UnboundedSender<Change>) {
        tokio::task::spawn(self.task(tx));
    }

    async fn task(mut self, tx: UnboundedSender<Change>) {
        tracing::debug!("Watcher notify manage init");
        let for_dir = self.for_dir;
        let tx_rename = self.rename_control.sender();
        while let Some(Ok(event)) = self.rx.recv().await {
            match event.kind {
                notify::EventKind::Create(CreateKind::Folder) => {
                    tracing::trace!("{event:?}");
                    let mut path = event.paths;
                    let path = path.pop().unwrap();


                    if let Err(err) = tx.send(Change::New {
                        dir: for_dir.get().directory(&path),
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
                    if let Err(err) = tx.send(Change::New {
                        dir: for_dir.get().directory(&path),
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
                    let path = to.parent().unwrap();

                    let from_file_name = from.file_name().and_then(|x| x.to_str()).unwrap();
                    if let Err(err) = tx.send(Change::Name {
                        dir: for_dir.get().directory(path),
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
                    
                    let parent = path.parent().unwrap();
                    let parent = for_dir.get().directory(parent);
                    tracing::trace!("[REMOVE] Directory: {parent:?}, file name: {file_name}");
                    if let Err(e) = tx.send(Change::Delete { parent, file_name }) {
                        tracing::error!("{e}");
                    }
                }
                _ => {}
            }
        }
    }

    fn tx(&self) -> UnboundedSender<Result<notify::Event, notify::Error>> {
        self.tx.clone()
    }
}
