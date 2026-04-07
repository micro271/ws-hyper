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
    mpsc::{self, Receiver, Sender},
};

use crate::{
    bucket::{Bucket, key::Key},
    manager::{
        Change,
        utils::{SplitTask, Task},
        websocket::user_tracker::UserTracker,
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
        let mut users = UserTracker::<Change>::new();
        tracing::info!("Web socket manage init");

        loop {
            let msg = self.rx.recv().await;
            tracing::trace!("{msg:?}");
            match msg {
                Some(MsgWs::Change(change)) => {
                    match change {
                        Change::NewObject {
                            bucket,
                            key,
                            object,
                        } => todo!(),
                        Change::NewKey { bucket, key } => todo!(),
                        Change::NewBucket { bucket } => todo!(),
                        Change::NameObject {
                            bucket,
                            key,
                            file_name,
                            to,
                        } => todo!(),
                        Change::NameBucket { from, to } => todo!(),
                        Change::NameKey { bucket, from, to } => todo!(),
                        Change::DeleteObject {
                            bucket,
                            key,
                            file_name,
                        } => todo!(),
                        Change::DeleteKey { bucket, key } => todo!(),
                        Change::DeleteBucket { bucket } => todo!(),
                    }
                    todo!();
                }
                Some(MsgWs::NewUser {
                    bucket,
                    key,
                    sender,
                }) => {
                    let mut rx = users.get_rx(bucket, key);

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
        bucket: Bucket<'static>,
        key: Key<'static>,
        sender: HyperWebsocket,
    },
    Change(Change),
}
