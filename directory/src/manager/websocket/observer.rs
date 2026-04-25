use crate::{actor::Actor, manager::websocket::WebSocketHandler};
use std::pin::Pin;

pub trait Observer {
    type Event: Send + 'static;
    fn update(&mut self, ev: Self::Event) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
}

pub struct UserObserver(<WebSocketHandler as Actor>::ActorRef);
