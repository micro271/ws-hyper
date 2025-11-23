pub mod bucket;
pub mod cli;
pub mod grpc_v1;
pub mod handlers;
pub mod manager;
pub mod state;
pub mod user;
pub mod ws;

use crate::{
    bucket::bucket_map::BucketMap,
    cli::Args,
    manager::{Manager, WatcherParams, utils::{Run, SplitTask}},
    state::{State, pg_listen::builder::ListenBucketBuilder},
};
use clap::Parser;
use hyper::server::conn::http1;
use std::{env, path::PathBuf, sync::Arc};
use tokio::{net::TcpListener, sync::RwLock};
use tracing::Level;
use tracing_subscriber::fmt;
use utils::{Io, Peer, service_with_state};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("{:?}", env::current_dir());

    dotenv::dotenv().ok();
    let Args {
        watcher,
        watcher_path,
        listen,
        port,
        log_level,
        grpc_auth_server,
        database_name,
        username,
        password,
        channel,
        database_host,
        database_port,
    } = Args::parse();

    let tr = fmt().with_max_level(Level::from(log_level)).finish();
    tracing::subscriber::set_global_default(tr)?;

    let listener = TcpListener::bind(format!("{listen}:{port}")).await?;

    let mut http = http1::Builder::new();
    http.keep_alive(true);

    let state = BucketMap::new(watcher_path)?;

    let state = Arc::new(RwLock::new(state));
    let listen_b = ListenBucketBuilder::default()
        .username(username)
        .database(database_name)
        .password(password)
        .channel(channel)
        .host(database_host)
        .port(database_port)
        .build().await;

    let ((websocker_subscribers, grpc_client), task) = Manager::new(
        state.clone(),
        match watcher {
            cli::TypeWatcher::Poll => {
                todo!()
            }
            cli::TypeWatcher::Event => WatcherParams::Event {
                path: PathBuf::from(state.read().await.path()),
                r#await: None,
            },
        },
        grpc_auth_server,
        listen_b,
    )
    .await.split();

    let state = Arc::new(State::new(state, websocker_subscribers, grpc_client).await);
    task.run();

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
