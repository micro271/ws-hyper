mod handlers;
mod models;
mod redirect;
mod repository;
mod stream_upload;

use models::logs::Logs;
use repository::Repository;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use utils::{Io, Peer, service_with_state};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().unwrap();
    tracing_subscriber::fmt().init();

    let socket = SocketAddr::new("0.0.0.0".parse().unwrap(), 4000);
    let listen = TcpListener::bind(socket).await?;

    let db_host = std::env::var("DB_HOST").expect("Database url is not defined");
    let db_port = std::env::var("DB_PORT").expect("Port is not defined");
    let db_user = std::env::var("DB_USERNAME").expect("Database's user is not defined");
    let db_passwd = std::env::var("DB_PASSWD").expect("Database's password is not defined");
    let db_name = std::env::var("DB_NAME").expect("Database's name is not defined");

    let repository = Arc::new(
        Repository::new(
            format!("mongodb://{db_host}:{db_port}"),
            db_user,
            db_passwd,
            db_name,
        )
        .await?,
    );

    repository.create_index::<Logs>().await?;

    tracing::info!("Listening: {:?}", &socket);
    loop {
        let (stream, _) = listen.accept().await?;
        let peer = stream.peer_addr().ok();
        let repo = Arc::clone(&repository);
        let io = Io::new(stream);
        tokio::task::spawn(async move {
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(
                    io,
                    service_with_state(repo, |mut req| {
                        let ext = req.extensions_mut();
                        ext.insert(Peer::new(peer));
                        handlers::entry(req)
                    }),
                )
                .await
            {
                tracing::error!("{e:?}");
            }
        });
    }
}
