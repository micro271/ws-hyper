pub mod channels_types;
pub mod new_file_tba;
pub mod utils;
pub mod watcher;
pub mod websocker;

use futures::{SinkExt, stream::SplitSink};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{HyperWebsocket, WebSocketStream, tungstenite::Message};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use std::{collections::VecDeque, path::PathBuf, sync::Arc, vec};
use utils::validate_name_and_replace;
use watcher::Watcher;

use serde_json::json;
use tokio::sync::{
    RwLock,
    broadcast::Receiver as ReceivedBr,
    mpsc::{Sender, UnboundedReceiver, UnboundedSender, channel, unbounded_channel},
};
use utils::Executing;

use crate::{
    bucket::{Bucket, object::Object, bucket_map::BucketMap},
    manager::{
        utils::{OneshotSender, Task},
        watcher::WatcherOwn,
        websocker::{MsgWs, WebSocker},
    },
};

type WsSenderType = SplitSink<WebSocketStream<TokioIo<Upgraded>>, Message>;

#[derive(Debug)]
pub struct Schedule<W, T, TxInner> {
    tx_ws: Sender<MsgWs>,
    pub state: Arc<RwLock<BucketMap>>,
    _watcher: Watcher<Executing, W, T, TxInner>,
}

impl<W, TxInner> Schedule<W, UnboundedSender<Change>, TxInner>
where
    W: WatcherOwn<UnboundedSender<Change>, TxInner> + Send + 'static + Sync,
    TxInner: OneshotSender + Clone + Sync,
{
    pub fn run(
        state: Arc<RwLock<BucketMap>>,
        watcher: Watcher<Task<W>, W, UnboundedSender<Change>, TxInner>,
    ) -> Sender<MsgWs> {
        let (tx_ws, rx_ws) = channel(128);

        let (tx_sch, rx_sch) = unbounded_channel();
        let resp = tx_ws.clone();
        let (watcher, task) = watcher.task();

        let myself = Arc::new(Self {
            tx_ws,
            state,
            _watcher: watcher,
        });

        task.run(tx_sch);
        WebSocker::run(rx_ws);
        tokio::task::spawn(myself.clone().run_scheduler_mg(rx_sch));
        resp
    }

    async fn run_scheduler_mg(self: Arc<Self>, mut rx_watcher: UnboundedReceiver<Change>) {
        tracing::info!("Scheduler init");
        let tx_ws = self.tx_ws.clone();
        loop {
            match rx_watcher.recv().await {
                Some(Change::New { dir, file }) => {
                    tracing::trace!("[Scheduler] Input dir: {dir:?} - file: {file}");
                    let mut wr = self.state.write().await;
                    let path = dir.as_ref().to_string();
                    let file_name = file.key().to_string();
                    if file.is_dir() {
                        let mut path = dir.path();
                        path.push(file.key());
                        let dir = Bucket::new_unchk_from_path(path);
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
                            subscriber: dir.clone(),
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
                        if let Some(file) = files.pop_if(|x| x.key() == file.key()) {
                            if file.is_dir() {
                                key_to_delete.push(file.key());
                                queue.push_front(Bucket::new_unchk_from_path(&key_to_delete));
                            }

                            tracing::warn!(
                                "[Scheduler] {{ Task: Delete }} Delete file {file:?} - is_dir: {}",
                                file.is_dir()
                            );
                        } else {
                            tracing::info!(
                                "[Scheduler] {{ Task: Delete }} Object {file:?} not found"
                            );
                        }
                    } else {
                        tracing::info!(
                            "[Scheduler] {{ Task: Delete }} Bucket {parent:?} not found"
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
                                    .filter(Object::is_dir)
                                    .map(|x| {
                                        key_to_delete.pop();
                                        key_to_delete.push(x.key());
                                        Bucket::new_unchk_from_path(&key_to_delete)
                                    })
                                    .collect::<VecDeque<Bucket>>(),
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
                    let file_name = to.key().to_string();
                    let is_dir = to.is_dir();

                    if let Some(files) = wr.get_mut(&dir) {
                        if let Some(file) = files.iter_mut().find(|x| x.key() == from) {
                            *file = to.clone();
                        } else {
                            tracing::info!("[Scheduler] Object {} not found", to.key());
                            tracing::info!("[Scheduler] Inser Object {}", to.key());
                            files.push(to.clone());
                        }
                    }

                    if is_dir {
                        let mut path = dir.path();
                        path.push(&from);
                        tracing::trace!("[Scheduler] IS DIR RENAME: {path:?}");
                        let dir = Bucket::new_unchk_from_path(&path);
                        let files = if let Some(files) = wr.remove(&dir) {
                            files
                        } else {
                            tracing::debug!("[Scheduler] Bucket {dir:?} is empty");
                            Vec::new()
                        };
                        path.pop();
                        path.push(to.key());
                        tracing::trace!(
                            "[Scheduler] {{ Rename directory }} from: {:?} to: {:?}",
                            dir.path(),
                            path
                        );
                        let dir = Bucket::new_unchk_from_path(&path);
                        wr.insert(dir, files);
                    }

                    if let Err(err) = tx_ws
                        .send(MsgWs::Change {
                            subscriber: dir.clone(),
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

    pub async fn add_cliente(&mut self, path: String, ws: HyperWebsocket) {
        _ = self
            .tx_ws
            .send(MsgWs::NewUser {
                sender: ws,
                subscriber: Bucket::new_unchk_from_path(path),
            })
            .await;
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum Change {
    New {
        dir: Bucket,
        file: Object,
    },
    Name {
        dir: Bucket,
        from: String,
        to: Object,
    },
    Delete {
        parent: Bucket,
        file: Object,
    },
}

pub async fn ws_changes_handle(mut ws: WsSenderType, mut rx: ReceivedBr<Change>) {
    while let Ok(recv) = rx.recv().await {
        ws.send(Message::Text(json!(recv).to_string().into()))
            .await
            .unwrap();
    }
}
