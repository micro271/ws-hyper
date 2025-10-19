pub mod new_file_tba;
pub mod utils;

use futures::{SinkExt, stream::SplitSink};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{WebSocketStream, tungstenite::Message};
use hyper_util::rt::TokioIo;
use notify::{
    Event, Watcher,
    event::{CreateKind, ModifyKind, RemoveKind, RenameMode},
};
use utils::validate_name_and_replace;
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
    vec,
};

use serde_json::json;
use tokio::{
    sync::{
        Mutex, RwLock,
        broadcast::{self, Receiver as ReceivedBr, Sender as SenderBr},
        mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender, channel, unbounded_channel},
    },
};

use crate::directory::{Directory, WithPrefixRoot, file::File, tree_dir::TreeDir};

type WsSenderType = SplitSink<WebSocketStream<TokioIo<Upgraded>>, Message>;

#[derive(Debug)]
pub struct Schedule {
    tx_ws: Sender<MsgWs>,
    pub state: Arc<RwLock<TreeDir>>,
    rename_control: RenameControl,
}

impl Schedule {
    pub fn new(state: Arc<RwLock<TreeDir>>) -> Arc<Self> {
        let (tx_ws, rx_ws) = channel(256);
        let (tx_sch, rx_sch) = unbounded_channel::<Change>();
        let (tx_watcher, rx_watcher) = unbounded_channel();
        let myself = Arc::new(Self {
            tx_ws,
            state,
            rename_control: RenameControl::new(tx_watcher.clone(), 2000),
        });

        tokio::task::spawn(
            myself
                .clone()
                .run_watcher_mg(tx_sch, tx_watcher, rx_watcher),
        );
        tokio::task::spawn(myself.clone().run_websocker_mg(rx_ws));
        tokio::task::spawn(myself.clone().run_scheduler_mg(rx_sch));

        myself
    }

    async fn run_websocker_mg(self: Arc<Self>, mut rx_ws: Receiver<MsgWs>) {
        let mut users = HashMap::<String, SenderBr<Change>>::new();
        tracing::debug!("Web socket manage init");
        loop {
            let msg = rx_ws.recv().await;
            tracing::trace!("{msg:?}");
            match msg {
                Some(MsgWs::Change { subscriber, change }) => {
                    if let Some(send) = users.get(&subscriber)
                        && let Err(err) = send.send(change)
                    {
                        tracing::error!("{err}");
                        _ = users.remove(&subscriber);
                    }
                }
                Some(MsgWs::NewUser {
                    subscriber,
                    mut sender,
                }) => {
                    let mut rx = if let Some(subs) = users.get(&subscriber) {
                        subs.subscribe()
                    } else {
                        let (tx, rx) = broadcast::channel(256);
                        users.insert(subscriber, tx);
                        rx
                    };
                    tokio::spawn(async move {
                        while let Ok(change) = rx.recv().await {
                            if let Err(err) = sender
                                .send(Message::Text(json!(change).to_string().into()))
                                .await
                            {
                                tracing::error!("{err}");
                            }
                        }
                    });
                }
                None => {
                    tracing::debug!("Peer tx_ws closed");
                    break;
                }
            }
        }
    }

    async fn run_watcher_mg(
        self: Arc<Self>,
        tx_sch: UnboundedSender<Change>,
        tx_watcher: UnboundedSender<Result<notify::Event, notify::Error>>,
        mut rx_watcher: UnboundedReceiver<Result<notify::Event, notify::Error>>,
    ) {
        tracing::debug!("Watcher notify manage init");
        let real_path = self.state.read().await.real_path().to_string();

        let mut watcher = notify::recommended_watcher(move |x| {
            _ = tx_watcher.send(x);
        })
        .unwrap();
        watcher
            .watch(Path::new(&real_path), notify::RecursiveMode::Recursive)
            .unwrap();
        let tx_rename = self.rename_control.sender();

        loop {
            while let Some(Ok(event)) = rx_watcher.recv().await {
                match event.kind {
                    notify::EventKind::Create(CreateKind::Folder) => {
                        tracing::trace!("{event:?}");
                        let rd = self.state.read().await;
                        let mut path = event.paths;
                        let path = path.pop().unwrap();

                        let dir = Directory::from(WithPrefixRoot::new(
                            path.parent().unwrap(),
                            rd.real_path(),
                            rd.root(),
                        ));

                        if let Err(err) = tx_sch.send(Change::New {
                            dir,
                            file: File::from(&path),
                        }) {
                            tracing::error!("New directory nofity error: {err}");
                        }
                    }
                    notify::EventKind::Create(action) => {
                        tracing::trace!("Event: {event:?}");
                        tracing::trace!("File Type: {action:?}");

                        let reader = self.state.read().await;
                        let mut path = event.paths;
                        let path = path.pop().unwrap();

                        if let Err(err) = tx_sch.send(Change::New {
                            dir: Directory::from(WithPrefixRoot::new(
                                path.parent().unwrap(),
                                reader.real_path(),
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
                        if let Err(err) = tx_rename.send(Rename::From(RenameFrom(path))) {
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
                            reader.real_path(),
                            reader.root(),
                        ));

                        let from_file_name = from.file_name().and_then(|x| x.to_str()).unwrap();
                        if let Err(err) = tx_sch.send(Change::Name {
                            dir,
                            from: from_file_name.to_string(),
                            to: File::from(&to),
                        }) {
                            tracing::error!("tx_watcher error: {err}");
                        }
                    }
                    notify::EventKind::Remove(_) => {
                        let mut path = event.paths;
                        let reader = self.state.read().await;
                        let path = path.pop().unwrap();
                        let file_name = path
                            .file_name()
                            .and_then(|x| x.to_str().map(ToString::to_string))
                            .unwrap();
                        let parent = path.parent().unwrap();
                        let parent = Directory::from(WithPrefixRoot::new(
                            parent,
                            reader.real_path(),
                            reader.root(),
                        ));
                        tracing::trace!("[REMOVE] Directory: {parent:?}, file name: {file_name}");
                        if let Err(e) = tx_sch.send(Change::Delete { parent, file_name }) {
                            tracing::error!("{e}");
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    async fn run_scheduler_mg(self: Arc<Self>, mut rx_watcher: UnboundedReceiver<Change>) {
        tracing::debug!("Scheduler init");
        let tx_ws = self.tx_ws.clone();
        loop {
            match rx_watcher.recv().await {
                Some(Change::New { dir, file }) => {
                    tracing::trace!("[Scheduler] Input dir: {dir:?} - file: {file}");
                    let mut wr = self.state.write().await;
                    let path = dir.as_ref().to_string();
                    let file_name = file.file_name().to_string();
                    if file.is_dir() {
                        let mut path = dir.path();
                        path.push(file.file_name());
                        let dir = Directory::new_unchk_from_path(path);
                        tracing::trace!("[Watch Manager]: New dir {dir:?}");
                        wr.insert(dir, vec![]);
                    }

                    if let Some(vec) = wr.get_mut(&dir) {
                        vec.push(file.clone());
                    } else {
                        tracing::debug!("{dir:#?} not found");
                    }

                    if let Err(err) = tx_ws
                        .send(MsgWs::Change {
                            subscriber: dir.to_string(),
                            change: Change::New { dir, file },
                        })
                        .await
                    {
                        tracing::error!("[Scheduler] Sent message to WebSocket manager: {err}");
                    }

                    if validate_name_and_replace(
                        PathBuf::from(path.replace(wr.root(), wr.real_path())),
                        &file_name,
                    )
                    .await
                    .is_err()
                    {
                        tracing::error!("[Scheduler] Validate error");
                    }
                }
                Some(Change::Delete { parent, file_name }) => {
                    let mut wr = self.state.write().await;

                    let mut queue = VecDeque::new();
                    let mut key_to_delete = parent.path().clone();
                    tracing::trace!("[Scheduler] {{ Some(Change::Delete) }} Directory: {parent:?}");
                    tracing::trace!(
                        "[Scheduler] {{ Some(Change::Delete) }} File name: {file_name:?}"
                    );
                    if let Some(files) = wr.get_mut(&parent) {
                        if let Some(file) = files.pop_if(|x| x.file_name() == file_name) {
                            if file.is_dir() {
                                key_to_delete.push(file.file_name());
                                queue.push_front(Directory::new_unchk_from_path(&key_to_delete));
                            }

                            tracing::info!(
                                "[Scheduler] Delete file {file:?} - is_dir: {}",
                                file.is_dir()
                            );
                        } else {
                            tracing::warn!("[Scheduler] File {file_name:?} not found");
                        }
                    } else {
                        tracing::warn!("[Scheduler] Directory {parent:?} not found");
                    }

                    while let Some(dir) = queue.pop_front() {
                        tracing::trace!(
                            "[Scheduler] {{ Some(Change::Delete)}} Delete dir: {dir:?}"
                        );
                        if let Some(files) = wr.remove(&dir) {
                            queue.extend(
                                files
                                    .into_iter()
                                    .filter(File::is_dir)
                                    .map(|x| {
                                        key_to_delete.pop();
                                        key_to_delete.push(x.file_name());
                                        Directory::new_unchk_from_path(&key_to_delete)
                                    })
                                    .collect::<VecDeque<Directory>>(),
                            );
                        }
                    }
                }
                Some(Change::Name { dir, from, to }) => {
                    let mut wr = self.state.write().await;
                    tracing::trace!(
                        "[Scheduler] {{ Some(Change::Name {{ dir: {dir:?}, from: {from:?}, to: {to:?} }}) }}"
                    );
                    let path = dir.as_ref().to_string();
                    let file_name = to.file_name().to_string();
                    let is_dir = to.is_dir();

                    if let Some(files) = wr.get_mut(&dir) {
                        if let Some(file) = files.iter_mut().find(|x| x.file_name() == from) {
                            *file = to.clone();
                        } else {
                            tracing::info!("[Scheduler] File {} not found", to.file_name());
                            tracing::info!("[Scheduler] Inser File {}", to.file_name());
                            files.push(to.clone());
                        }
                    }

                    if is_dir {
                        let mut path = dir.path();
                        path.push(&from);
                        tracing::trace!("[Scheduler] IS DIR RENAME: {path:?}");
                        let dir = Directory::new_unchk_from_path(&path);
                        let files = if let Some(files) = wr.remove(&dir) {
                            files
                        } else {
                            tracing::debug!("[Scheduler] Directory {dir:?} is empty");
                            Vec::new()
                        };
                        path.pop();
                        path.push(to.file_name());
                        tracing::trace!(
                            "[Scheduler] {{ Rename directory }} from: {:?} to: {:?}",
                            dir.path(),
                            path
                        );
                        let dir = Directory::new_unchk_from_path(&path);
                        wr.insert(dir, files);
                    }

                    if let Err(err) = tx_ws
                        .send(MsgWs::Change {
                            subscriber: dir.path().to_str().unwrap().to_string(),
                            change: Change::Name { dir, from, to },
                        })
                        .await
                    {
                        eprintln!("{err}");
                    }

                    if validate_name_and_replace(
                        PathBuf::from(path.replace(wr.root(), wr.real_path())),
                        &file_name,
                    )
                    .await
                    .is_err()
                    {
                        tracing::error!("[Scheduler] Validate error");
                    }
                }
                None => {
                    tracing::error!("[Scheduler] Peer: tx_watcher closed");
                    break;
                }
            }
        }
    }

    pub async fn add_cliente(&mut self, path: String, ws: WsSenderType) {
        _ = self
            .tx_ws
            .send(MsgWs::NewUser {
                sender: ws,
                subscriber: path,
            })
            .await;
    }
}

#[derive(Debug)]
pub enum MsgWs {
    NewUser {
        subscriber: String,
        sender: WsSenderType,
    },
    Change {
        subscriber: String,
        change: Change,
    },
}

#[derive(Debug, Clone, Serialize)]
pub enum Change {
    New {
        dir: Directory,
        file: File,
    },
    Name {
        dir: Directory,
        from: String,
        to: File,
    },
    Delete {
        parent: Directory,
        file_name: String,
    },
}

#[derive(Debug)]
struct RenameControl {
    notify: UnboundedSender<Rename>,
}

impl RenameControl {
    pub(self) fn new(
        sender_watcher: UnboundedSender<Result<notify::Event, notify::Error>>,
        r#await: u64,
    ) -> Self {
        let (tx, mut rx) = unbounded_channel();
        tokio::spawn(async move {
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
                    _ => tracing::error!("[RenameControl] Sender was close"),
                }
            }
        });

        Self { notify: tx }
    }

    pub fn sender(&self) -> UnboundedSender<Rename> {
        self.notify.clone()
    }
}

#[derive(Debug)]
struct DropDelete;

#[derive(Debug)]
struct RenameFrom(PathBuf);

enum Rename {
    From(RenameFrom),
    Decline(PathBuf),
}

pub async fn ws_changes_handle(mut ws: WsSenderType, mut rx: ReceivedBr<Change>) {
    while let Ok(recv) = rx.recv().await {
        ws.send(Message::Text(json!(recv).to_string().into()))
            .await
            .unwrap();
    }
}