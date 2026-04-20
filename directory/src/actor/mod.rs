use std::marker::PhantomData;

use tokio::sync::{
    mpsc::{UnboundedSender, error::SendError},
    oneshot,
};

pub trait Actor: Send + 'static {
    type Msg: Send + 'static;
    type Handler: Send + 'static;

    fn start(self) -> Self::Handler;
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

pub trait Handler: Actor {
    type Reply: Send + 'static;

    fn handle(&mut self, message: Self::Msg) -> impl Future<Output = Self::Reply>;
}

#[derive(Debug)]
pub struct Envelope<H: Handler> {
    pub message: H::Msg,
    pub reply_to: Option<tokio::sync::oneshot::Sender<H::Reply>>,
}

impl<H: Handler> Envelope<H> {
    pub fn tell(msg: H::Msg) -> Self {
        Self {
            message: msg,
            reply_to: None,
        }
    }

    pub fn ask(msg: H::Msg) -> (Self, oneshot::Receiver<H::Reply>) {
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
    pub async fn tell(&self, msg: H::Msg) {
        let _ = self.sender.send(Envelope::tell(msg)).await;
    }

    pub async fn ask(&self, msg: H::Msg) -> H::Reply {
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
