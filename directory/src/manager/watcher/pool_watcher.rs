use std::{path::PathBuf, time::Duration};

use notify::{Config, PollWatcher, Watcher};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::{manager::{
    utils::OneshotSender,
    watcher::{WatcherOwn, error::WatcherErr},
}, state};

pub struct PollWatcherNotify {
    _poll_watcher: PollWatcher,
    tx: UnboundedSender<Result<notify::Event, notify::Error>>,
    rx: Option<UnboundedReceiver<Result<notify::Event, notify::Error>>>,
    path: String,
}

impl PollWatcherNotify {
    pub fn new<T: AsRef<str>>(real_path: T, root: T, interval: u64) -> Result<Self, WatcherErr> {
        let mut path = PathBuf::from(real_path.as_ref());

        if path.is_relative() {
            path = path
                .canonicalize()
                .map_err(|x| WatcherErr::new(x.to_string()))?;
        };

        let (tx, rx) = unbounded_channel();
        let tx_cp = tx.clone();
        let mut poll = PollWatcher::new(
            move |ev| {
                if let Err(err) = tx_cp.send(ev) {
                    tracing::error!("[Watcher] {{ Inner Task Error }} {err}");
                }
            },
            Config::default().with_poll_interval(Duration::from_millis(interval)),
        )
        .map_err(|x| WatcherErr::new(x.to_string()))?;

        poll.watch(&path, notify::RecursiveMode::Recursive)
            .map_err(|x| WatcherErr::new(x.to_string()))?;

        Ok(Self {
            tx,
            rx: Some(rx),
            _poll_watcher: poll,
            path: real_path.as_ref().to_string(),
        })
    }
}

impl<T> WatcherOwn<T, UnboundedSender<Result<notify::Event, notify::Error>>> for PollWatcherNotify
where
    T: OneshotSender,
{
    fn run(self, tx: T) {
        todo!()
    }

    async fn task(mut self, tx: T) {
        let mut rx = self.rx.take().unwrap();

        while let Some(inc) = rx.recv().await {
            // TODO
            tracing::info!("{inc:?}");
        }

        tracing::error!("[Watcher] {{ Event Loop broken }} rx was close");
    }

    fn tx(&self) -> UnboundedSender<Result<notify::Event, notify::Error>> {
        self.tx.clone()
    }
}
