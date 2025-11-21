pub mod builder;
pub mod error;
pub mod event_watcher;
pub mod pool_watcher;

type NotifyChType = Result<notify::Event, notify::Error>;
