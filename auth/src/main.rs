mod handler;
mod models;
mod repository;
use crate::{handler::entry, repository::PgRepository};
use hyper::server::conn::http1;
use std::sync::Arc;
use utils::{Io, service_with_state};
type Repository = Arc<PgRepository>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    let db_host = std::env::var("DB_HOST").expect("Database url is not defined");
    let db_port = std::env::var("DB_PORT").expect("Port is not defined");
    let db_user = std::env::var("DB_USERNAME").expect("Database's user is not defined");
    let secret = std::env::var("DB_PASSWD").expect("Database's password is not defined");
    let db_name = std::env::var("DB_NAME").expect("Database's name is not defined");

    let uri = format!("postgres://{db_user}:{secret}@{db_host}:{db_port}/{db_name}");

    let repo = Arc::new(PgRepository::with_default_user(uri).await?);

    let app_port = std::env::var("PORT").unwrap_or("2525".to_string());
    let app_host = std::env::var("APP_HOST").unwrap_or("0.0.0.0".to_string());

    let listener = tokio::net::TcpListener::bind(format!("{app_host}:{app_port}")).await?;

    loop {
        let (stream, _) = listener.accept().await?;
        let peer = stream.peer_addr().ok();
        let repo = Arc::clone(&repo);
        let io = Io::new(stream);
        tokio::task::spawn(async move {
            if let Err(e) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_with_state(repo, |mut req| {
                        req.extensions_mut().insert(peer);
                        entry(req)
                    }),
                )
                .await
            {
                panic!("{e}");
            }
        });
    }
}
