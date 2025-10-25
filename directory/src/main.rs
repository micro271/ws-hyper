pub mod cli;
pub mod directory;
pub mod grpc_v1;
pub mod handlers;
pub mod manager;
pub mod state;
pub mod user;
pub mod ws;

use crate::{
    cli::Args,
    directory::tree_dir::TreeDir,
    manager::{
        Schedule,
        watcher::{Watcher, event_watcher::EventWatcherBuilder, pool_watcher::PollWatcherNotify},
    },
    state::State,
};
use clap::Parser;
use hyper::server::conn::http1;
use std::{path::PathBuf, sync::Arc};
use tokio::{net::TcpListener, sync::RwLock};
use tracing::Level;
use tracing_subscriber::fmt;
use utils::{Io, Peer, service_with_state};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let Args {
        watcher,
        watcher_path,
        listen,
        port,
        log_level,
        prefix_root,
    } = Args::parse();

    let tr = fmt().with_max_level(Level::from(log_level)).finish();
    tracing::subscriber::set_global_default(tr)?;

    let listener = TcpListener::bind(format!("{listen}:{port}")).await?;

    let mut http = http1::Builder::new();
    http.keep_alive(true);

    let state = TreeDir::new_async(&watcher_path, prefix_root).await?;

    let state = Arc::new(RwLock::new(state));

    let websocker_subscribers = match watcher {
        cli::TypeWatcher::Poll => {
            let w = PollWatcherNotify::new(
                state.read().await.real_path().to_string(),
                state.read().await.root().to_string(),
                2000,
            )
            .unwrap();
            Schedule::run(state.clone(), Watcher::new(w))
        }
        cli::TypeWatcher::Event => {
            let w = EventWatcherBuilder::default()
                .rename_control_await(2000)
                .path(PathBuf::from(state.read().await.real_path()))
                .unwrap()
                .for_dir_root(state.read().await.root())
                .build()
                .unwrap();
            Schedule::run(state.clone(), Watcher::new(w))
        }
    };

    let state = Arc::new(State::new(state, websocker_subscribers));

    loop {
        let (stream, _) = listener.accept().await?;
        let peer = Peer::new(stream.peer_addr().ok());
        let state = state.clone();
        let io = Io::new(stream);
        let conn = http
            .serve_connection(
                io,
                service_with_state(state, move |mut req| {
                    req.extensions_mut().insert(peer);
                    handlers::middleware_jwt(req, handlers::entry)
                }),
            )
            .with_upgrades();
        tokio::task::spawn(async move {
            if let Err(e) = conn.await {
                tracing::error!("{e}");
            }
        });
    }
}
