use std::path::PathBuf;

use notify::PollWatcher;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub struct PollWatcherNotify {
    path: PathBuf,
    pool_watcher: PollWatcher,
    tx: UnboundedSender<Result<notify::Event, notify::Error>>,
    rx: Option<UnboundedReceiver<Result<notify::Event, notify::Error>>>,
}