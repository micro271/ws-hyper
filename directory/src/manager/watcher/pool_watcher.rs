use std::{path::PathBuf, time::Duration};

use notify::{Config, PollWatcher, Watcher};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::manager::{watcher::{error::WatcherErr, WatcherOwn}, Change};

pub struct PollWatcherNotify {
    path: String,
    _poll_watcher: PollWatcher,
    tx: UnboundedSender<Result<notify::Event, notify::Error>>,
    rx: Option<UnboundedReceiver<Result<notify::Event, notify::Error>>>,
}

impl PollWatcherNotify {
    pub fn new(mut path: PathBuf, interval: u64) -> Result<Self, WatcherErr> {

        if path.is_relative() {
            path = path.canonicalize().map_err( |x| WatcherErr::new(x.to_string()))?;
        };

        let (tx, rx) = unbounded_channel();
        let tx_cp = tx.clone();
        let mut poll = PollWatcher::new(move |ev| {
            if let Err(err) = tx_cp.send(ev) {
                tracing::error!("[Watcher] {{ Inner Task Error }} {err}");
            }
        }, Config::default().with_poll_interval(Duration::from_millis(interval))).map_err(|x| WatcherErr::new(x.to_string()))?;

        poll.watch(&path, notify::RecursiveMode::Recursive).map_err(|x| WatcherErr::new(x.to_string()))?;

        let path = path.to_str().map(ToString::to_string).ok_or(WatcherErr::new(format!("Error parse from {path:?} to String")))?;

        Ok(Self {
            path,
            tx,
            rx: Some(rx),
            _poll_watcher: poll,
        })
    }
}

impl WatcherOwn<Change, Result<notify::Event, notify::Error>> for PollWatcherNotify {
    fn run(self, tx: UnboundedSender<Change>) {
        todo!()
    }

    async fn task(mut self, tx: UnboundedSender<Change>) {
        let mut rx = self.rx.take().unwrap();

        while let Some(inc) = rx.recv().await {
            tracing::info!("{inc:?}");
        }

        tracing::error!("[Watcher] {{ Event Loop broken }} rx was close");
    }

    fn get_send(&self) -> UnboundedSender<Result<notify::Event, notify::Error>> {
        self.tx.clone()
    }
}