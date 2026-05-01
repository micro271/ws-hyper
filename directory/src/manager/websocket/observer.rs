use std::pin::Pin;

use futures::FutureExt;
use hyper_tungstenite::tungstenite;

use crate::{actor::Actor, manager::websocket::WebSocketHandler};

pub trait Observer {
    type Event: Send + 'static;
    fn update(&mut self, ev: Self::Event) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

pub struct UserObserver(<WebSocketHandler as Actor>::ActorRef);

impl std::fmt::Debug for UserObserver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UserObserver {{ .. }}")
    }
}

impl Observer for UserObserver {
    type Event = tungstenite::Message;

    fn update(&mut self, ev: Self::Event) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        self.0.tell(ev).boxed()
    }
}
