use std::{collections::HashMap, marker::PhantomData};

use futures::SinkExt;
use hyper_tungstenite::tungstenite::Message;
use serde_json::json;
use tokio::sync::broadcast::Sender as SenderBr;

use crate::manager::{Change, MsgWs, utils::AsyncRecv};

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
                    if let Some(send) = users.get(&subscriber)
                        && let Err(err) = send.send(change)
                    {
                        tracing::error!("{err}");
                        _ = users.remove(&subscriber);
                    }
                }
                Some(MsgWs::NewUser {
                    subscriber,
                    mut sender,
                }) => {
                    let mut rx = if let Some(subs) = users.get(&subscriber) {
                        subs.subscribe()
                    } else {
                        let (tx, rx) = tokio::sync::broadcast::channel(256);
                        users.insert(subscriber, tx);
                        rx
                    };
                    tokio::spawn(async move {
                        while let Ok(change) = rx.recv().await {
                            if let Err(err) = sender
                                .send(Message::Text(json!(change).to_string().into()))
                                .await
                            {
                                tracing::error!("{err}");
                            }
                        }
                    });
                }
                None => {
                    tracing::debug!("Peer tx_ws closed");
                    break;
                }
            }
        }
    }
}
