pub mod bucket;
pub mod cli;
pub mod grpc_v1;
pub mod grpc_v1_server;
pub mod handlers;
pub mod manager;
pub mod models;
pub mod state;
pub mod user;
pub mod ws;

use crate::{
    bucket::bucket_map::BucketMap,
    cli::Args,
    manager::{
        Manager, WatcherParams,
        utils::{Run, SplitTask},
    },
    state::{State, local_storage::LocalStorageBuild, pg_listen::builder::ListenBucketBuilder},
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
    _ = dotenv::dotenv();

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
        md_host,
        md_port,
        md_username,
        md_pass,
        md_database,
        ignore_rename_suffix,
    } = Args::parse();

    let tr = fmt().with_max_level(Level::from(log_level)).finish();
    tracing::subscriber::set_global_default(tr)?;

    let listener = TcpListener::bind(format!("{listen}:{port}")).await?;

    let mut http = http1::Builder::new();
    http.keep_alive(true);

    let state = Arc::new(RwLock::new(BucketMap::new(watcher_path)?));
    let listen_b = ListenBucketBuilder::default()
        .username(username)
        .database(database_name)
        .password(password)
        .channel(channel)
        .host(database_host)
        .port(database_port)
        .workdir(state.read().await.path().to_string_lossy().into_owned())
        .build()
        .await;

    let ls = LocalStorageBuild::default()
        .host(md_host)
        .port(md_port)
        .password(md_pass)
        .username(md_username)
        .database(md_database)
        .build()
        .await;

    let ls = Arc::new(ls);
    state.write().await.build(ls.as_ref()).await.unwrap();

    let (msgs, task) = Manager::new(
        state.clone(),
        match watcher {
            cli::TypeWatcher::Poll => {
                todo!()
            }
            cli::TypeWatcher::Event => WatcherParams::Event {
                path: PathBuf::from(state.read().await.path()),
                r#await: None,
                ignore_rename_suffix,
            },
        },
        grpc_auth_server,
        listen_b,
        ls,
    )
    .await
    .split();

    let state = Arc::new(State::new(state, msgs).await);
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
