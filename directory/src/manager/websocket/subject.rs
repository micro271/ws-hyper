use crate::manager::websocket::observer::Observer;

pub trait Publisher {
    type Observer: Observer;
    type ObserverId: Send + 'static;

    fn subscriber(&mut self, observer: Self::Observer) -> Self::ObserverId;
    fn unsubscriber(&mut self, observer: Self::ObserverId);
    fn notify(&self, event: <Self::Observer as Observer>::Event);
}
