mod grpc_v1;
mod handlers;
mod models;
mod redirect;
mod stream_upload;

use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use utils::{Io, Peer, service_with_state};

use crate::grpc_v1::GrpcClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().unwrap();
    tracing_subscriber::fmt().init();

    let ip_app = std::env::var("IP_APP").unwrap_or("0.0.0.0".to_string());
    let endpoint_grpc_client_check =
        std::env::var("GRPC_USER_CHECK").expect("Grpc endpoint for user check is not defined");

    let socket = SocketAddr::new(ip_app.parse().unwrap(), 4000);
    let listen = TcpListener::bind(socket).await?;

    let sender = Arc::new(GrpcClient::new(endpoint_grpc_client_check).await);

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
                    }),
                )
                .await
            {
                tracing::error!("{e:?}");
            }
        });
    }
}
