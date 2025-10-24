pub mod new_file_tba;
pub mod utils;
pub mod watcher;

use futures::{SinkExt, stream::SplitSink};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{WebSocketStream, tungstenite::Message};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::Arc,
    vec,
};
use utils::validate_name_and_replace;
use watcher::Watcher;

use serde_json::json;
use tokio::sync::{
    RwLock,
    broadcast::{self, Receiver as ReceivedBr, Sender as SenderBr},
    mpsc::{Receiver, Sender, UnboundedReceiver, channel, unbounded_channel},
};

use crate::{
    directory::{Directory, file::File, tree_dir::TreeDir},
    manager::watcher::{Executing, Task, WatcherOwn},
};

type WsSenderType = SplitSink<WebSocketStream<TokioIo<Upgraded>>, Message>;

#[derive(Debug)]
pub struct Schedule<W, T, R> {
    tx_ws: Sender<MsgWs>,
    pub state: Arc<RwLock<TreeDir>>,
    _watcher: Watcher<Executing, W, T, R>,
}

impl<W, R> Schedule<W, Change, R>
where
    W: WatcherOwn<Change, R> + 'static,
    R: Send + Sync + 'static,
{
    pub fn run(state: Arc<RwLock<TreeDir>>, watcher: Watcher<Task<W>, W, Change, R>) {
        let (tx_ws, rx_ws) = channel(256);
        let (tx_sch, rx_sch) = unbounded_channel();

        let (watcher, task) = watcher.task();

        let myself = Arc::new(Self {
            tx_ws,
            state,
            _watcher: watcher,
        });

        task.run(tx_sch);
        tokio::task::spawn(myself.clone().run_websocker_mg(rx_ws));
        tokio::task::spawn(myself.clone().run_scheduler_mg(rx_sch));
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

                    if let Err(err) = validate_name_and_replace(
                        PathBuf::from(path.replace(wr.root(), wr.real_path())),
                        &file_name,
                    )
                    .await
                    {
                        tracing::error!("[Scheduler] Validate error - {err:?}");
                    }
                }
                Some(Change::Delete { parent, file }) => {
                    let mut wr = self.state.write().await;

                    let mut queue = VecDeque::new();
                    let mut key_to_delete = parent.path().clone();
                    tracing::trace!(
                        "[Scheduler] {{ Task: Delete }} {{ Some(Change::Delete {{ parent: {parent:?}, file_name: {file:?} }}) }}"
                    );

                    if let Some(files) = wr.get_mut(&parent) {
                        if let Some(file) = files.pop_if(|x| x.file_name() == file.file_name()) {
                            if file.is_dir() {
                                key_to_delete.push(file.file_name());
                                queue.push_front(Directory::new_unchk_from_path(&key_to_delete));
                            }

                            tracing::warn!(
                                "[Scheduler] {{ Task: Delete }} Delete file {file:?} - is_dir: {}",
                                file.is_dir()
                            );
                        } else {
                            tracing::info!(
                                "[Scheduler] {{ Task: Delete }} File {file:?} not found"
                            );
                        }
                    } else {
                        tracing::info!(
                            "[Scheduler] {{ Task: Delete }} Directory {parent:?} not found"
                        );
                    }

                    while let Some(dir) = queue.pop_front() {
                        tracing::trace!(
                            "[Scheduler] {{ Task: Delete }} {{ Some(Change::Delete)}} Delete dir: {dir:?}"
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

                    if let Err(err) = validate_name_and_replace(
                        PathBuf::from(path.replace(wr.root(), wr.real_path())),
                        &file_name,
                    )
                    .await
                    {
                        tracing::error!("[Scheduler] Validate error {path:?} {err:?}");
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
        file: File,
    },
}

pub async fn ws_changes_handle(mut ws: WsSenderType, mut rx: ReceivedBr<Change>) {
    while let Ok(recv) = rx.recv().await {
        ws.send(Message::Text(json!(recv).to_string().into()))
            .await
            .unwrap();
    }
}
