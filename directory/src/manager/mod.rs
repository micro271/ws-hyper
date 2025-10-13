pub mod utils;

use futures::{SinkExt, stream::SplitSink};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{WebSocketStream, tungstenite::Message};
use hyper_util::rt::TokioIo;
use notify::{
    Watcher,
    event::{ModifyKind, RenameMode},
};
use serde::Serialize;
use std::{collections::HashMap, path::Path, sync::Arc};

use serde_json::json;
use tokio::{
    fs,
    sync::{
        RwLock,
        broadcast::{self, Receiver as ReceivedBr, Sender as SenderBr},
        mpsc::{Sender, channel, unbounded_channel},
    },
};

use crate::{
    directory::{file::File, tree_dir::TreeDir},
    manager::utils::FromDirEntyAsync,
};

type WsSenderType = SplitSink<WebSocketStream<TokioIo<Upgraded>>, Message>;

pub struct Schedule {
    tx_ws: Sender<MsgWs>,
    pub state: Arc<RwLock<TreeDir>>,
}

impl Schedule {
    pub async fn new(state: Arc<RwLock<TreeDir>>, path: String) -> Self {
        // websocket
        let (tx_ws, mut rx_ws) = channel(256);
        let path = fs::canonicalize(path)
            .await
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        tokio::spawn(async move {
            let users = RwLock::new(HashMap::<String, SenderBr<Change>>::new());
            loop {
                let msg = rx_ws.recv().await;
                println!("{:?}",msg);
                match msg {
                    Some(MsgWs::Change { subscriber, change }) => {
                        let sender = users.read().await;
                        if let Some(send) = sender.get(&subscriber) {
                            if let Err(err) = send.send(change) {
                                drop(sender);
                                tracing::error!("{err}");
                                _ = users.write().await.remove(&subscriber);
                            }
                        }
                        continue;
                    }
                    Some(MsgWs::NewUser {
                        subscriber,
                        mut sender,
                    }) => {
                        let mut rx = {
                            let reader = users.read().await;
                            match reader.get(&subscriber) {
                                Some(subs) => subs.subscribe(),
                                None => {
                                    let (tx, rx) = broadcast::channel(256);
                                    drop(reader);
                                    users.write().await.insert(subscriber, tx);
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
                    None => {}
                }
            }
        });

        // whatcher
        let (tx_watcher, mut rx_watcher) = unbounded_channel::<Change>();
        let path_cp = path.clone();
        let state_copy = state.clone();
        tokio::spawn(async move {
            let state = state_copy;
            loop {
                let (tx, mut rx) = unbounded_channel();
                let mut watcher = notify::recommended_watcher(move |x| {
                    _ = tx.send(x);
                })
                .unwrap();
                watcher
                    .watch(Path::new(&path_cp), notify::RecursiveMode::Recursive)
                    .unwrap();
                while let Some(Ok(event)) = rx.recv().await {
                    match event.kind {
                        notify::EventKind::Create(_) => {
                            let reader = state.read().await;
                            let root = reader.path_prefix();
                            let mut path = event.paths;
                            let path = path.pop().unwrap();
                            let name = path.file_name().and_then(|x| x.to_str()).unwrap().to_string();
                            let path = path.parent().and_then(|x| x.to_str()).unwrap().to_string();
                            println!("{name} - {path} - {root:?}");
                            _ = tx_watcher.send(Change::New { path, name });
                        }
                        notify::EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                            let mut paths = event.paths;
                            let reader = state.read().await;
                            let root_path = reader.real_path().to_string();
                            let to = paths
                                .pop()
                                .and_then(|x| {
                                    x.to_str().and_then(|x| {
                                        x.strip_prefix(&format!("{root_path}/"))
                                            .map(ToString::to_string)
                                    })
                                })
                                .unwrap();
                            let from = paths
                                .pop()
                                .and_then(|x| {
                                    x.to_str().and_then(|x| {
                                        x.strip_prefix(&format!("{root_path}/"))
                                            .map(ToString::to_string)
                                    })
                                })
                                .unwrap();

                            let (to, from) = match reader.path_prefix() {
                                Some(e) => (format!("{e}{to}"), format!("{e}{from}")),
                                None => (to, from),
                            };
                            _ = tx_watcher.send(Change::Name {
                                path: root_path,
                                from,
                                to,
                            });
                        }
                        notify::EventKind::Remove(remove_kind) => {
                            println!("notifi remove:{:?}", remove_kind);
                            for i in event.paths {
                                _ = tx_watcher.send(Change::New {
                                    path: i.to_str().unwrap().to_string(),
                                    name: "ALGO".to_string(),
                                });
                                println!("{i:?}");
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        //scheduler
        let st = Arc::clone(&state);
        let tx_ws_cp = tx_ws.clone();
        tokio::spawn(async move {
            let tx_ws = tx_ws_cp;
            let state = st;
            loop {
                match rx_watcher.recv().await {
                    Some(Change::New { path, name }) => {
                        println!("{path} - {name}");
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
                                                _ = tx_ws
                                                    .send(MsgWs::Change {
                                                        change: Change::New {
                                                            path: path.clone(),
                                                            name,
                                                        },
                                                        subscriber: path,
                                                    })
                                                    .await;
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
                        if let Err(err) = tx_ws.send(MsgWs::Change { subscriber: path.clone(), change: Change::Name { path: path, from, to } }).await {
                            eprintln!("{err}");
                        }
                    }
                    None => println!("NADA"),
                }
            }
        });

        Self { tx_ws, state }
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
