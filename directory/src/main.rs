pub mod directory;
pub mod grpc_v1;
pub mod handlers;
pub mod manager;
pub mod state;
pub mod ws;

use crate::{directory::tree_dir::TreeDir, manager::{watcher::{WatchFabric, Watcher}, Schedule}, state::State};
use hyper::server::conn::http1;
use std::{path::PathBuf, sync::Arc};
use tokio::{net::TcpListener, sync::RwLock};
use tracing_subscriber::{EnvFilter, fmt};
use utils::{Io, Peer, service_with_state};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let tr = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(tr)?;

    let ip = std::env::var("IP").unwrap_or("0.0.0.0".to_string());
    let port = std::env::var("PORT").unwrap_or("3500".to_string());
    let root_path = std::env::var("ROOT_PATH").unwrap_or("prueba".to_string());
    let listener = TcpListener::bind(format!("{ip}:{port}")).await?;

    let mut http = http1::Builder::new();
    http.keep_alive(true);

    let state = Arc::new(RwLock::new(
        TreeDir::new_async(&root_path, String::new()).await?,
    ));

    let watcher = WatchFabric::event_watcher_builder()
        .path(PathBuf::from(&root_path))
        .rename_control_await(2000)
        .state(state.clone())
        .build()?;
    let watcher = Watcher::new(watcher);
    _ = Schedule::new(state.clone(), watcher);

    let state = Arc::new(State::new(state));

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
                    handlers::entry(req)
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
