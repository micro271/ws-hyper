pub mod handlers;
pub mod directory;

use hyper::server::conn::http1;
use std::sync::Arc;
use tokio::net::TcpListener;
use utils::{Io, Peer, service_with_state};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("0.0.0.0:3500").await?;
    let state = Arc::new(1);

    let mut http = http1::Builder::new();
    http.keep_alive(true);

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
