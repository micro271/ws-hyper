use std::collections::HashMap;

use hyper_tungstenite::tungstenite;
use uuid::Uuid;

use crate::{
    actor::{Actor, ActorRef, Context, Envelope},
    manager::websocket::observer::{Observer, UserObserver},
};

#[derive(Default)]
pub struct WSBroker {
    observers: HashMap<uuid::Uuid, UserObserver>,
}

impl Actor for WSBroker {
    type Message = WSBrokerMessage;
    type Reply = uuid::Uuid;
    type ActorRef = ActorRef<tokio::sync::mpsc::Sender<Envelope<Self>>, Self>;
    type Context = Context<Self>;

    fn start(mut self) -> Self::ActorRef {
        let (tx, mut rx) = tokio::sync::mpsc::channel(32);

        let actor = ActorRef::new(tx);
        let actor_0 = actor.clone();

        tokio::spawn(async move {
            let _ctx = Context::<Self>::new(actor_0);

            loop {
                match rx.recv().await {
                    Some(Envelope {
                        message: WSBrokerMessage::Ubsubscriber(id),
                        ..
                    }) => {
                        self.observers.remove(&id);
                    }
                    Some(Envelope {
                        message: WSBrokerMessage::Subscriber(new_user),
                        reply_to,
                    }) => {
                        let uuid = Uuid::new_v4();
                        match reply_to {
                            Some(repl) => {
                                if repl.send(uuid).is_err() {
                                    tracing::error!("[ Broker ] reply error");
                                }
                                self.observers.insert(uuid, new_user);
                            }
                            None => {
                                tracing::debug!("[ Broker ] Nothing reply");
                            }
                        }
                    }
                    Some(Envelope {
                        message: WSBrokerMessage::Message(msg),
                        ..
                    }) => {
                        tracing::debug!(
                            "[ WSBroker ] Send message {{ {msg:?} }} to {} observers",
                            self.observers.len()
                        );
                        for (_, i) in &mut self.observers {
                            i.update(msg.clone()).await;
                        }
                    }

                    None => {}
                }
            }
        });

        actor
    }
}

pub enum WSBrokerMessage {
    Subscriber(UserObserver),
    Ubsubscriber(uuid::Uuid),
    Message(tungstenite::Message),
}
