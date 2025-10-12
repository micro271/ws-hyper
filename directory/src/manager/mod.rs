pub mod utils;

use futures::{SinkExt, stream::SplitSink};
use hyper_tungstenite::{WebSocketStream, tungstenite::Message};
use notify::Watcher;
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

pub struct Schedule<S> {
    tx_ws: Sender<MsgWs<S>>,
    state: Arc<RwLock<TreeDir>>,
    root: String,
}

impl<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + 'static + Send> Schedule<S> {
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
                match msg {
                    Some(MsgWs::Change { subscriber, change }) => {
                        if users.read().await.get(&subscriber).is_none() {
                            continue;
                        }
                        match change {
                            Change::New { path, name } => todo!(),
                            Change::Name { path, from, to } => todo!(),
                            Change::Delete { path, name } => todo!(),
                        }
                    }
                    Some(MsgWs::NewUser {
                        subscriber,
                        mut sender,
                    }) => {
                        let mut rx = match users.write().await.get_mut(&subscriber.to_lowercase()) {
                            Some(subs) => subs.subscribe(),
                            None => {
                                let (tx, rx) = broadcast::channel(256);
                                users.write().await.insert(subscriber, tx);
                                rx
                            }
                        };
                        tokio::spawn(async move {
                            while let Ok(change) = rx.recv().await {
                                _ = sender
                                    .send(Message::Text(json!(change).to_string().into()))
                                    .await;
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
        tokio::spawn(async move {
            loop {
                {
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
                            notify::EventKind::Create(create_kind) => {
                                println!("create: {:?}", create_kind);
                                for i in event.paths {
                                    _ = tx_watcher.send(Change::New {
                                        path: i.to_str().unwrap().to_string(),
                                        name: "ALGO".to_string(),
                                    });
                                    println!("{i:?}");
                                }
                            }
                            notify::EventKind::Modify(modify_kind) => {
                                println!("notifi modify: {:?}", modify_kind);
                                for i in event.paths {
                                    _ = tx_watcher.send(Change::New {
                                        path: i.to_str().unwrap().to_string(),
                                        name: "ALGO".to_string(),
                                    });
                                    println!("{i:?}");
                                }
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
            }
        });

        //scheduler
        let st = state.clone();
        let tx_ws_cp = tx_ws.clone();
        tokio::spawn(async move {
            loop {
                match rx_watcher.recv().await {
                    Some(Change::New { path, name }) => {
                        let mut tmp = st.write().await;
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
                                                _ = tx_ws_cp
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
                    Some(Change::Name { path, from, to }) => {}
                    None => println!("NADA"),
                }
            }
        });

        Self {
            tx_ws,
            state,
            root: path,
        }
    }

    pub async fn add_cliente(&mut self, path: String, ws: SplitSink<WebSocketStream<S>, Message>) {
        _ = self
            .tx_ws
            .send(MsgWs::NewUser {
                sender: ws,
                subscriber: path,
            })
            .await;
    }
}

pub enum MsgWs<S> {
    NewUser {
        subscriber: String,
        sender: SplitSink<WebSocketStream<S>, Message>,
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

pub async fn ws_changes_handle<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
    mut ws: SplitSink<WebSocketStream<S>, Message>,
    mut rx: ReceivedBr<Change>,
) {
    while let Ok(recv) = rx.recv().await {
        ws.send(Message::Text(json!(recv).to_string().into()))
            .await
            .unwrap();
    }
}
