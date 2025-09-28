mod handlers;
mod models;
mod redirect;
mod stream_upload;

use hyper::service::service_fn;
use std::{net::SocketAddr};
use tokio::net::TcpListener;
use utils::{Io, Peer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().unwrap();
    tracing_subscriber::fmt().init();

    let socket = SocketAddr::new("0.0.0.0".parse().unwrap(), 4000);
    let listen = TcpListener::bind(socket).await?;

    tracing::info!("Listening: {:?}", &socket);
    loop {
        let (stream, _) = listen.accept().await?;
        let peer = Peer::new(stream.peer_addr().ok());
        let io = Io::new(stream);
        tokio::task::spawn(async move {
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(async |mut req| {
                        req.extensions_mut().insert(peer);
                        handlers::entry(req).await
                    }),
                )
                .await
            {
                tracing::error!("{e:?}");
            }
        });
    }
}
