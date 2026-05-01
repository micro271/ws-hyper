pub mod broker;
pub mod observer;

use futures::{SinkExt, stream::SplitSink};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{WebSocketStream, tungstenite};
use hyper_util::rt::TokioIo;

use crate::{
    actor::{Actor, ActorRef, ActorRefWithShutdown, Context, Envelope},
    manager::websocket::{
        broker::{WSBroker, WSBrokerMessage},
        observer::UserObserver,
    },
};

pub struct WebSocketHandler {
    pub user: SplitSink<WebSocketStream<TokioIo<Upgraded>>, tungstenite::Message>,
    pub broker: <WSBroker as Actor>::ActorRef,
}

impl Actor for WebSocketHandler {
    type Message = tungstenite::Message;
    type Reply = ();
    type ActorRef = ActorRefWithShutdown<tokio::sync::mpsc::Sender<Envelope<Self>>, Self>;
    type Context = Context<Self>;

    fn start(mut self) -> Self::ActorRef {
        let (tx, mut rx) = tokio::sync::mpsc::channel(32);
        let (tx_shut, mut rx_shut) = tokio::sync::oneshot::channel();
        let actor_ref = ActorRefWithShutdown::new(ActorRef::new(tx.clone()), tx_shut);
        let actor_ref_clone = actor_ref.clone();
        let user_obs = UserObserver::new(actor_ref.clone());

        tokio::spawn(async move {
            let _context = Context::<Self>::new(actor_ref_clone);
            let id = self.broker.ask(WSBrokerMessage::Subscriber(user_obs)).await;
            loop {
                tokio::select! {
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
            self.broker.tell(WSBrokerMessage::Ubsubscriber(id)).await;
        });

        actor_ref
    }
}

impl std::fmt::Debug for WebSocketHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WebSocketHandler {{ .. }}")
    }
}
