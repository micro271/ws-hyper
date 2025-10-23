
use crate::manager::utils::ForDir;

use super::{PathBuf, WatcherErr, RenameControl, EventWatcher, unbounded_channel, Watcher, RecursiveMode};

#[derive(Debug, Default)]
pub struct EventWatcherBuilder {
    path: Option<PathBuf>,
    r#await: Option<u64>,
    for_dir: Option<ForDir<String>>,
}

impl EventWatcherBuilder {
    pub fn rename_control_await(mut self, r#await: u64) -> Self {
        self.r#await = Some(r#await);
        self
    }

    pub fn path(mut self, mut path: PathBuf) -> Result<Self, WatcherErr> {
        if path.is_relative() {
            path = path.canonicalize().map_err(|x| WatcherErr::new(x.to_string()))?;
        }

        self.path = Some(path);

        Ok(self)
    }

    pub fn for_dir(mut self, real_path: String, root: String) -> Self {
        self.for_dir = Some(ForDir::new(root, real_path));
        self
    }

    pub fn build(self) -> Result<EventWatcher, WatcherErr> {
        let Some(path) = self.path else {
            return Err(WatcherErr::new("Path not defined"));
        };
        let r#await = self.r#await.unwrap_or(2000);

        let Some(for_dir) = self.for_dir else {
            return Err(WatcherErr::new("Metadata to create directory type isn't present"));
        };

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