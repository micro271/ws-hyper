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
use std::sync::Arc;
use watcher::Watcher;

use serde_json::json;
use tokio::sync::{
    RwLock,
    broadcast::Receiver as ReceivedBr,
    mpsc::{Sender, UnboundedReceiver, UnboundedSender, channel, unbounded_channel},
};
use utils::Executing;

use crate::{
    bucket::{
        Bucket,
        bucket_map::BucketMap,
        key::Key,
        object::{Object, ObjectName},
    },
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
                Some(change) => {
                    self.state.write().await.change(change.clone()).await;
                    if let Err(er) = tx_ws.send(MsgWs::Change(change)).await {
                        tracing::error!("{er}");
                    }
                }
                None => {
                    tracing::error!("[Scheduler] Peer: tx_watcher closed");
                    break;
                }
            }
        }
    }

    pub async fn add_watcher(&mut self, bucket: Bucket, key: Key, ws: HyperWebsocket) {
        _ = self
            .tx_ws
            .send(MsgWs::NewUser {
                bucket,
                key,
                sender: ws,
            })
            .await;
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
