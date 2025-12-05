pub mod channels_types;
pub mod new_file_tba;
pub mod utils;
pub mod watcher;
pub mod websocker;

use futures::{SinkExt, stream::SplitSink};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{WebSocketStream, tungstenite::Message};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use tonic::transport::Endpoint;

use serde_json::json;
use tokio::sync::{
    RwLock,
    broadcast::Receiver as ReceivedBr,
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};

use crate::{
    bucket::{
        Bucket,
        bucket_map::BucketMap,
        key::Key,
        object::Object,
        utils::rename_handlers::{NewObjNameHandlerBuilder, RenameObjHandlerBuilder},
    },
    grpc_v1::ConnectionAuthMS,
    manager::{
        utils::{Run, SplitTask, Task},
        watcher::{event_watcher::EventWatcherBuilder, pool_watcher::PollWatcherNotify},
        websocker::{MsgWs, WebSocket, WebSocketChSender},
    },
    state::{
        local_storage::LocalStorage,
        pg_listen::{ListenBucket, ListenBucketChSender},
    },
};

type WsSenderType = SplitSink<WebSocketStream<TokioIo<Upgraded>>, Message>;

#[derive(Debug)]
pub struct ManagerChSenders {
    grpc: ConnectionAuthMS,
    ws: WebSocketChSender,
}

pub struct Manager {
    state: Arc<RwLock<BucketMap>>,
    tx: UnboundedSender<Change>,
    rx: UnboundedReceiver<Change>,
    listen_bk: ListenBucket,
    grpc: ConnectionAuthMS,
    ws: WebSocket,
    watcher_params: WatcherParams,
    local_storage: Arc<LocalStorage>,
}

pub struct ManagerRunning {
    ws_sender: WebSocketChSender,
    rx: UnboundedReceiver<Change>,
    state: Arc<RwLock<BucketMap>>,
    grpc: ConnectionAuthMS,
    lst_sender: ListenBucketChSender,
    local_storage: Arc<LocalStorage>,
}

impl Manager {
    pub async fn new(
        state: Arc<RwLock<BucketMap>>,
        watcher_params: WatcherParams,
        endpoint: Endpoint,
        listen_bk: ListenBucket,
        local_storage: Arc<LocalStorage>,
    ) -> Self {
        let (tx_sch, rx_sch) = unbounded_channel();

        Self {
            state,
            tx: tx_sch.clone(),
            rx: rx_sch,
            listen_bk,
            grpc: ConnectionAuthMS::new(endpoint, tx_sch).await,
            watcher_params,
            ws: WebSocket::new(),
            local_storage,
        }
    }
}

impl Run for Manager {
    fn run(self)
    where
        Self: Sized,
    {
        match self.watcher_params {
            WatcherParams::Event {
                path,
                r#await,
                ignore_rename_suffix,
            } => {
                let task = EventWatcherBuilder::default()
                    .path(path)
                    .unwrap()
                    .change_notify(self.tx.clone())
                    .rename_control_await(r#await.unwrap_or(3000))
                    .ignore_rename_prefix(ignore_rename_suffix)
                    .build()
                    .unwrap();
                task.run();
            }
            WatcherParams::Poll { path, interval } => {
                let task = PollWatcherNotify::new(path, interval.unwrap_or_default()).unwrap();
                task.run();
            }
        }
        let (lst_ch, lst_task) = self.listen_bk.split();
        let (ws_sender, ws_task) = self.ws.split();

        lst_task.run();
        ws_task.run();

        let task = ManagerRunning {
            ws_sender,
            rx: self.rx,
            state: self.state,
            lst_sender: lst_ch,
            grpc: self.grpc,
            local_storage: self.local_storage,
        };

        task.run();
    }

    fn executor(self) -> impl Run
    where
        Self: Sized,
    {
        self
    }
}

impl SplitTask for Manager {
    type Output = ManagerChSenders;

    fn split(self) -> (<Self as SplitTask>::Output, impl Run) {
        (
            ManagerChSenders {
                ws: self.ws.get_sender(),
                grpc: self.grpc.clone(),
            },
            self,
        )
    }
}

impl Task for ManagerRunning {
    async fn task(mut self) {
        tracing::info!("Scheduler init");

        let tx_ws = self.ws_sender;

        loop {
            match self.rx.recv().await {
                Some(mut change) => {
                    match &mut change {
                        Change::NewObject {
                            object,
                            key,
                            bucket,
                        } => {
                            NewObjNameHandlerBuilder::default()
                                .bucket(bucket)
                                .key(key)
                                .object(object)
                                .build()
                                .run(self.local_storage.clone())
                                .await;
                        }
                        Change::DeleteObject {
                            file_name,
                            bucket,
                            key,
                        } => {
                            self.local_storage
                                .delete_object(bucket, key, file_name)
                                .await;
                        }
                        Change::NameObject {
                            key,
                            to,
                            bucket,
                            file_name,
                        } => {
                            RenameObjHandlerBuilder::default()
                                .bucket(bucket)
                                .key(key)
                                .to(to)
                                .from(file_name)
                                .build()
                                .run(self.local_storage.clone())
                                .await;
                        }
                        Change::NewBucket { .. }
                        | Change::DeleteBucket { .. }
                        | Change::NameBucket { .. } => {
                            if let Err(er) = self.lst_sender.send(change.clone()).await {
                                tracing::error!("Send to ListenBucket Error: {er}")
                            }
                        }
                        _ => {}
                    }
                    self.state.write().await.change(change.clone()).await;
                    tx_ws.send(MsgWs::Change(change.clone())).await.unwrap();
                }
                None => {
                    tracing::error!("[Scheduler] Peer: tx_watcher closed");
                    break;
                }
            }
        }
    }
}

impl ManagerChSenders {
    fn client_grpc(&self) -> &ConnectionAuthMS {
        &self.grpc
    }

    fn ws_sender(&self) -> &WebSocketChSender {
        &self.ws
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum Change {
    NewObject {
        bucket: Bucket,
        key: Key,
        object: Object,
    },
    NewKey {
        bucket: Bucket,
        key: Key,
    },
    NewBucket {
        bucket: Bucket,
    },
    NameObject {
        bucket: Bucket,
        key: Key,
        file_name: String,
        to: String,
    },
    NameBucket {
        from: Bucket,
        to: Bucket,
    },
    NameKey {
        bucket: Bucket,
        from: Key,
        to: Key,
    },
    DeleteObject {
        bucket: Bucket,
        key: Key,
        file_name: String,
    },
    DeleteKey {
        bucket: Bucket,
        key: Key,
    },
    DeleteBucket {
        bucket: Bucket,
    },
}

#[derive(Debug)]
enum Scope {
    Bucket(Bucket),
    Key(Bucket, Key),
}

impl Change {
    fn scope(&self) -> Scope {
        match self {
            Change::NewObject { bucket, key, .. }
            | Change::NewKey { bucket, key }
            | Change::NameObject { bucket, key, .. }
            | Change::DeleteObject { bucket, key, .. }
            | Change::DeleteKey { bucket, key } => Scope::Key(bucket.clone(), key.clone()),
            Change::NameBucket { from, .. } => Scope::Bucket(from.clone()),
            Change::NameKey { bucket, from, .. } => Scope::Key(bucket.clone(), from.clone()),
            Change::NewBucket { bucket } | Change::DeleteBucket { bucket } => {
                Scope::Bucket(bucket.clone())
            }
        }
    }
}

pub async fn ws_changes_handle(mut ws: WsSenderType, mut rx: ReceivedBr<Change>) {
    while let Ok(recv) = rx.recv().await {
        ws.send(Message::Text(json!(recv).to_string().into()))
            .await
            .unwrap();
    }
}

#[derive(Debug)]
pub enum WatcherParams {
    Event {
        path: PathBuf,
        r#await: Option<u64>,
        ignore_rename_suffix: String,
    },
    Poll {
        path: PathBuf,
        interval: Option<u64>,
    },
}
