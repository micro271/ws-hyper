use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use futures::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{HyperWebsocket, WebSocketStream, tungstenite::Message};
use hyper_util::rt::TokioIo;
use serde_json::json;
use tokio::sync::{Mutex, broadcast::{self, Receiver, Sender as SenderBr}};

use crate::{
    bucket::{Bucket, key::Key},
    manager::{Change, utils::AsyncRecv},
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
                Some(MsgWs::Change { bucket, key, change }) => {
                    let Some(sender) = users.get_sender(&bucket, &key) else {
                        tracing::error!("Nobody is listening the bucket");
                        continue;
                    };

                    if let Err(er) = sender.send(change) {
                        tracing::error!("[Error to notification] {er}");
                    }
                }
                Some(MsgWs::NewUser { bucket, key, sender }) => {
                    
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
    Change {
        bucket: Bucket,
        key: Key,
        change: Change,
    },
}


struct ListToNotification<T>(HashMap::<Bucket, HashMap<Key, SenderBr<T>>>);

impl<T: Clone + Send + 'static> ListToNotification<T> {
    fn new() -> Self {
        Self(HashMap::new())
    }

    fn get_sender(&self, bucket: &Bucket, key: &Key) -> Option<SenderBr<T>> {
        self.0.get(bucket).and_then(|x| x.get(key).map(|x| x.clone()))
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