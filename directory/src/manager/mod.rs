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
    mpsc::{Sender, UnboundedReceiver, channel, unbounded_channel},
};

use crate::{
    bucket::{
        Bucket,
        bucket_map::BucketMap,
        key::Key,
        object::{Object, ObjectName},
    },
    grpc_v1::InfoUserGrpc,
    manager::{
        utils::{Run, SplitTask},
        watcher::{event_watcher::EventWatcherBuilder, pool_watcher::PollWatcherNotify},
        websocker::{MsgWs, WebSocker},
    },
    state::pg_listen::{ListenBucket, ListenBucketCh},
};

type WsSenderType = SplitSink<WebSocketStream<TokioIo<Upgraded>>, Message>;

#[derive(Debug)]
pub struct Manager {
    tx_ws: RwLock<Sender<MsgWs>>,
    pub state: Arc<RwLock<BucketMap>>,
}

impl Manager {
    pub async fn run(
        state: Arc<RwLock<BucketMap>>,
        watcher: WatcherParams,
        endpoint: Endpoint,
        listen_bk: ListenBucket,
    ) -> (Sender<MsgWs>, InfoUserGrpc) {
        let (tx_ws, rx_ws) = channel(128);
        let (tx_sch, rx_sch) = unbounded_channel();

        let tx_sch_clone = tx_sch.clone();

        let grpc_client = InfoUserGrpc::new(endpoint, tx_sch_clone).await;
        let resp = tx_ws.clone();

        let (listener_ch, lst_task) = listen_bk.split();

        let myself = Arc::new(Self {
            tx_ws: RwLock::new(tx_ws),
            state,
        });

        match watcher {
            WatcherParams::Event { path, r#await } => {
                let task = EventWatcherBuilder::default()
                    .path(path)
                    .unwrap()
                    .change_notify(tx_sch)
                    .rename_control_await(r#await.unwrap_or(2000))
                    .build()
                    .unwrap();
                task.run();
            }
            WatcherParams::Poll { path, interval } => {
                let task = PollWatcherNotify::new(path, interval.unwrap_or_default()).unwrap();
                task.run();
            }
        }
        lst_task.run();
        WebSocker::new(rx_ws).run();

        grpc_client.run_stream().await;
        tokio::task::spawn(myself.clone().run_scheduler_mg(rx_sch, listener_ch));
        (resp, grpc_client)
    }

    async fn run_scheduler_mg(
        self: Arc<Self>,
        mut rx_watcher: UnboundedReceiver<Change>,
        lst: ListenBucketCh,
    ) {
        tracing::info!("Scheduler init");
        let tx_ws = self.tx_ws.read().await.clone();

        loop {
            match rx_watcher.recv().await {
                Some(change) => {
                    self.state.write().await.change(change.clone()).await;
                    tx_ws.send(MsgWs::Change(change.clone())).await.unwrap();
                    lst.send(change).await.unwrap();
                }
                None => {
                    tracing::error!("[Scheduler] Peer: tx_watcher closed");
                    break;
                }
            }
        }
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
        from: ObjectName<'static>,
        to: Object,
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
        object: Object,
    },
    DeleteKey {
        bucket: Bucket,
        key: Key,
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
            Change::NewBucket { bucket } => Scope::Bucket(bucket.clone()),
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

pub enum WatcherParams {
    Event {
        path: PathBuf,
        r#await: Option<u64>,
    },
    Poll {
        path: PathBuf,
        interval: Option<u64>,
    },
}
