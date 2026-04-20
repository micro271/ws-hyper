pub mod user_tracker;
use std::{fmt::Debug, sync::Arc};

use futures::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{HyperWebsocket, WebSocketStream, tungstenite::Message};
use hyper_util::rt::TokioIo;
use serde_json::json;
use tokio::sync::{
    Mutex,
    mpsc::{self, Sender},
};

use crate::{
    actor::{Actor, ActorRef, Envelope, Handler},
    bucket::{Bucket, key::Key},
    manager::{Change, websocket::user_tracker::UserTracker},
};

#[derive(Clone, Debug)]
pub struct WebSocketChSender(Sender<MsgWs>);

impl std::ops::Deref for WebSocketChSender {
    fn deref(&self) -> &Self::Target {
        &self.0
    }

    type Target = Sender<MsgWs>;
}

impl WebSocketChSender {
    pub fn inner(self) -> Sender<MsgWs> {
        self.0
    }
}

pub struct WebSocket {
    users: UserTracker<Change>,
}

impl Actor for WebSocket {
    type Msg = MsgWs;
    type Handler = ActorRef<Sender<Envelope<Self>>, Self>;

    fn start(mut self) -> Self::Handler {
        let (tx, mut rx) = mpsc::channel(128);

        tokio::spawn(async move {
            tracing::info!("[ WebSocket Init ]");
            loop {
                let msg: Envelope<WebSocket> = rx.recv().await.unwrap();
                self.handle(msg.message).await;
            }
        });

        ActorRef::new(tx)
    }
}

impl Handler for WebSocket {
    type Reply = ();

    async fn handle(&mut self, message: Self::Msg) -> Self::Reply {
        match message {
            MsgWs::Change(change) => {
                let bucket = match &change {
                    Change::NewObject { bucket, .. } => bucket.clone(),
                    Change::NewKey { bucket, .. } => bucket.clone(),
                    Change::NewBucket { bucket } => bucket.clone(),
                    Change::NameObject { bucket, .. } => bucket.clone(),
                    Change::NameKey { bucket, .. } => bucket.clone(),
                    Change::DeleteObject { bucket, .. } => bucket.clone(),
                    Change::DeleteKey { bucket, .. } => bucket.clone(),
                    Change::NameBucket { from, .. } => from.clone(),
                    Change::DeleteBucket { bucket } => bucket.clone(),
                };

                let key = match &change {
                    Change::NewObject { key, .. } => Some(key.clone()),
                    Change::NewKey { key, .. } => Some(key.clone()),
                    Change::NameObject { key, .. } => Some(key.clone()),
                    Change::NameKey { from, .. } => Some(from.clone()),
                    Change::DeleteObject { key, .. } => Some(key.clone()),
                    Change::DeleteKey { key, .. } => Some(key.clone()),
                    _ => None,
                };

                self.users.broadcast(bucket, key, change);
            }
            MsgWs::NewUser {
                bucket,
                key,
                sender,
            } => {
                let mut rx = self.users.get_rx(bucket, key);

                let (tx_client, rx_client) = sender.await.unwrap().split();
                let tx_client = Arc::new(Mutex::new(tx_client));
                let tx_client_clone = tx_client.clone();
                tokio::spawn(async move {
                    while let Ok(change) = rx.recv().await {
                        if let Err(err) = tx_client_clone
                            .lock()
                            .await
                            .send(Message::Text(json!(change).to_string().into()))
                            .await
                        {
                            tracing::error!("{err}");
                        }
                    }
                });

                tokio::spawn(Self::client_messages_handler(rx_client, tx_client));
            }
        }
    }
}

impl WebSocket {
    pub fn new() -> Self {
        Self {
            users: UserTracker::<Change>::new(),
        }
    }
    async fn client_messages_handler(
        mut ws: SplitStream<WebSocketStream<TokioIo<Upgraded>>>,
        tx: Arc<Mutex<SplitSink<WebSocketStream<TokioIo<Upgraded>>, Message>>>,
    ) -> Result<(), &'static str> {
        while let Some(Ok(msg)) = ws.next().await {
            match msg {
                Message::Text(txt) => {
                    tracing::debug!("{txt:?}")
                }
                Message::Ping(bytes) => {
                    tracing::debug!("Received ping message: {bytes:02X?}");
                    if let Err(er) = tx.lock().await.send(Message::Pong(bytes)).await {
                        tracing::error!("[WebSocket] Server Sender error {er}");
                    }
                }
                Message::Pong(bytes) => {
                    tracing::debug!("Received pong message: {bytes:02X?}");
                }
                Message::Close(close_frame) => {
                    if let Some(msg) = close_frame {
                        tracing::debug!(
                            "Received close message with code {} and message: {}",
                            msg.code,
                            msg.reason
                        );
                    } else {
                        tracing::debug!("Received close message");
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum MsgWs {
    NewUser {
        bucket: Bucket<'static>,
        key: Key<'static>,
        sender: HyperWebsocket,
    },
    Change(Change),
}
