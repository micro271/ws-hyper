pub mod error;
pub mod manager;

pub trait Konnection {
    type Message: Send + 'static + std::fmt::Debug + Clone;
    type Future<F>: Send + 'static + Future<Output = F>;
    fn ping(&self) -> Self::Future<Ping>;
    fn handler(&self, msj: Self::Message) -> Self::Future<Result<Self::Message, error::Error>>;
}

#[derive(Debug, PartialEq, PartialOrd)]
pub enum Ping {
    Pong,
    Loss,
}
