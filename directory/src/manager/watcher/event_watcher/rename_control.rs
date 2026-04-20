use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use notify::{Event, event::RemoveKind};
use tokio::sync::{
    Mutex,
    mpsc::{UnboundedSender, unbounded_channel},
};

use crate::{
    actor::{Actor, ActorRef, Envelope, Handler},
    manager::watcher::event_watcher::EventWatcher,
};

pub struct RenameControlCh(UnboundedSender<Rename>);

impl RenameControlCh {
    pub fn inner(self) -> UnboundedSender<Rename> {
        self.0
    }
}

pub struct RenameControl {
    r#await: u64,
    sender_watcher: <EventWatcher as Actor>::Handler,
    files: Arc<Mutex<HashMap<PathBuf, UnboundedSender<DropDelete>>>>,
}

impl RenameControl {
    pub fn new(sender_watcher: <EventWatcher as Actor>::Handler, r#await: u64) -> Self {
        Self {
            sender_watcher,
            r#await,
            files: Arc::new(Mutex::new(
                HashMap::<PathBuf, UnboundedSender<DropDelete>>::new(),
            )),
        }
    }
}

impl Actor for RenameControl {
    type Msg = Rename;
    type Handler = ActorRef<UnboundedSender<Envelope<Self>>, Self>;

    fn start(mut self) -> Self::Handler {
        let (tx, mut rx) = unbounded_channel();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Some(Envelope { message, .. }) => self.handle(message).await,
                    None => {}
                }
            }
        });

        ActorRef::new(tx)
    }
}

impl Handler for RenameControl {
    type Reply = ();

    async fn handle(&mut self, message: Self::Msg) -> Self::Reply {
        let files_inner = self.files.clone();
        let duration = Duration::from_millis(self.r#await);
        match message {
            Rename::From(RenameFrom(from)) => {
                let (tx_inner, mut rx_inner) = unbounded_channel::<DropDelete>();
                files_inner.lock().await.insert(from.clone(), tx_inner);
                let sender_watcher = self.sender_watcher.clone();

                tokio::spawn(async move {
                    tokio::select! {
                        () = tokio::time::sleep(duration) => {
                            if files_inner.lock().await.remove(&from).is_some() {
                                tracing::trace!("[RenameControl] {{ Time expired }} Delete {from:?}");

                                let event = Event::new(notify::EventKind::Remove(RemoveKind::Any)).add_path(from);
                                sender_watcher.tell(event).await;
                            }
                        }
                        resp = rx_inner.recv() => {
                            tracing::trace!("[RenameControl Inner task] Decline {from:?}");
                            if resp.is_none() {
                                tracing::error!("tx_inner of the RenameControl closed");
                            }
                        }
                    };
                });
            }
            Rename::Decline(path) => {
                if let Some(sender) = files_inner.lock().await.remove(&path) {
                    tracing::trace!("[RenameControl] Decline from Watcher, path: {path:?}");
                    if let Err(err) = sender.send(DropDelete) {
                        tracing::error!("{err}");
                    }
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct DropDelete;

#[derive(Debug)]
pub struct RenameFrom(PathBuf);

impl RenameFrom {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }
}

pub enum Rename {
    From(RenameFrom),
    Decline(PathBuf),
}
