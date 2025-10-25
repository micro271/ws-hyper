use std::marker::PhantomData;

use crate::manager::utils::TakeOwn;

use super::{
    EventWatcher, ForDir, PathBuf, RecursiveMode, RenameControl, Watcher, WatcherErr,
    unbounded_channel,
};

#[derive(Debug)]
pub struct EventWatcherBuilder<P, F, Tx> {
    path: P,
    r#await: Option<u64>,
    for_dir: F,
    _phantom: PhantomData<Tx>,
}

impl<P, F, Tx> EventWatcherBuilder<P, F, Tx> {
    pub fn rename_control_await(mut self, r#await: u64) -> Self {
        self.r#await = Some(r#await);
        self
    }

    pub fn path(
        self,
        mut path: PathBuf,
    ) -> Result<EventWatcherBuilder<EventWatcherPath, F, Tx>, WatcherErr> {
        if path.is_relative() {
            path = path
                .canonicalize()
                .map_err(|x| WatcherErr::new(x.to_string()))?;
        }

        Ok(EventWatcherBuilder {
            path: EventWatcherPath(path),
            r#await: self.r#await,
            for_dir: self.for_dir,
            _phantom: self._phantom,
        })
    }

    pub fn for_dir(
        self,
        real_path: String,
        root: String,
    ) -> EventWatcherBuilder<P, EventWatcherForDir<String>, Tx> {
        EventWatcherBuilder {
            path: self.path,
            r#await: self.r#await,
            for_dir: EventWatcherForDir(ForDir::new(root, real_path)),
            _phantom: self._phantom,
        }
    }
}

impl<F, Tx> EventWatcherBuilder<EventWatcherPath, F, Tx> {
    pub fn for_dir_root<T: AsRef<str>>(
        self,
        root: T,
    ) -> EventWatcherBuilder<EventWatcherPath, EventWatcherForDir<String>, Tx> {
        let path = self.path.0.to_str().map(ToString::to_string).unwrap();

        EventWatcherBuilder {
            path: self.path,
            r#await: self.r#await,
            for_dir: EventWatcherForDir(ForDir::new(root.as_ref().to_string(), path)),
            _phantom: self._phantom,
        }
    }
}

impl<Tx> EventWatcherBuilder<EventWatcherPath, EventWatcherForDir<String>, Tx> {
    pub fn build(self) -> Result<EventWatcher<Tx>, WatcherErr> {
        let path = self.path.take();
        let for_dir = self.for_dir.take();
        let r#await = self.r#await.unwrap_or(2000);

        if !path.exists() {
            return Err(WatcherErr::new(format!("Path {path:?} not exists")));
        }

        let (tx, rx) = unbounded_channel();
        let tx_cp = tx.clone();
        let mut notify_watcher = notify::recommended_watcher(move |event| _ = tx_cp.send(event))
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
            for_dir,
            _pantom: self._phantom,
        })
    }
}

pub struct EventWatcherNoForDir;

pub struct EventWatcherForDir<T>(ForDir<T>);

pub struct EventWatcherNoPath;

pub struct EventWatcherPath(PathBuf);

impl<Tx> std::default::Default
    for EventWatcherBuilder<EventWatcherNoPath, EventWatcherNoForDir, Tx>
{
    fn default() -> Self {
        Self {
            path: EventWatcherNoPath,
            r#await: None,
            for_dir: EventWatcherNoForDir,
            _phantom: PhantomData,
        }
    }
}

impl TakeOwn<ForDir<String>> for EventWatcherForDir<String> {
    fn take(self) -> ForDir<String> {
        self.0
    }
}

impl TakeOwn<PathBuf> for EventWatcherPath {
    fn take(self) -> PathBuf {
        self.0
    }
}
