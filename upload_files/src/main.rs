mod grpc_v1;
mod handlers;
mod models;
mod redirect;
mod stream_upload;

use bytes::Bytes;
use http::{Method, Request, Response, header};
use http_body_util::Full;
use hyper::{body::Incoming, server::conn::http1, service::service_fn};
use std::{collections::HashMap, convert::Infallible, net::SocketAddr};
use tokio::net::TcpListener;
use utils::{
    Io, Peer,
    middleware::{Layer, MiddlwareStack, cors::CorsBuilder, log_layer::builder::LogLayerBuilder},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().unwrap();
    tracing_subscriber::fmt().init();

    let ip_app = std::env::var("IP_APP").unwrap_or("0.0.0.0".to_string());
    let endpoint_grpc_client_check =
        std::env::var("GRPC_USER_CHECK").expect("Grpc endpoint for user check is not defined");

    let socket = SocketAddr::new(ip_app.parse().unwrap(), 4000);
    let listen = TcpListener::bind(socket).await?;

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

    tracing::info!("Listening: {:?}", &socket);

    let stack = MiddlwareStack::default()
        .entry_fn(async |req: Request<Incoming>| {
            Result::<Response<Full<Bytes>>, Infallible>::Ok(Response::new(Full::new(
                Bytes::default(),
            )))
        })
        .layer(cors)
        .layer(trace);

    loop {
        let (stream, _) = listen.accept().await?;
        let io = Io::new(stream);
        let _stack = stack.clone();
        tokio::task::spawn(async move {
            if let Err(e) = http1::Builder::new()
                .serve_connection(io, service_fn(|req| _stack.call(req)))
                .await
            {
                tracing::error!("{e:?}");
            }
        });
    }
}
