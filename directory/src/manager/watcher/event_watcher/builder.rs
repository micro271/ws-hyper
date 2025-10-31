use notify::{Error, Event};
use std::marker::PhantomData;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::manager::utils::TakeOwn;

use super::{
    EventWatcher, PathBuf, RecursiveMode, RenameControl, Watcher, WatcherErr, unbounded_channel,
};

#[derive(Debug)]
pub struct EventWatcherBuilder<P, Tx> {
    path: P,
    r#await: Option<u64>,
    _phantom: PhantomData<Tx>,
}

impl<P, Tx> EventWatcherBuilder<P, Tx> {
    pub fn rename_control_await(mut self, r#await: u64) -> Self {
        self.r#await = Some(r#await);
        self
    }

    pub fn path(
        self,
        mut path: PathBuf,
    ) -> Result<EventWatcherBuilder<EventWatcherPath, Tx>, WatcherErr> {
        if path.is_relative() {
            path = path
                .canonicalize()
                .map_err(|x| WatcherErr::new(x.to_string()))?;
        }

        Ok(EventWatcherBuilder {
            path: EventWatcherPath(path),
            r#await: self.r#await,
            _phantom: self._phantom,
        })
    }
}

impl<Tx> EventWatcherBuilder<EventWatcherPath, Tx> {
    pub fn build(
        self,
    ) -> Result<
        EventWatcher<
            Tx,
            UnboundedSender<Result<Event, Error>>,
            UnboundedReceiver<Result<Event, Error>>,
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
            path: path.to_str().map(ToString::to_string).unwrap(),
            _pantom: self._phantom,
        })
    }
}

pub struct EventWatcherNoForDir;

pub struct EventWatcherNoPath;

pub struct EventWatcherPath(PathBuf);

impl<Tx> std::default::Default for EventWatcherBuilder<EventWatcherNoPath, Tx> {
    fn default() -> Self {
        Self {
            path: EventWatcherNoPath,
            r#await: None,
            _phantom: PhantomData,
        }
    }
}

impl TakeOwn<PathBuf> for EventWatcherPath {
    fn take(self) -> PathBuf {
        self.0
    }
}
