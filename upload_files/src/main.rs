mod grpc_v1;
mod handlers;
mod models;
mod redirect;
mod stream_upload;

use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use utils::{service_with_state, Io, Peer};

use crate::grpc_v1::GrpcClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().unwrap();
    tracing_subscriber::fmt().init();

    let socket = SocketAddr::new("0.0.0.0".parse().unwrap(), 4000);
    let listen = TcpListener::bind(socket).await?;

    let sender = Arc::new(GrpcClient::new("".to_string()).await);

    tracing::info!("Listening: {:?}", &socket);
    loop {
        let (stream, _) = listen.accept().await?;
        let peer = Peer::new(stream.peer_addr().ok());
        let io = Io::new(stream);
        let sender = sender.clone();
        tokio::task::spawn(async move {
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(
                    io,
                    service_with_state(sender, async |mut req| {
                        req.extensions_mut().insert(peer);
                        handlers::entry(req).await
                    })
                )
                .await
            {
                tracing::error!("{e:?}");
            }
        });
    }
}
