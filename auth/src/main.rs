mod grpc_v1;
mod handler;
mod models;
mod state;
use crate::{
    grpc_v1::user_control::InfoUserProgram, handler::entry, models::user::default_account_admin,
    state::PgRepository,
};
use grpc_v1::user_control::InfoServer;
use hyper::{Method, header, server::conn::http1};
use std::sync::Arc;
use tonic::transport::Server;
use tracing_subscriber::{EnvFilter, fmt};
use utils::{
    GenEcdsa, Io, JwtHandle, Peer,
    middleware::{cors::CorsBuilder, log_layer::builder::LogLayerBuilder},
    service_with_state,
};

type Repository = Arc<PgRepository>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let subscriber = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let db_host = std::env::var("DB_HOST").expect("Database url is not defined");
    let db_port = std::env::var("DB_PORT").expect("Port is not defined");
    let db_user = std::env::var("DB_USERNAME").expect("Database's user is not defined");
    let secret = std::env::var("DB_PASSWD").expect("Database's password is not defined");
    let db_name = std::env::var("DB_NAME").expect("Database's name is not defined");

    let uri = format!("postgres://{db_user}:{secret}@{db_host}:{db_port}/{db_name}");

    let repo = Arc::new(PgRepository::with_default_user(uri, default_account_admin()?).await?);

    let app_port = std::env::var("PORT").unwrap_or("3000".to_string());
    let app_host = std::env::var("APP_HOST").unwrap_or("0.0.0.0".to_string());

    let listener = tokio::net::TcpListener::bind(format!("{app_host}:{app_port}")).await?;

    JwtHandle::gen_ecdsa(None)?;
    let gprc_ceck_user = "[::]:50051".parse()?;
    let user_check = InfoUserProgram::new(repo.clone());

    tracing::info!("Listening {}:{}", app_host, app_port);

    tokio::spawn(async move {
        Server::builder()
            .add_service(InfoServer::new(user_check))
            .serve(gprc_ceck_user)
            .await
    });

    let cors = Arc::new(
        CorsBuilder::default()
            .allow_origin("http://localhost:5173")
            .next(entry)
            .allow_method(Method::PUT)
            .allow_method(Method::GET)
            .allow_method(Method::OPTIONS)
            .allow_method(Method::PATCH)
            .allow_header(header::CONTENT_TYPE)
            .allow_header(header::COOKIE)
            .allow_header(header::AUTHORIZATION)
            .allow_credentials(true)
            .build(),
    );

    let layer = Arc::new(
        LogLayerBuilder::default()
            .next(async move |x| cors.middleware(x).await)
            .on_request(async |x| {
                tracing::info!(
                    "{{ on_request }} path={} method={} peer={:?} headers {:?}",
                    x.uri().path(),
                    x.method(),
                    x.extensions().get::<Peer>(),
                    x.headers().iter().filter(|(x, _)| [
                        header::CONTENT_TYPE,
                        header::COOKIE,
                        header::AUTHORIZATION,
                        header::USER_AGENT,
                        header::ORIGIN
                    ]
                    .contains(x))
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
            .build(),
    );

    loop {
        let (stream, _) = listener.accept().await?;
        let peer = Peer::new(stream.peer_addr().ok());
        let repo = repo.clone();
        let io = Io::new(stream);
        let layer_ = layer.clone();
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_with_state(repo, |mut req| {
                        req.extensions_mut().insert(peer);
                        layer_.middleware(req)
                    }),
                )
                .await
            {
                tracing::error!("{err}");
            }
        });
    }
}
