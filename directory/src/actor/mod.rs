use std::{marker::PhantomData, sync::Arc};

use tokio::sync::{
    Mutex,
    mpsc::{UnboundedSender, error::SendError},
    oneshot,
};

pub struct Context<A: Actor> {
    self_ref: A::ActorRef,
}

impl<A: Actor> Context<A> {
    pub fn new(actor_ref: A::ActorRef) -> Self {
        Self {
            self_ref: actor_ref,
        }
    }
}

pub trait ActorContext: Send + 'static {
    type Actor: Actor;
    fn actor_ref(&self) -> &<Self::Actor as Actor>::ActorRef;
}

impl<T: Actor> ActorContext for Context<T> {
    type Actor = T;

    fn actor_ref(&self) -> &<Self::Actor as Actor>::ActorRef {
        &self.self_ref
    }
}

pub trait Actor: Send + 'static {
    type Message: Send + 'static;
    type Reply: Send + 'static;
    type ActorRef: Send + Clone + 'static;
    type Context: ActorContext;

    fn start(self) -> Self::ActorRef;
}

pub struct Shutdown;

pub struct ActorRefWithShutdown<S, A> {
    actor_ref: ActorRef<S, A>,
    shutdown: Arc<Mutex<Option<tokio::sync::oneshot::Sender<Shutdown>>>>,
}

impl<S: Clone, A> std::clone::Clone for ActorRefWithShutdown<S, A> {
    fn clone(&self) -> Self {
        Self {
            actor_ref: self.actor_ref.clone(),
            shutdown: self.shutdown.clone(),
        }
    }
}

impl<S, A> ActorRefWithShutdown<S, A> {
    pub fn new(actor_ref: ActorRef<S, A>, sender: tokio::sync::oneshot::Sender<Shutdown>) -> Self {
        Self {
            actor_ref,
            shutdown: Arc::new(Mutex::new(Some(sender))),
        }
    }

    pub async fn shutdown(self) {
        match self.shutdown.lock().await.take() {
            Some(sender) => {
                if let Err(_) = sender.send(Shutdown) {
                    tracing::error!("[ ActorRefWithShutdown ] Send command error");
                }
            }
            None => tracing::error!("[ ActorRefWithShutdown ] Channel closed"),
        }
    }
}

pub struct ActorRef<S, A> {
    sender: S,
    _actor: PhantomData<A>,
}

impl<S, A> ActorRef<S, A> {
    pub fn new(sender: S) -> Self {
        Self {
            sender,
            _actor: PhantomData,
        }
    }
}

impl<S: Clone, A> std::clone::Clone for ActorRef<S, A> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            _actor: PhantomData,
        }
    }
}

pub trait Handler: Actor + Sized {
    fn handle(
        &mut self,
        message: Self::Message,
        _ctx: &mut Self::Context,
    ) -> impl Future<Output = Self::Reply>;
}

#[derive(Debug)]
pub struct Envelope<A: Actor> {
    pub message: A::Message,
    pub reply_to: Option<tokio::sync::oneshot::Sender<A::Reply>>,
}

impl<A: Actor> Envelope<A> {
    pub fn tell(msg: A::Message) -> Self {
        Self {
            message: msg,
            reply_to: None,
        }
    }

    pub fn ask(msg: A::Message) -> (Self, oneshot::Receiver<A::Reply>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                message: msg,
                reply_to: Some(tx),
            },
            rx,
        )
    }
}

impl<S: SenderActor<H>, H: Handler> ActorRef<S, H> {
    pub async fn tell(&self, msg: H::Message) {
        let _ = self.sender.send(Envelope::tell(msg)).await;
    }

    pub async fn ask(&self, msg: H::Message) -> H::Reply {
        let (msg, rx) = Envelope::ask(msg);
        let _ = self.sender.send(msg).await;
        rx.await.unwrap()
    }
}

pub trait SenderActor<H: Handler> {
    type Error: Send + 'static;
    fn send(&self, msg: Envelope<H>) -> impl Future<Output = Result<(), Self::Error>>;
}

impl<H: Handler> SenderActor<H> for UnboundedSender<Envelope<H>> {
    type Error = SendError<Envelope<H>>;
    async fn send(&self, msg: Envelope<H>) -> Result<(), Self::Error> {
        self.send(msg)
    }
}
