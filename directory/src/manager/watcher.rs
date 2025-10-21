use std::{
    path::PathBuf, sync::{mpsc::{channel, Receiver, Sender}, Arc}, time::Duration
};

use notify::{
    event::{CreateKind, ModifyKind, RenameMode}, Config, INotifyWatcher, PollWatcher, RecursiveMode, Watcher as _
};
use tokio::sync::{mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender}, RwLock};

use crate::{
    directory::{file::File, tree_dir::TreeDir, Directory, WithPrefixRoot},
    manager::{Change, Rename, RenameControl, RenameFrom},
};

pub trait WatcherOwn<T: Send>: Send + Sync {
    fn run(self, tx: UnboundedSender<Change>);
    fn task(self, tx: UnboundedSender<Change>) -> impl std::future::Future<Output = ()>;
    fn get_send(&self) -> UnboundedSender<T>;
}

pub struct Watcher<W, R> {
    inner: Option<W>,
    tx: UnboundedSender<R>,
}

impl<W, R> Watcher<W, R> 
    where 
        W: WatcherOwn<R>,
        R: Send + Send,
{
    pub fn new(watcher: W) -> Self {
        let tx = watcher.get_send();

        Self {
            inner: Some(watcher),
            tx,
        }
    }

    pub fn get_sender(&self) -> UnboundedSender<R> {
        self.tx.clone()
    }

    pub fn into_inner(&mut self) -> Result<W, WatcherErr> {
        self.inner.take().ok_or(WatcherErr("Watcher not defined".to_string()))
    }
}

pub struct WatchFabric;

impl WatchFabric {
    pub fn poll_watcher(interval: u64, path: PathBuf) -> Result<PollWatcherNotify, WatcherErr> {
        if !path.exists() {
            return Err(WatcherErr(format!("Path {path:?} not found")));
        }

        let (tx, rx) = channel();
        let conf = Config::default().with_poll_interval(Duration::from_micros(interval));
        let tx_cp = tx.clone();
        let mut pw = PollWatcher::new(move |x| _ = tx_cp.send(x), conf).map_err(|x| WatcherErr(x.to_string()))?;
        pw.watch(&path, RecursiveMode::Recursive).map_err(|x| WatcherErr(x.to_string()))?;

        Ok(PollWatcherNotify {
            path,
            pool_watcher: pw,
            tx,
            rx: Some(rx),
        })
    }

    pub fn event_watcher_builder() -> EventWatcherBuilder {
        EventWatcherBuilder::default()
    }
}



pub struct PollWatcherNotify {
    path: PathBuf,
    pool_watcher: PollWatcher,
    tx: Sender<Result<notify::Event, notify::Error>>,
    rx: Option<Receiver<Result<notify::Event, notify::Error>>>,
}

#[derive(Debug, Default)]
pub struct EventWatcherBuilder {
    path: Option<PathBuf>,
    r#await: Option<u64>,
    state: Option<Arc<RwLock<TreeDir>>>,
}

impl EventWatcherBuilder {
    pub fn rename_control_await(
        mut self,
        r#await: u64,
    ) -> Self {
        self.r#await = Some(r#await);
        self
    }

    pub fn path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    pub fn state(mut self, state: Arc<RwLock<TreeDir>>) -> Self {
        self.state = Some(state);
        self
    }

    pub fn build(self) -> Result<EventWatcher, WatcherErr> {
        let Some(path) = self.path else {
            return Err(WatcherErr("Path not defined".to_string()));
        }; 
        let r#await = self.r#await.unwrap_or(2000); 
        
        let Some(state) = self.state else {
            return Err(WatcherErr("State not defined".to_string()));
        };

        if !path.exists() {
            return Err(WatcherErr(format!("Path {path:?} not exists")));
        }
        let (tx, rx) = unbounded_channel();
        let tx_cp = tx.clone();
        let mut notify_watcher = notify::recommended_watcher(move |event| _ = tx_cp.send(event))
            .map_err(|x| WatcherErr(x.to_string()))?;

        notify_watcher.watch(&path, RecursiveMode::Recursive).map_err(|x| WatcherErr(x.to_string()))?;
        
        let rename_control = RenameControl::new(tx.clone(), self.r#await.unwrap_or(r#await));

        Ok(EventWatcher {
            _notify_watcher: notify_watcher,
            rename_control,
            tx,
            rx,
            state,
        })
    }
}

pub struct EventWatcher {
    rename_control: RenameControl,
    _notify_watcher: INotifyWatcher,
    tx: UnboundedSender<Result<notify::Event, notify::Error>>,
    rx: UnboundedReceiver<Result<notify::Event, notify::Error>>,
    state: Arc<RwLock<TreeDir>>,
}

impl WatcherOwn<Result<notify::Event, notify::Error>> for EventWatcher {

    fn run(self, tx: UnboundedSender<Change>)
    where 
        Self: 'static,
    {
        tokio::task::spawn(self.task(tx));
    }

    async fn task(mut self, tx: UnboundedSender<Change>) {
        tracing::debug!("Watcher notify manage init");
        let tx_rename = self.rename_control.sender();

        while let Some(Ok(event)) = self.rx.recv().await {
            match event.kind {
                notify::EventKind::Create(CreateKind::Folder) => {
                    tracing::trace!("{event:?}");
                    let mut path = event.paths;
                    let path = path.pop().unwrap();
                    let reader = self.state.read().await;
                    let dir = Directory::from(WithPrefixRoot::new(
                        path.parent().unwrap(),
                        reader.real_path(),
                        reader.root(),
                    ));

                    if let Err(err) = tx.send(Change::New {
                        dir,
                        file: File::from(&path),
                    }) {
                        tracing::error!("New directory nofity error: {err}");
                    }
                }
                notify::EventKind::Create(action) => {
                    tracing::trace!("Event: {event:?}");
                    tracing::trace!("File Type: {action:?}");
                    let mut path = event.paths;
                    let path = path.pop().unwrap();
                    let reader = self.state.read().await;
                    if let Err(err) = tx.send(Change::New {
                        dir: Directory::from(WithPrefixRoot::new(
                            path.parent().unwrap(),
                            reader.real_path(),
                            reader.root(),
                        )),
                        file: File::from(&path),
                    }) {
                        tracing::error!("New file nofity error: {err}");
                    }
                }
                notify::EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                    let mut path = event.paths;
                    let path = path.pop().unwrap();

                    tracing::debug!(
                        "[Watcher] {{ ModifyKind::Name(RenameMode::From) }} {path:?} (Maybe Delete)"
                    );
                    if let Err(err) = tx_rename.send(Rename::From(RenameFrom(path))) {
                        tracing::error!("{err}");
                    }
                }
                notify::EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                    tracing::trace!("Modify both {:?}", event.paths);

                    let mut paths = event.paths;
                    let to = paths.pop().unwrap();
                    let from = paths.pop().unwrap();

                    if let Err(err) = tx_rename.send(Rename::Decline(from.clone())) {
                        tracing::error!("{err}");
                    }
                    let reader = self.state.read().await;
                    let path = to.parent().unwrap();
                    let dir = Directory::from(WithPrefixRoot::new(
                        path,
                        reader.real_path(),
                        reader.root(),
                    ));

                    let from_file_name = from.file_name().and_then(|x| x.to_str()).unwrap();
                    if let Err(err) = tx.send(Change::Name {
                        dir,
                        from: from_file_name.to_string(),
                        to: File::from(&to),
                    }) {
                        tracing::error!("tx_watcher error: {err}");
                    }
                }
                notify::EventKind::Remove(_) => {
                    let mut path = event.paths;
                    let path = path.pop().unwrap();
                    let file_name = path
                        .file_name()
                        .and_then(|x| x.to_str().map(ToString::to_string))
                        .unwrap();
                    let reader = self.state.read().await;
                    let parent = path.parent().unwrap();
                    let parent = Directory::from(WithPrefixRoot::new(
                        parent,
                        reader.real_path(),
                        reader.root(),
                    ));
                    tracing::trace!("[REMOVE] Directory: {parent:?}, file name: {file_name}");
                    if let Err(e) = tx.send(Change::Delete { parent, file_name }) {
                        tracing::error!("{e}");
                    }
                }
                _ => {}
            }
        }
    }

    fn get_send(&self) -> UnboundedSender<Result<notify::Event, notify::Error>> {
        self.tx.clone()
    }
}

#[derive(Debug)]
pub struct WatcherErr(String);

impl std::fmt::Display for WatcherErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for WatcherErr {}
