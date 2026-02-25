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
    handlers::entry,
    manager::{
        Manager, WatcherParams,
        utils::{Run, SplitTask},
    },
    state::{State, local_storage::LocalStorageBuild},
};
use clap::Parser;
use http::{Method, header};
use hyper::{server::conn::http1, service::service_fn};
use std::{collections::HashMap, env, path::PathBuf, sync::Arc};
use tokio::{net::TcpListener, sync::RwLock};
use tonic::transport::Server;
use tracing::Level;
use tracing_subscriber::fmt;
use utils::{
    Io, Peer,
    middleware::{Layer, MiddlwareStack, cors::CorsBuilder, log_layer::builder::LogLayerBuilder},
};

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
        md_host,
        md_port,
        md_username,
        md_pass,
        md_database,
        ignore_rename_suffix,
        pki_dir: _,
        grpc_endpoint,
    } = Args::parse();

    let tr = fmt().with_max_level(Level::from(log_level)).finish();
    tracing::subscriber::set_global_default(tr)?;

    let listener = TcpListener::bind(format!("{listen}:{port}")).await?;

    let mut http = http1::Builder::new();
    http.keep_alive(true);

    let state = Arc::new(RwLock::new(BucketMap::new(watcher_path)?));

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

    let grpc_dir_manager =
        grpc_v1_server::BucketGrpcSrv::new(state.clone(), state.read().await.path());

    Server::builder()
        .add_service(grpc_v1_server::DirectoryServer::new(grpc_dir_manager))
        .serve(grpc_endpoint)
        .await?;

    tracing::info!(
        "[GRPC SERVER DIRECTORY MANAGER IS ALREADY RUNNING]: Endpoint {}",
        grpc_endpoint
    );

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
        ls,
    )
    .await
    .split();

    let state = Arc::new(State::new(state, msgs).await);
    task.run();

    let cors = CorsBuilder::default()
        .allow_origin("http://localhost:5173")
        .allow_method(Method::PUT)
        .allow_method(Method::GET)
        .allow_method(Method::OPTIONS)
        .allow_method(Method::PATCH)
        .allow_header(header::CONTENT_TYPE)
        .allow_header(header::COOKIE)
        .allow_header(header::AUTHORIZATION)
        .allow_credentials(true)
        .build();

    let trace = LogLayerBuilder::default()
        .on_request(async |x| {
            let hd = [
                (header::CONTENT_TYPE, x.headers().get(header::CONTENT_TYPE)),
                (header::COOKIE, x.headers().get(header::COOKIE)),
                (
                    header::AUTHORIZATION,
                    x.headers().get(header::AUTHORIZATION),
                ),
                (header::USER_AGENT, x.headers().get(header::USER_AGENT)),
                (header::ORIGIN, x.headers().get(header::ORIGIN)),
            ]
            .into_iter()
            .filter_map(|(name, value)| value.map(|v| (name, v)))
            .collect::<HashMap<_, _>>();

            tracing::info!(
                "{{ on_request }} path={} method={} peer={:?} headers {:?}",
                x.uri().path(),
                x.method(),
                x.extensions().get::<Peer>(),
                hd,
            )
        })
        .on_response(async |x, i| {
            tracing::info!(
                "{{ on_response }} status = {} duration = {}ms headers = {:?}",
                x.status(),
                i.elapsed().as_millis(),
                x.headers()
            )
        })
        .build();

    let stack_layer = Arc::new(
        MiddlwareStack::default()
            .entry_fn(entry)
            .state(state)
            .layer(cors)
            .layer(trace),
    );

    tracing::info!("Listen: {listen}:{port}");

    loop {
        let (stream, _) = listener.accept().await?;
        let peer = Peer::new(stream.peer_addr().ok());
        let io = Io::new(stream);
        let stack_layer = stack_layer.clone();

        tokio::task::spawn(async move {
            if let Err(e) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(|mut req| {
                        req.extensions_mut().insert(peer);
                        stack_layer.call(req)
                    }),
                )
                .with_upgrades()
                .await
            {
                tracing::error!("{e}");
            }
        });
    }
}
