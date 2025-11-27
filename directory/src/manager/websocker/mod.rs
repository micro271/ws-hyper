use std::{collections::HashMap, fmt::Debug, sync::Arc};

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
    broadcast::{self, Receiver as ReceivedBr, Sender as SenderBr},
    mpsc::{self, Receiver, Sender},
};

use crate::{
    bucket::{Bucket, key::Key},
    manager::{
        Change, Scope,
        utils::{SplitTask, Task},
    },
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

#[derive(Debug)]
pub struct WebSocket {
    rx: Receiver<MsgWs>,
    tx: Sender<MsgWs>,
}

impl WebSocket {
    pub fn get_sender(&self) -> WebSocketChSender {
        WebSocketChSender(self.tx.clone())
    }
}

impl SplitTask for WebSocket {
    fn split(self) -> (<Self as SplitTask>::Output, impl super::utils::Run) {
        (WebSocketChSender(self.tx.clone()), self)
    }

    type Output = WebSocketChSender;
}

impl Task for WebSocket {
    async fn task(mut self)
    where
        Self: Sized,
    {
        let mut users = ListToNotification::<Change>::new();
        tracing::info!("Web socket manage init");

        loop {
            let msg = self.rx.recv().await;
            tracing::trace!("{msg:?}");
            match msg {
                Some(MsgWs::Change(change)) => match change.scope() {
                    Scope::Bucket(bucket) => match users.send_all_in_bucket(&bucket) {
                        Err(er) => tracing::error!("No one is listening the bucket {er}"),
                        Ok(snd) => {
                            if let Err(er) = snd.send(change) {
                                tracing::error!(
                                    "We had a problem sending the message to this keys {er:?}"
                                );
                            }
                        }
                    },
                    Scope::Key(bucket, key) => match users.send_message(&bucket, &key) {
                        Err(er) => tracing::error!("No one is listening the bucket {er}"),
                        Ok(snd) => {
                            if let Err(er) = snd.send(change) {
                                tracing::error!(
                                    "We had a problem sending the message to this key {er:?}"
                                );
                            }
                        }
                    },
                },
                Some(MsgWs::NewUser {
                    bucket,
                    key,
                    sender,
                }) => {
                    let mut rx = users.rcv_or_create(bucket, key);

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
                _ => {
                    tracing::debug!("Peer tx_ws closed");
                    break;
                }
            }
        }
    }
}

impl WebSocket {
    pub fn new() -> Self {
        Self::default()
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

impl std::default::Default for WebSocket {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel(128);
        Self { rx, tx }
    }
}

#[derive(Debug)]
pub enum MsgWs {
    NewUser {
        bucket: Bucket,
        key: Key,
        sender: HyperWebsocket,
    },
    Change(Change),
}

struct ListToNotification<T>(HashMap<Bucket, HashMap<Key, SenderBr<T>>>);

impl<T: Clone + Send + 'static> ListToNotification<T> {
    fn new() -> Self {
        Self(HashMap::new())
    }

    fn send_message<'a>(
        &'a self,
        bucket: &'a Bucket,
        key: &'a Key,
    ) -> Result<SendMessage<'a, T>, ListToNotificationError<'a, T>> {
        let sender = self
            .0
            .get(bucket)
            .ok_or(ListToNotificationError::BucketNotFound(bucket.to_string()))
            .and_then(|x| {
                x.get(key)
                    .ok_or(ListToNotificationError::KeyNotFound(key.name()))
            })?;

        Ok(SendMessage(sender))
    }

    pub fn send_all_in_bucket<'a>(
        &'a self,
        bucket: &'a Bucket,
    ) -> Result<SendAllBucket<'a, T>, ListToNotificationError<'a, T>> {
        Ok(SendAllBucket(self.0.get(bucket).ok_or(
            ListToNotificationError::BucketNotFound(bucket.to_string()),
        )?))
    }

    fn rcv_or_create(&mut self, bucket: Bucket, key: Key) -> ReceivedBr<T> {
        let bucket = self.0.entry(bucket).or_default();
        let key = bucket.entry(key).or_insert_with(|| {
            let (tx, _) = broadcast::channel::<T>(128);
            tx
        });
        key.subscribe()
    }
}

struct SendMessage<'a, T>(&'a SenderBr<T>);

impl<'a, T: Clone> SendMessage<'a, T> {
    pub fn send(self, msj: T) -> Result<usize, broadcast::error::SendError<T>> {
        self.0.send(msj.clone())
    }
}

struct SendAllBucket<'a, T>(&'a HashMap<Key, SenderBr<T>>);

impl<'a, T: Clone> SendAllBucket<'a, T> {
    pub fn send(self, msj: T) -> Result<(), Vec<(Key, broadcast::error::SendError<T>)>> {
        let mut err = Vec::new();
        for (key, snd) in self.0 {
            if let Err(er) = snd.send(msj.clone()) {
                err.push((key.clone(), er));
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum ListToNotificationError<'a, T> {
    BucketNotFound(String),
    KeyNotFound(&'a str),
    SendError(broadcast::error::SendError<T>),
}

impl<'a, T: Debug> std::error::Error for ListToNotificationError<'a, T> {}

impl<'a, T> std::fmt::Display for ListToNotificationError<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListToNotificationError::BucketNotFound(bucket) => {
                write!(f, "Bucket {bucket:?} not found")
            }
            ListToNotificationError::KeyNotFound(key) => write!(f, "Bucket {key:?} not found"),
            ListToNotificationError::SendError(send_error) => write!(f, "{send_error}"),
        }
    }
}
