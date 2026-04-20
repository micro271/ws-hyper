mod rename_control;

use notify::{
    INotifyWatcher, Watcher as _,
    event::{CreateKind, Event, ModifyKind, RenameMode},
};
pub use rename_control::*;
use std::path::PathBuf;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::{
    actor::{Actor, ActorRef, Context, Envelope, Handler},
    manager::{
        Manager,
        utils::{
            hd_new_bucket_or_key_watcher, hd_new_object_watcher, hd_rename_object, hd_rename_path,
            skipper::Skipper,
        },
    },
};

pub struct EventWatcher {
    notify_watcher: Option<INotifyWatcher>,
    r#await: u64,
    ref_manager: Option<<Manager as Actor>::Handler>,
    path: PathBuf,
    ref_rename_control: Option<<RenameControl as Actor>::Handler>,
    skipper: Skipper,
}

impl std::fmt::Debug for EventWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventWatcher").finish()
    }
}

impl EventWatcher {
    pub fn new(path: PathBuf) -> Self {
        Self {
            notify_watcher: None,
            r#await: 2000,
            ref_manager: None,
            ref_rename_control: None,
            path,
            skipper: Skipper::default(),
        }
    }

    pub fn set_ref_manager(&mut self, actor_ref: <Manager as Actor>::Handler) {
        self.ref_manager = Some(actor_ref);
    }

    pub fn set_rename_control_await(&mut self, r#await: u64) {
        self.r#await = r#await;
    }
}

impl std::clone::Clone for EventWatcher {
    fn clone(&self) -> Self {
        Self {
            notify_watcher: None,
            r#await: self.r#await,
            ref_manager: None,
            ref_rename_control: None,
            path: self.path.clone(),
            skipper: Skipper::default(),
        }
    }
}

impl Actor for EventWatcher {
    type Message = Event;
    type Reply = ();
    type Context = Context<Self>;
    type Handler = ActorRef<UnboundedSender<Envelope<Self>>, Self>;

    fn start(mut self) -> Self::Handler {
        let (tx, mut rx) = unbounded_channel();
        let tx_0 = tx.clone();
        let self_ref = ActorRef::new(tx);
        let mut notify_w = notify::recommended_watcher(move |ev| match ev {
            Ok(ev) => {
                tracing::info!("[ New Event ]: {ev:?}");
                tx_0.send(Envelope::tell(ev)).unwrap();
            }
            Err(er) => tracing::error!("[ Notify Error ]: {er}"),
        })
        .unwrap();

        notify_w
            .watch(&self.path, notify::RecursiveMode::Recursive)
            .unwrap();

        let rename_control = RenameControl::new(self_ref.clone(), self.r#await);
        self.ref_rename_control = Some(rename_control.start());

        self.notify_watcher = Some(notify_w);

        let mut ctx = Context::new(self_ref.clone());

        tokio::spawn(async move {
            tracing::info!("[ EventWatcher Init ]");
            loop {
                match rx.recv().await {
                    Some(e) => {
                        tracing::debug!("[ EventWatcher Actor ] new message {e:?}");
                        self.handle(e.message, &mut ctx).await
                    }
                    None => todo!(),
                }
            }
        });

        self_ref
    }
}

impl Handler for EventWatcher {
    async fn handle(&mut self, message: Self::Message, _ctx: &mut Self::Context) -> Self::Reply {
        let root = &self.path;
        let event = message;
        match event.kind {
            notify::EventKind::Create(CreateKind::Folder) => {
                let mut paths = event.paths;
                tracing::debug!("[ EventWatcher ] Path: {paths:?}");

                let Some(path) = paths.pop() else {
                    return ();
                };

                match hd_new_bucket_or_key_watcher(path, root, self.skipper.clone()).await {
                    Ok(ch) => {
                        self.ref_manager.as_ref().unwrap().tell(ch).await;
                    }
                    Err(()) => {
                        tracing::error!("[ CreateKinfFolder ] Error")
                    }
                }
            }
            notify::EventKind::Create(action) => {
                tracing::trace!("Event: {event:?}");
                tracing::trace!("Action: {action:?}");
                let mut path = event.paths;

                let Some(path) = path.pop() else {
                    tracing::error!(
                        "[ EventWatcher ] {{ Create file skip }} Path is not present in action.path"
                    );
                    return ();
                };

                match hd_new_object_watcher(path, root, self.skipper.clone()).await {
                    Ok(ch) => {
                        self.ref_manager.as_ref().unwrap().tell(ch).await;
                    }
                    Err(()) => {
                        tracing::error!("[ CreateKinfOther ] Error")
                    }
                }
            }
            notify::EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                let mut path = event.paths;
                let path = path.pop().unwrap();

                tracing::debug!(
                    "[ EventWatcher ] {{ ModifyKind::Name(RenameMode::From) }} {path:?} (Maybe Delete)"
                );

                self.ref_rename_control
                    .as_ref()
                    .unwrap()
                    .tell(Rename::From(RenameFrom::new(path)))
                    .await;
            }
            notify::EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                tracing::trace!("Modify both {:?}", event.paths);
                let mut paths = event.paths;

                let (Some(to), Some(from)) = (paths.pop(), paths.pop()) else {
                    tracing::error!("[ EventWatcher ] we couldn't obtain the paths: [{paths:?}]");
                    return ();
                };

                self.ref_rename_control
                    .as_ref()
                    .unwrap()
                    .tell(Rename::Decline(from.clone()))
                    .await;

                let ch = if to.is_dir() {
                    hd_rename_path(root, from, to, self.skipper.clone()).await
                } else {
                    hd_rename_object(root, from, to, self.skipper.clone()).await
                };

                match ch {
                    Ok(ch) => {
                        self.ref_manager.as_ref().unwrap().tell(ch).await;
                    }
                    Err(()) => {
                        tracing::error!("[ ModifyKind::Rename ] Error")
                    }
                }
            }
            notify::EventKind::Remove(er) => {
                tracing::trace!("{er:?}");
                todo!("soon!")
            }
            _ => {}
        }
    }
}
