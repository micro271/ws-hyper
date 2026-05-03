pub mod broker;
pub mod observer;
pub mod web_socket_actor;

use std::sync::Arc;

use futures::StreamExt;
use hyper_tungstenite::HyperWebsocket;
use tokio::sync::RwLock;
pub use web_socket_actor::*;

use crate::{
    actor::Actor,
    bucket::{Bucket, bucket_map::BucketMap, key::Key},
};

pub struct WebSocket;

impl WebSocket {
    pub fn build(
        ws: HyperWebsocket,
        bucket: Option<Bucket<'static>>,
        key: Option<Key<'static>>,
        state: Arc<RwLock<BucketMap>>,
    ) {
        tokio::spawn(async move {
            let (tx, mut rx) = match ws.await {
                Ok(ws) => ws.split(),
                Err(er) => {
                    tracing::error!("[ WebSocket ] Handshake error: {er} ");
                    return;
                }
            };

            let Some(broker) = state.read().await.subscriber(bucket, key).await else {
                return;
            };

            let actor_ref = WebSocketHandler {
                user: tx,
                broker: broker,
            }
            .start();

            tokio::spawn(async move {
                loop {
                    match rx.next().await {
                        Some(Ok(msg)) => {
                            tracing::info!("[ WebSocketPeer from Subscriber BucketMap ]: {msg}");
                        }
                        Some(Err(er)) => {
                            tracing::error!(
                                "[ WebSocketPeer from Subscriber BucketMap ] error {er}"
                            );
                        }
                        None => {
                            actor_ref.shutdown().await;
                            break;
                        }
                    }
                }
            });
        });
    }
}
