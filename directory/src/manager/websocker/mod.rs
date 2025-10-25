use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use futures::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{HyperWebsocket, WebSocketStream, tungstenite::Message};
use hyper_util::rt::TokioIo;
use serde_json::json;
use tokio::sync::{Mutex, broadcast::Sender as SenderBr};

use crate::manager::{Change, utils::AsyncRecv};

#[derive(Debug, Clone)]
pub struct WebSocker<Rx> {
    _priv: PhantomData<Rx>,
}

impl<Rx: AsyncRecv<Item = MsgWs>> WebSocker<Rx>
where
    Self: Send + 'static,
{
    pub async fn run(rx: Rx) {
        tokio::spawn(Self::task(rx));
    }
    pub async fn task(mut rx: Rx) {
        let mut users = HashMap::<String, SenderBr<Change>>::new();
        tracing::debug!("Web socket manage init");
        loop {
            let msg = rx.recv().await;
            tracing::trace!("{msg:?}");
            match msg {
                Some(MsgWs::Change { subscriber, change }) => {
                    if let Some(send) = users.get(&subscriber) {
                        match send.send(change) {
                            Ok(n) => {
                                if n == 0 {
                                    users.remove(&subscriber);
                                }
                            }
                            Err(err) => {
                                tracing::error!("{err}");
                            }
                        }
                    }
                }
                Some(MsgWs::NewUser { subscriber, sender })  => {
                    let mut rx = if let Some(subs) = users.get(&subscriber) {
                        subs.subscribe()
                    } else {
                        let (tx, rx) = tokio::sync::broadcast::channel(256);
                        users.insert(subscriber, tx.clone());
                        rx
                    };
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
                    let path = txt.strip_prefix("subscribe: ");
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
        subscriber: String,
        sender: HyperWebsocket,
    },
    Change {
        subscriber: String,
        change: Change,
    }
}