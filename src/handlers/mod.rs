pub mod api_v1;
pub mod error;
pub mod program;
pub mod utils;

use super::{redirect::Redirect, repository::Repository};
use bytes::Bytes;
use error::ResponseError;
use http::{Method, Request, Response, StatusCode, header};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use serde::de::DeserializeOwned;
use std::{
    convert::Infallible,
    net::SocketAddr,
    sync::{Arc, LazyLock},
};
use tera::{Context, Tera};
use tokio::sync::Mutex;

type ResultResponse = Result<Response<Full<Bytes>>, ResponseError>;
type ResponsesHttp = Response<Full<Bytes>>;

pub type State = Arc<Repository>;

static HTTP: LazyLock<Mutex<Tera>> =
    LazyLock::new(|| Mutex::new(Tera::new("www/**/*").expect("Dir error")));

pub async fn entry(
    req: Request<Incoming>,
    peer: Option<SocketAddr>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    tracing::debug!(
        "Request:  {{ Method: {}, Uri: {}, Version: {:#?}, Headers: {:#?} }}",
        req.method(),
        req.uri(),
        req.version(),
        req.headers()
    );

    let duration = std::time::Instant::now();
    let path = req.uri().path().to_string();
    let response = hello(req).await;
    let peer = peer.map(|x| x.to_string()).unwrap_or("Unknown".to_string());
    let duration = duration.elapsed().as_millis();
    match response {
        Ok(r) => {
            tracing::info!(
                "Response {{ Status: {}, Path={}, duration: {}ms, Peer: {:?} }}",
                r.status(),
                path,
                duration,
                peer,
            );
            Ok(r)
        }
        Err(e) => {
            tracing::error!(
                "Response {{ Status: {}, Path={}, duration: {}ms, Peer: {}, error: {:?} }}",
                e.status(),
                path,
                e,
                duration,
                peer,
            );
            Ok(e.into())
        }
    }
}

pub async fn hello(mut req: Request<Incoming>) -> ResultResponse {
    let protected = ["/", "/api/v1"];
    match (req.uri().path(), req.method()) {
        (_, &Method::OPTIONS) => Ok(cors()),
        ("/login", &Method::GET) => Ok(login().await),
        ("/login", &Method::POST) => api_v1::login(req).await,
        (path, _) if protected.iter().any(|x| path.starts_with(x)) => {
            let Some(claims) = api_v1::verifi_token_from_cookie(req.headers()).await else {
                return Ok(Redirect::to("/login").into());
            };
            req.extensions_mut().insert(claims);

            match req.uri().path() {
                "/" => Ok(great().await),
                path if path.starts_with("/api/v1") => api_v1::api(req).await,
                _ => Ok(fallback().await),
            }
        }
        _ => Ok(fallback().await),
    }
}

async fn fallback() -> ResponsesHttp {
    let tera = HTTP
        .lock()
        .await
        .render("fallback.html", &Context::new())
        .unwrap();

    html_basic(tera, StatusCode::BAD_REQUEST).unwrap_or_default()
}

async fn great() -> ResponsesHttp {
    let tera = HTTP
        .lock()
        .await
        .render("index.html", &Context::new())
        .unwrap();

    html_basic(tera, StatusCode::BAD_REQUEST).unwrap_or_default()
}

async fn login() -> ResponsesHttp {
    let tera = HTTP
        .lock()
        .await
        .render("login.html", &Context::new())
        .unwrap();

    html_basic(tera, StatusCode::BAD_REQUEST).unwrap_or_default()
}

fn html_basic(body: String, status: StatusCode) -> Result<Response<Full<Bytes>>, http::Error> {
    Response::builder()
        .header(header::CONTENT_TYPE, "text/html")
        .header(header::CONTENT_LENGTH, body.len())
        .status(status)
        .body(Full::new(Bytes::from(body)))
}

pub fn cors() -> Response<Full<Bytes>> {
    Response::builder()
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "http://localhost")
        .header(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            "GET, POST, PATCH, UPDATE, DELETE",
        )
        .header(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            "Content-Type, Authorization",
        )
        .header(header::ACCESS_CONTROL_ALLOW_CREDENTIALS, "true")
        .header(header::ACCESS_CONTROL_MAX_AGE, 3600)
        .status(StatusCode::NO_CONTENT)
        .body(Full::new(Bytes::new()))
        .unwrap_or_default()
}
