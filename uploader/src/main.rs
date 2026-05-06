mod handler;
mod utils;

use ::utils::{
    Io, Peer,
    middleware::{
        Layer, MiddlewareBuilder, cors::CorsBuilder, log_layer::builder::LogLayerBuilder,
    },
};
use hyper::{Method, http::header, server::conn::http1, service::service_fn};
use std::{collections::HashMap, env, sync::Arc};
use tokio::net::TcpListener;
use tracing::Level;
use tracing_subscriber::fmt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("{:?}", env::current_dir());
    _ = dotenv::dotenv();

    let tr = fmt().with_max_level(Level::TRACE).finish();
    tracing::subscriber::set_global_default(tr)?;

    let listener = TcpListener::bind(format!("0.0.0.0:3501")).await?;

    let mut http = http1::Builder::new();
    http.keep_alive(true);

    let cors = CorsBuilder::default()
        .allow_origin("http://localhost:8080")
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
        MiddlewareBuilder::entry(handler::Entry)
            .layer(cors)
            .layer(trace),
    );

    tracing::info!("Listen: 0.0.0.0:3501");

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
