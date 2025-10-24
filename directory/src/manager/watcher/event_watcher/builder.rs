use crate::manager::utils::TakeOwn;

use super::{
    EventWatcher, ForDir, PathBuf, RecursiveMode, RenameControl, Watcher, WatcherErr,
    unbounded_channel,
};

#[derive(Debug)]
pub struct EventWatcherBuilder<P, F> {
    path: P,
    r#await: Option<u64>,
    for_dir: F,
}

impl<P, F> EventWatcherBuilder<P, F> {
    pub fn rename_control_await(mut self, r#await: u64) -> Self {
        self.r#await = Some(r#await);
        self
    }

    pub fn path(
        self,
        mut path: PathBuf,
    ) -> Result<EventWatcherBuilder<EventWatcherPath, F>, WatcherErr> {
        if path.is_relative() {
            path = path
                .canonicalize()
                .map_err(|x| WatcherErr::new(x.to_string()))?;
        }

        Ok(EventWatcherBuilder {
            path: EventWatcherPath(path),
            r#await: self.r#await,
            for_dir: self.for_dir,
        })
    }

    pub fn for_dir(
        self,
        real_path: String,
        root: String,
    ) -> EventWatcherBuilder<P, EventWatcherForDir<String>> {
        EventWatcherBuilder {
            path: self.path,
            r#await: self.r#await,
            for_dir: EventWatcherForDir(ForDir::new(root, real_path)),
        }
    }
}

impl<F> EventWatcherBuilder<EventWatcherPath, F> {
    pub fn for_dir_root<T: AsRef<str>>(
        self,
        root: T,
    ) -> EventWatcherBuilder<EventWatcherPath, EventWatcherForDir<String>> {
        let path = self.path.0.to_str().map(ToString::to_string).unwrap();

        EventWatcherBuilder {
            path: self.path,
            r#await: self.r#await,
            for_dir: EventWatcherForDir(ForDir::new(root.as_ref().to_string(), path)),
        }
    }
}

impl EventWatcherBuilder<EventWatcherPath, EventWatcherForDir<String>> {
    pub fn build(self) -> Result<EventWatcher, WatcherErr> {
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
        })
    }
}

pub struct EventWatcherNoForDir;

pub struct EventWatcherForDir<T>(ForDir<T>);

pub struct EventWatcherNoPath;

pub struct EventWatcherPath(PathBuf);

impl std::default::Default for EventWatcherBuilder<EventWatcherNoPath, EventWatcherNoForDir> {
    fn default() -> Self {
        Self {
            path: EventWatcherNoPath,
            r#await: None,
            for_dir: EventWatcherNoForDir,
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
