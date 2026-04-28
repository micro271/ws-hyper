use futures::{SinkExt, stream::SplitSink};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{WebSocketStream, tungstenite};
use hyper_util::rt::TokioIo;

use crate::actor::{self, Actor, ActorRef, ActorRefWithShutdown, Context, Envelope};

pub mod observer;
pub mod subject;

pub struct WebSocketHandler {
    user: SplitSink<WebSocketStream<TokioIo<Upgraded>>, tungstenite::Message>,
    receiver_broadcast: tokio::sync::broadcast::Receiver<<Self as Actor>::Message>,
}

impl Actor for WebSocketHandler {
    type Message = tungstenite::Message;

    type Reply = ();

    type ActorRef = ActorRefWithShutdown<tokio::sync::mpsc::Sender<Envelope<Self>>, Self>;

    type Context = Context<Self>;

    fn start(mut self) -> Self::ActorRef {
        let (tx, mut rx) = tokio::sync::mpsc::channel(32);
        let (tx_shut, mut rx_shut) = tokio::sync::oneshot::channel();
        let actor_ref = ActorRef::new(tx.clone());
        let actor_ref = ActorRefWithShutdown::new(actor_ref, tx_shut);
        let actor_ref_clone = actor_ref.clone();
        tokio::spawn(async move {
            let _context = Context::<Self>::new(actor_ref_clone);

            loop {
                tokio::select! {
                    br_message = self.receiver_broadcast.recv() => {
                        tracing::debug!("[ WebSocketHandler ] New message from broadcast: {br_message:?}");
                        match br_message {
                            Ok(msg) => {
                                if let Err(er) = self.user.send(msg).await {
                                    tracing::error!("[ WebSocketHandler ] error: {er:?}");
                                    break;
                                }
                            },
                            Err(er) => {
                                tracing::error!("[ WebSocketHandler ] Receiver's broadcast error: {er:?}");
                                if let tokio::sync::broadcast::error::RecvError::Closed = er {
                                    break;
                                }
                            }
                        }
                    },
                    message = rx.recv() => {
                        tracing::debug!("[ WebSocketHandler ] New message from Actor receiver: {message:?}");
                        match message {
                            Some(Envelope { message, .. }) => if let Err(er) = self.user.send(message).await {
                                tracing::error!("[ WebSocketHandler ] error: {er:?}");
                                break;
                            },
                            None => { break; },
                        }
                    },
                    _ = &mut rx_shut => {
                        _ = self.user.close().await;
                        break;
                    }
                }
            }
        });

        actor_ref
    }
}

impl std::fmt::Debug for WebSocketHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WebSocketHandler {{ .. }}")
    }
}
