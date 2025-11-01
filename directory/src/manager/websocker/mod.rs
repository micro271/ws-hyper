use std::{collections::HashMap, marker::PhantomData, sync::Arc};

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
    broadcast::{self, Receiver, Sender as SenderBr},
};

use crate::{
    bucket::{Bucket, key::Key},
    manager::{Change, Route, utils::AsyncRecv}, 
};

#[derive(Debug, Clone)]
pub struct WebSocker<Rx> {
    _priv: PhantomData<Rx>,
}

impl<Rx: AsyncRecv<Item = MsgWs>> WebSocker<Rx>
where
    Self: Send + 'static,
{
    pub fn run(rx: Rx) {
        tokio::spawn(Self::task(rx));
    }
    pub async fn task(mut rx: Rx) {
        let mut users = ListToNotification::<Change>::new();
        tracing::debug!("Web socket manage init");

        loop {
            let msg = rx.recv().await;
            tracing::trace!("{msg:?}");
            match msg {
                Some(MsgWs::Change(change)) => {
                    match change.route() {
                        Route::Bucket(bucket) => {
                            if let Err(er)  = users.send_all_in_bucket(&bucket).unwrap().send(change) {
                                tracing::error!("");
                            }
                        },
                        Route::Pair(bucket, key) => {
                            if let Err(er) = users.send_message(&bucket, &key).send(change) {
                                tracing::error!("");
                            }
                        },
                    }
                }
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

    pub async fn client_messages_handler(
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

    fn get_sender(&self, bucket: &Bucket, key: &Key) -> Option<SenderBr<T>> {
        self.0
            .get(bucket)
            .and_then(|x| x.get(key).map(|x| x.clone()))
    }
    
    fn send_message(&self, bucket: &Bucket, key: &Key) -> SendMessage<'_, T> {
        let sender = self.0
            .get(bucket)
            .and_then(|x| x.get(key)).unwrap();

        SendMessage(sender)
    }

    pub fn send_all_in_bucket(&self, bucket: &Bucket) -> Result<SendAllBucket<'_, T>,()> {
        Ok(SendAllBucket(self.0.get(bucket).unwrap()))
    }

    fn rcv_or_create(&mut self, bucket: Bucket, key: Key) -> Receiver<T> {
        let bucket = self.0.entry(bucket).or_insert(HashMap::new());
        let key = bucket.entry(key).or_insert_with(|| {
            let (tx, _) = broadcast::channel::<T>(128);
            tx
        });
        key.subscribe()
    }
}

struct SendMessage<'a, T>(&'a SenderBr<T>);

impl<'a, T: Clone> SendMessage<'a, T> {
    pub fn send(self, msj: T) -> Result<(),()> {
        self.0.send(msj.clone());
        Ok(())
    }
}


struct SendAllBucket<'a, T>(&'a HashMap<Key, SenderBr<T>>);

impl<'a, T: Clone> SendAllBucket<'a, T> {
    pub fn send(self, msj: T) -> Result<(),()> {
        for snd in self.0.values() {
            snd.send(msj.clone());
        }
        Ok(())
    }
}