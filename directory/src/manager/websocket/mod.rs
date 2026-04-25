use futures::{SinkExt, stream::SplitSink};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{WebSocketStream, tungstenite};
use hyper_util::rt::TokioIo;

use crate::actor::{Actor, ActorRef, ActorRefWithShutdown, Context, Envelope};

pub mod observer;
pub mod subject;

pub struct WebSocketHandler(SplitSink<WebSocketStream<TokioIo<Upgraded>>, tungstenite::Message>);

impl Actor for WebSocketHandler {
    type Message = tungstenite::Message;

    type Reply = ();

    type ActorRef = ActorRefWithShutdown<tokio::sync::mpsc::Sender<Envelope<Self>>, Self>;

    type Context = Context<Self>;

    fn start(mut self) -> Self::ActorRef {
        let (tx, mut rx) = tokio::sync::mpsc::channel(32);
        let (tx_shut, mut rx_shut) = tokio::sync::oneshot::channel();
        let actor_ref = ActorRef::new(tx.clone());

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    message = rx.recv() => {
                        match message {
                            Some(Envelope { message, .. }) => if let Err(er) = self.0.send(message).await {
                                tracing::error!("[ WebSocketHandler ] error: {er:?}");
                            },
                            None => { break; },
                        }
                    },
                    _ = &mut rx_shut => {
                        _ = self.0.close().await;
                        break;
                    }
                }
            }
        });

        ActorRefWithShutdown::new(actor_ref, tx_shut)
    }
}
