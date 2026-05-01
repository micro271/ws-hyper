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
    bucket::{
        Bucket, Cowed,
        bucket_map::BucketMap,
        key::{Key, Segment},
        object::Object,
    },
    manager::{utils::change_local_storage, watcher::event_watcher::EventWatcher},
    state::local_storage::LocalStorage,
};

pub struct Manager {
    state: Arc<RwLock<BucketMap>>,
    ref_watcher: Option<<EventWatcher as Actor>::ActorRef>,
    watcher: EventWatcher,
    local_storage: Arc<LocalStorage>,
}

impl Manager {
    pub async fn new(
        state: Arc<RwLock<BucketMap>>,
        watcher: EventWatcher,
        local_storage: Arc<LocalStorage>,
    ) -> Self {
        Self {
            state,
            ref_watcher: None,
            watcher,
            local_storage,
        }
    }
}

impl Actor for Manager {
    type Message = ManagerMessage;
    type Reply = ManagerReply;
    type Context = Context<Self>;
    type ActorRef = ActorRef<UnboundedSender<Envelope<Self>>, Self>;

    fn start(mut self) -> Self::ActorRef {
        let (tx, mut rx) = unbounded_channel();
        let actor_ref_manager = ActorRef::new(tx);

        let mut w = self.watcher.clone();
        w.set_ref_manager(actor_ref_manager.clone());

        self.ref_watcher = Some(w.start());

        let mut ctx = Context::new(actor_ref_manager.clone());

        tokio::spawn(async move {
            tracing::info!("[ Manager Init ]");
            loop {
                match rx.recv().await {
                    Some(Envelope { message, reply_to }) => {
                        let reply = self.handle(message, &mut ctx).await;
                        if let Some(reply_to) = reply_to {
                            if let Err(_er) = reply_to.send(reply) {
                                tracing::error!("[ ManagerActor ] error reply");
                            }
                        }
                    }
                    None => todo!(),
                }
            }
        });

        actor_ref_manager
    }
}

impl Handler for Manager {
    async fn handle(&mut self, message: Self::Message, _ctx: &mut Self::Context) -> Self::Reply {
        match message {
            ManagerMessage::Change(mut change) => {
                tracing::info!("[Scheduler]: New change: {change:?}");
                change_local_storage(&mut change, self.local_storage.clone()).await;
                self.state.write().await.change(change.clone()).await;
                //self.ws(MsgWs::Change(message.clone())).await.unwrap();
                ManagerReply::None
            }
            ManagerMessage::Ask(ManagerAsk::WhatIs(path)) => {
                let tree = self.state.read().await;
                let root = tree.path();
                if path.parent().is_some_and(|x| x == root) {
                    ManagerReply::IsDir
                } else {
                    let bucket = Bucket::find_bucket(root, &path).unwrap();

                    let key = Key::from_bucket(bucket.borrow(), &path).unwrap();

                    if tree.get_entry(&bucket, &key).is_some() {
                        ManagerReply::IsDir
                    } else {
                        ManagerReply::IsFile
                    }
                }
            }
        }
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
        to: Segment<'static>,
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

pub enum ManagerMessage {
    Change(Change),
    Ask(ManagerAsk),
}

pub enum ManagerAsk {
    WhatIs(PathBuf),
}

pub enum ManagerReply {
    None,
    IsDir,
    IsFile,
}
