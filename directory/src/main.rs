pub mod directory;
pub mod grpc_v1;
pub mod handlers;
pub mod manager;
pub mod ws;

use crate::{directory::tree_dir::TreeDir, manager::Schedule};
use hyper::server::conn::http1;
use std::sync::Arc;
use tokio::{net::TcpListener, sync::RwLock};
use utils::{Io, Peer, service_with_state};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let ip = std::env::var("IP").unwrap_or("0.0.0.0".to_string());
    let port = std::env::var("PORT").unwrap_or("3500".to_string());
    let root_path = std::env::var("ROOT_PATH").unwrap_or("./".to_string());
    let listener = TcpListener::bind(format!("{ip}:{port}")).await?;

    let mut http = http1::Builder::new();
    http.keep_alive(true);

    let state = Arc::new(RwLock::new(
        TreeDir::new_async(root_path.clone().into()).await.unwrap(),
    ));

    let sch = Arc::new(Schedule::new(state.clone(), root_path).await);

    loop {
        let (stream, _) = listener.accept().await?;
        let peer = Peer::new(stream.peer_addr().ok());
        let state = sch.clone();
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
