pub mod utils;
pub mod watcher;
pub mod websocket;

use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{
    RwLock,
    mpsc::{UnboundedSender, unbounded_channel},
};

use crate::{
    actor::{Actor, ActorRef, Context, Envelope, Handler},
    bucket::{Bucket, bucket_map::BucketMap, key::Key, object::Object},
    manager::{
        utils::change_local_storage, watcher::event_watcher::EventWatcher, websocket::WebSocket,
    },
    state::local_storage::LocalStorage,
};

pub struct Manager {
    state: Arc<RwLock<BucketMap<'static>>>,
    ref_ws: Option<<WebSocket as Actor>::Handler>,
    ref_watcher: Option<<EventWatcher as Actor>::Handler>,
    watcher: EventWatcher,
    local_storage: Arc<LocalStorage>,
}

impl Manager {
    pub async fn new(
        state: Arc<RwLock<BucketMap<'static>>>,
        watcher: EventWatcher,
        local_storage: Arc<LocalStorage>,
    ) -> Self {
        Self {
            state,
            ref_watcher: None,
            ref_ws: None,
            watcher,
            local_storage,
        }
    }
}

impl Actor for Manager {
    type Message = Change;
    type Reply = ();
    type Context = Context<Self>;
    type Handler = ActorRef<UnboundedSender<Envelope<Self>>, Self>;

    fn start(mut self) -> Self::Handler {
        let (tx, mut rx) = unbounded_channel();
        let actor_ref_manager = ActorRef::new(tx);

        let mut w = self.watcher.clone();
        w.set_ref_manager(actor_ref_manager.clone());

        self.ref_watcher = Some(w.start());

        let ws = WebSocket::new().start();
        self.ref_ws = Some(ws);
        let mut ctx = Context::new(actor_ref_manager.clone());

        tokio::spawn(async move {
            tracing::info!("[ Manager Init ]");
            loop {
                match rx.recv().await {
                    Some(e) => self.handle(e.message, &mut ctx).await,
                    None => todo!(),
                }
            }
        });

        actor_ref_manager
    }
}

impl Handler for Manager {
    async fn handle(
        &mut self,
        mut message: Self::Message,
        _ctx: &mut Self::Context,
    ) -> Self::Reply {
        tracing::info!("[Scheduler]: New change: {message:?}");
        change_local_storage(&mut message, self.local_storage.clone()).await;
        self.state.write().await.change(message.clone()).await;
        //self.ws(MsgWs::Change(message.clone())).await.unwrap();
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
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
        from: String,
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
