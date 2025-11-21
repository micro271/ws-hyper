use notify::{Error, Event};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::manager::{
    Change,
    utils::{OneshotSender, TakeOwn},
};

use super::{
    EventWatcher, PathBuf, RecursiveMode, RenameControl, Watcher, WatcherErr, unbounded_channel,
};

#[derive(Debug)]
pub struct EventWatcherBuilder<P, CN> {
    path: P,
    r#await: Option<u64>,
    change_notify: CN,
}

impl<P, CN> EventWatcherBuilder<P, CN> {
    pub fn rename_control_await(mut self, r#await: u64) -> Self {
        self.r#await = Some(r#await);
        self
    }
}

impl<P> EventWatcherBuilder<P, EventWatcherNoNotify> {
    pub fn change_notify<Tx>(self, tx: Tx) -> EventWatcherBuilder<P, EventWatcherNotify<Tx>> {
        EventWatcherBuilder {
            path: self.path,
            r#await: self.r#await,
            change_notify: EventWatcherNotify(tx),
        }
    }
}

impl<CN> EventWatcherBuilder<EventWatcherNoPath, CN> {
    pub fn path(
        self,
        mut path: PathBuf,
    ) -> Result<EventWatcherBuilder<EventWatcherPath, CN>, WatcherErr> {
        if path.is_relative() {
            path = path
                .canonicalize()
                .map_err(|x| WatcherErr::new(x.to_string()))?;
        }

        Ok(EventWatcherBuilder {
            path: EventWatcherPath(path),
            r#await: self.r#await,
            change_notify: self.change_notify,
        })
    }
}

impl<Tx> EventWatcherBuilder<EventWatcherPath, EventWatcherNotify<Tx>>
where
    Tx: OneshotSender<Item = Change>,
{
    pub fn build(
        self,
    ) -> Result<
        EventWatcher<
            UnboundedSender<Result<Event, Error>>,
            UnboundedReceiver<Result<Event, Error>>,
            Tx,
        >,
        WatcherErr,
    > {
        let path = self.path.take();
        let r#await = self.r#await.unwrap_or(2000);

        if !path.exists() {
            return Err(WatcherErr::new(format!("Path {path:?} not exists")));
        }

        let (tx, rx) = unbounded_channel();
        let tx_cp = tx.clone();
        let mut notify_watcher = notify::recommended_watcher(move |event| {
            if let Err(err) = tx_cp.send(event) {
                tracing::error!("[Inner Task Notify] err {err}");
            }
        })
        .map_err(|x| WatcherErr::new(x.to_string()))?;

        notify_watcher
            .watch(&path, RecursiveMode::Recursive)
            .map_err(|x| WatcherErr::new(x.to_string()))?;

        let rename_control = RenameControl::new(tx.clone(), self.r#await.unwrap_or(r#await));

        Ok(EventWatcher {
            _notify_watcher: notify_watcher,
            rename_control,
            tx,
            rx,
            path: path.to_string_lossy().into_owned(),
            change_notify: self.change_notify.take(),
        })
    }
}

pub struct EventWatcherNoPath;

pub struct EventWatcherPath(PathBuf);

pub struct EventWatcherNoNotify;

pub struct EventWatcherNotify<T>(T);

impl std::default::Default for EventWatcherBuilder<EventWatcherNoPath, EventWatcherNoNotify> {
    fn default() -> Self {
        Self {
            path: EventWatcherNoPath,
            r#await: None,
            change_notify: EventWatcherNoNotify,
        }
    }
}

impl TakeOwn<PathBuf> for EventWatcherPath {
    fn take(self) -> PathBuf {
        self.0
    }
}

impl<T: Send + 'static> TakeOwn<T> for EventWatcherNotify<T> {
    fn take(self) -> T {
        self.0
    }
}
