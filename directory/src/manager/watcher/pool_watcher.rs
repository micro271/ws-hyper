use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use notify::{Config, PollWatcher, Watcher};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::manager::{
    utils::{SplitTask, Task},
    watcher::error::WatcherErr,
};

pub struct PollWatcherNotifyCh<Tx>(Tx);

impl<Tx> PollWatcherNotifyCh<Tx>
where
    Tx: Clone,
{
    fn new(tx: Tx) -> Self {
        Self(tx)
    }
}

pub struct PollWatcherNotify {
    _poll_watcher: PollWatcher,
    tx: UnboundedSender<Result<notify::Event, notify::Error>>,
    rx: Option<UnboundedReceiver<Result<notify::Event, notify::Error>>>,
    path: String,
}

impl PollWatcherNotify {
    pub fn new<T: AsRef<Path>>(real_path: T, interval: u64) -> Result<Self, WatcherErr> {
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
            path: real_path.as_ref().to_string_lossy().into_owned(),
        })
    }
}

impl Task for PollWatcherNotify {

    fn task(mut self) -> impl Future<Output = ()> + Send + 'static
    where
        Self: Sized,
    {
        async move {
            let mut rx = self.rx.take().unwrap();
            loop {
                rx.recv().await;
            }
        }
    }
}

impl SplitTask for PollWatcherNotify {
    type Output = PollWatcherNotifyCh<UnboundedSender<Result<notify::Event, notify::Error>>>;
    fn split(self) -> (<Self as SplitTask>::Output, impl crate::manager::utils::Run) {
        (PollWatcherNotifyCh::new(self.tx.clone()), self)
    }
}
