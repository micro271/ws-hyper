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
        Bucket, Cowed,
        bucket_map::BucketMap,
        key::Key,
        object::Object,
        utils::rename_handlers::{NewObjNameHandlerBuilder, RenameObjHandlerBuilder},
    },
    grpc_v1::ConnectionAuthMS,
    manager::{
        utils::{Run, SplitTask, Task, change_local_storage},
        watcher::{event_watcher::EventWatcherBuilder, pool_watcher::PollWatcherNotify},
        websocker::{MsgWs, WebSocket, WebSocketChSender},
    },
    state::local_storage::LocalStorage,
};

type WsSenderType = SplitSink<WebSocketStream<TokioIo<Upgraded>>, Message>;

#[derive(Debug)]
pub struct ManagerChSenders {
    grpc: ConnectionAuthMS,
    ws: WebSocketChSender,
}

pub struct Manager {
    state: Arc<RwLock<BucketMap<'static>>>,
    tx: UnboundedSender<Change>,
    rx: UnboundedReceiver<Change>,
    grpc: ConnectionAuthMS,
    ws: WebSocket,
    watcher_params: WatcherParams,
    local_storage: Arc<LocalStorage>,
}

pub struct ManagerRunning {
    ws_sender: WebSocketChSender,
    rx: UnboundedReceiver<Change>,
    state: Arc<RwLock<BucketMap<'static>>>,
    grpc: ConnectionAuthMS,
    local_storage: Arc<LocalStorage>,
}

impl Manager {
    pub async fn new(
        state: Arc<RwLock<BucketMap<'static>>>,
        watcher_params: WatcherParams,
        endpoint: Endpoint,
        local_storage: Arc<LocalStorage>,
    ) -> Self {
        let (tx_sch, rx_sch) = unbounded_channel();

        Self {
            state,
            tx: tx_sch.clone(),
            rx: rx_sch,
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

        let (ws_sender, ws_task) = self.ws.split();

        ws_task.run();

        let task = ManagerRunning {
            ws_sender,
            rx: self.rx,
            state: self.state,
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
                    tracing::info!("[Scheduler]: New change: {change:?}");

                    change_local_storage(&mut change, self.local_storage.clone()).await;
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
        bucket: Bucket<'static>,
        key: Key<'static>,
        object: Object,
    },
    NewKey {
        bucket: Bucket<'static>,
        key: Key<'static>,
    },
    NewBucket {
        bucket: Bucket<'static>,
    },
    NameObject {
        bucket: Bucket<'static>,
        key: Key<'static>,
        file_name: String,
        to: String,
    },
    NameBucket {
        from: Bucket<'static>,
        to: Bucket<'static>,
    },
    NameKey {
        bucket: Bucket<'static>,
        from: Key<'static>,
        to: Key<'static>,
    },
    DeleteObject {
        bucket: Bucket<'static>,
        key: Key<'static>,
        file_name: String,
    },
    DeleteKey {
        bucket: Bucket<'static>,
        key: Key<'static>,
    },
    DeleteBucket {
        bucket: Bucket<'static>,
    },
}

#[derive(Debug)]
enum Scope {
    Bucket(Bucket<'static>),
    Key(Bucket<'static>, Key<'static>),
}

impl Change {
    fn scope(&self) -> Scope {
        match self {
            Change::NewObject { bucket, key, .. }
            | Change::NewKey { bucket, key }
            | Change::NameObject { bucket, key, .. }
            | Change::DeleteObject { bucket, key, .. }
            | Change::DeleteKey { bucket, key } => Scope::Key(bucket.cloned(), key.cloned()),
            Change::NameBucket { from, .. } => Scope::Bucket(from.cloned()),
            Change::NameKey { bucket, from, .. } => Scope::Key(bucket.cloned(), from.cloned()),
            Change::NewBucket { bucket } | Change::DeleteBucket { bucket } => {
                Scope::Bucket(bucket.cloned())
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
