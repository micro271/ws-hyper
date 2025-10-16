pub mod utils;

use futures::{SinkExt, stream::SplitSink};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{WebSocketStream, tungstenite::Message};
use hyper_util::rt::TokioIo;
use notify::{
    Watcher,
    event::{ModifyKind, RenameMode},
};
use regex::Regex;
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc, time::Duration,
};

use serde_json::json;
use tokio::{
    fs,
    sync::{
        RwLock,
        broadcast::{self, Receiver as ReceivedBr, Sender as SenderBr},
        mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender, channel, unbounded_channel},
    },
};

use crate::{
    directory::{file::{self, File}, tree_dir::TreeDir},
    manager::utils::FromDirEntyAsync,
};

type WsSenderType = SplitSink<WebSocketStream<TokioIo<Upgraded>>, Message>;

#[derive(Debug)]
pub struct Schedule {
    tx_ws: Sender<MsgWs>,
    pub state: Arc<RwLock<TreeDir>>,
}

impl Schedule {
    pub async fn new(state: Arc<RwLock<TreeDir>>) -> Self {
        let (tx_ws, rx_ws) = channel(256);
        let (tx_watcher, rx_watcher) = unbounded_channel::<Change>();
        let own = tx_ws.clone();

        tokio::task::spawn(Self::run_watcher_mg(state.clone(), tx_watcher));
        tokio::task::spawn(Self::run_websocker_mg(rx_ws));
        tokio::task::spawn(Self::run_scheduler_mg(state.clone(), tx_ws, rx_watcher));

        Self { tx_ws: own, state }
    }

    async fn run_websocker_mg(mut rx_ws: Receiver<MsgWs>) {
        let mut users = HashMap::<String, SenderBr<Change>>::new();
        tracing::debug!("Web socket manage init");
        loop {
            let msg = rx_ws.recv().await;
            tracing::debug!("{msg:?}");
            match msg {
                Some(MsgWs::Change { subscriber, change }) => {
                    if let Some(send) = users.get(&subscriber) {
                        if let Err(err) = send.send(change) {
                            tracing::error!("{err}");
                            _ = users.remove(&subscriber);
                        }
                    }
                }
                Some(MsgWs::NewUser {
                    subscriber,
                    mut sender,
                }) => {
                    let mut rx = {
                        match users.get(&subscriber) {
                            Some(subs) => subs.subscribe(),
                            None => {
                                let (tx, rx) = broadcast::channel(256);
                                users.insert(subscriber, tx);
                                rx
                            }
                        }
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
                },
            }
        }
    }

    async fn run_watcher_mg(
        state: Arc<RwLock<TreeDir>>,
        tx_watcher: UnboundedSender<Change>,
    ) {
        tracing::debug!("Watcher notify manage init");
        let real_path = state.read().await.real_path().to_string();

        let (tx, mut rx) = unbounded_channel();
        let mut watcher = notify::recommended_watcher(move |x| {
            _ = tx.send(x);
        })
        .unwrap();
        watcher
            .watch(Path::new(&real_path), notify::RecursiveMode::Recursive)
            .unwrap();

        loop {
            while let Some(Ok(event)) = rx.recv().await {
                match event.kind {
                    notify::EventKind::Create(_) => {
                        tracing::debug!("{event:?}");
                        let reader = state.read().await;
                        let mut path = event.paths;
                        let path = path.pop().unwrap();
                        let name = path
                            .file_name()
                            .and_then(|x| x.to_str())
                            .unwrap()
                            .to_string();
                        let path = path.parent().and_then(|x| x.to_str()).unwrap().to_string();

                        
                        if let Err(err) = tx_watcher.send(Change::New { path, name }) {
                            tracing::error!("New filt nofity error: {err}");
                        }
                    }
                    notify::EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {

                        let re = Regex::new(r"(^\.[^.])|(^\.\.)|(\s+)|(^$)").unwrap();

                        let mut paths = event.paths;
                        let to = paths.pop().unwrap();
                        let from = paths.pop().unwrap();
                        tracing::info!("Rename from: {from:?} - to: {to:?}");
                        let to_file_name = to.file_name().unwrap().to_str().unwrap();

                        if re.is_match(to_file_name) {
                            tracing::info!("file name: {to_file_name:?} is not valid, auto rename executed");

                            let new_to_file_name = re.replace_all(&to_file_name, |caps: &regex::Captures<'_>|{
                                if caps.get(1).is_some() {
                                    "[DOT]".to_string()
                                } else if caps.get(2).is_some() {
                                    "[DOT][DOT]".to_string()
                                } else if caps.get(3).is_some() {
                                    "_".to_string()
                                }  else if caps.get(4).is_some() {
                                    uuid::Uuid::new_v4().to_string()
                                } else {
                                    caps.get(0).unwrap().as_str().to_string()
                                }
                            }).to_string();

                            let new_to = format!("{}/{}",to.parent().and_then(|x| x.to_str()).unwrap(), new_to_file_name);
                            tracing::debug!("Attempt to rename from: {to:?} - to: {new_to:?}");
                            
                            if let Err(err) = fs::rename(&to, &new_to).await {
                                tracing::error!("Auto rename error from: {to:?} to: {new_to:?}, error: {err}");
                            }
                            tracing::warn!("Auto rename from: {to:?} - to: {new_to:?}");
                            continue;
                        }
                        
                        let reader = state.read().await;
                        let root = reader.root();
                        let path = to.parent().and_then(|x| x.to_str().map(|x| format!("{}/", x))).unwrap();
                        let path = path.replace(&real_path, &root);
                        let from_file_name = from.file_name().and_then(|x| x.to_str()).unwrap();

                        if let Err(err) = tx_watcher.send(Change::Name { path, from: from_file_name.to_string(), to: to_file_name.to_string() }) {
                            tracing::error!("tx_watcher error: {err}");
                        }
                    }
                    notify::EventKind::Remove(remove_kind) => {
                        for i in event.paths {
                            _ = tx_watcher.send(Change::New {
                                path: i.to_str().unwrap().to_string(),
                                name: "ALGO".to_string(),
                            });
                        }
                    }
                    _ => {  }
                }
            }
        }
    }

    async fn run_scheduler_mg(
        state: Arc<RwLock<TreeDir>>,
        tx_ws: Sender<MsgWs>,
        mut rx_watcher: UnboundedReceiver<Change>,
    ) {
        tracing::debug!("Scheduler init");
        loop {
            match rx_watcher.recv().await {
                Some(Change::New { path, name }) => {
                    let mut tmp = state.write().await;
                    let directory = path.into();
                    match tmp.get_mut(&directory) {
                        Some(file) => {
                            let mut entry = fs::read_dir(directory.as_ref()).await.unwrap();
                            loop {
                                match entry.next_entry().await {
                                    Ok(Some(entry)) => {
                                        if entry.file_name().to_str().unwrap() == name {
                                            file.push(File::from_entry(entry).await);
                                            let path = directory.inner();
                                            if let Err(err) = tx_ws
                                                .send(MsgWs::Change {
                                                    change: Change::New {
                                                        path: path.clone(),
                                                        name,
                                                    },
                                                    subscriber: path,
                                                })
                                                .await
                                            {
                                                tracing::error!("Producer websocket error: {err}");
                                            } else {
                                                tracing::debug!("info send to rx_ws")
                                            }
                                            break;
                                        }
                                    }
                                    Ok(None) => break,
                                    Err(_) => todo!(),
                                }
                            }
                        }
                        None => {}
                    }
                }
                Some(Change::Delete { path, name }) => {}
                Some(Change::Name { path, from, to }) => {
                    if let Err(err) = tx_ws
                        .send(MsgWs::Change {
                            subscriber: path.clone(),
                            change: Change::Name {
                                path: path,
                                from,
                                to,
                            },
                        })
                        .await
                    {
                        eprintln!("{err}");
                    }
                }
                None => {
                    tracing::error!("Peer: tx_watcher closed");
                    break;
                },
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
        path: String,
        name: String,
    },
    Name {
        path: String,
        from: String,
        to: String,
    },
    Delete {
        path: String,
        name: String,
    },
}

pub async fn ws_changes_handle(mut ws: WsSenderType, mut rx: ReceivedBr<Change>) {
    while let Ok(recv) = rx.recv().await {
        ws.send(Message::Text(json!(recv).to_string().into()))
            .await
            .unwrap();
    }
}
