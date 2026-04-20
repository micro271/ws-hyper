use std::{collections::HashMap, path::PathBuf, time::Duration};

use notify::{Event, event::RemoveKind};
use tokio::{
    sync::mpsc::{UnboundedSender, unbounded_channel},
    task::JoinHandle,
};

use crate::{
    actor::{Actor, ActorContext, ActorRef, Context, Envelope, Handler},
    manager::watcher::event_watcher::EventWatcher,
};

pub struct RenameControl {
    r#await: u64,
    sender_watcher: <EventWatcher as Actor>::Handler,
    tasks: HashMap<PathBuf, JoinHandle<()>>,
}

impl RenameControl {
    pub fn new(sender_watcher: <EventWatcher as Actor>::Handler, r#await: u64) -> Self {
        Self {
            sender_watcher,
            r#await,
            tasks: HashMap::new(),
        }
    }
}

impl Actor for RenameControl {
    type Reply = ();
    type Message = Rename;
    type Context = Context<Self>;
    type Handler = ActorRef<UnboundedSender<Envelope<Self>>, Self>;

    fn start(mut self) -> Self::Handler {
        let (tx, mut rx) = unbounded_channel();
        let self_ref = ActorRef::new(tx);
        let mut ctx = Context::new(self_ref.clone());
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Some(Envelope { message, .. }) => {
                        self.handle(message, &mut ctx).await;
                    }
                    None => {}
                }
            }
        });

        self_ref
    }
}

impl Handler for RenameControl {
    async fn handle(&mut self, message: Self::Message, _ctx: &mut Self::Context) -> Self::Reply {
        let duration = Duration::from_millis(self.r#await);
        match message {
            Rename::From(RenameFrom(from)) => {
                let sender_watcher = self.sender_watcher.clone();
                let from_ = from.clone();
                let self_ref = _ctx.actor_ref();
                let handler = tokio::spawn(async move {
                    tokio::time::sleep(duration).await;
                    let event = Event::new(notify::EventKind::Remove(RemoveKind::Any))
                        .add_path(from.clone());
                    sender_watcher.tell(event).await;
                    self_ref.tell(Rename::Expire(from)).await;
                });

                self.tasks.insert(from_, handler);
            }
            Rename::Decline(path) => {
                if let Some(handler) = self.tasks.remove(&path) {
                    tracing::trace!("[RenameControl] Decline from Watcher, path: {path:?}");
                    handler.abort();
                } else {
                    tracing::warn!("[ RenameControl ] {path:?} Nothing task found");
                }
            }
            Rename::Expire(path) => match self.tasks.remove(&path) {
                Some(_) => tracing::trace!("[ RenameControl ] Expired {path:?}"),
                None => tracing::warn!("[ RenameControl ] Expire for unknown path {path:?}"),
            },
        }
    }
}

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
    Expire(PathBuf),
}
