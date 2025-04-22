pub mod api;
pub mod error;
pub mod file;
pub mod user;

use bytes::Bytes;
use error::{Redirect, ResponseError};
use http::{Request, Response, StatusCode, header};
use http_body_util::Full;
use hyper::body::Incoming;
use std::{
    convert::Infallible,
    net::SocketAddr,
    sync::{Arc, LazyLock},
};
use tera::{Context, Tera};
use tokio::sync::Mutex;

use crate::repository::Repository;

type ResponseWithError = Result<Response<Full<Bytes>>, ResponseError>;
type ResponsesHttp = Result<Response<Full<Bytes>>, Infallible>;

static HTTP: LazyLock<Mutex<Tera>> =
    LazyLock::new(|| Mutex::new(Tera::new("www/**/*").expect("Dir error")));

pub async fn entry(
    req: Request<Incoming>,
    peer: Option<SocketAddr>,
    repository: Arc<Repository>,
) -> ResponseWithError {
    let duration = std::time::Instant::now();
    let user_agent = http::header::USER_AGENT;
    let user_agent_value = req
        .headers()
        .get(&user_agent)
        .and_then(|x| x.to_str().map(ToString::to_string).ok())
        .unwrap_or_default();
    let user_agent = user_agent.to_string();
    let path = req.uri().path().to_string();
    let tmp = hello(req, repository).await;
    let duration = duration.elapsed().as_millis();
    match tmp {
        Ok(e) => {
            tracing::info!(
                "Request [ Path={}, duration: {}ms, Peer: {:?}, {}: {} ]",
                path,
                duration,
                peer,
                user_agent,
                user_agent_value
            );
            Ok(e)
        }
        Err(e) => {
            tracing::error!(
                "Request [ Path={}, error: {:?}, duration: {}ms, Peer: {:?}, {}: {}]",
                path,
                e,
                duration,
                peer,
                user_agent,
                user_agent_value
            );
            Err(e)
        }
    }
}

pub async fn hello(req: Request<Incoming>, repository: Arc<Repository>) -> ResponseWithError {
    let protected = ["/", "/api/v1"];
    match req.uri().path() {
        e if protected.iter().any(|x| e.starts_with(x)) => {
            let Some(_claims) = api::verifi_token_from_cookie(req.headers()).await else {
                return Ok(Redirect::to("/login").into());
            };

            match e {
                "/" => Ok(great().await.unwrap()),
                "/api/v1" => api::api(req, repository).await,
                _ => Ok(fallback().await.unwrap()),
            }
        }
        _ => Ok(login().await.unwrap()),
    }
}

async fn fallback() -> ResponsesHttp {
    let tera = HTTP
        .lock()
        .await
        .render("fallback.html", &Context::new())
        .unwrap();

    Ok(html_basic(tera, StatusCode::BAD_REQUEST).unwrap_or_default())
}

async fn great() -> ResponsesHttp {
    let tera = HTTP
        .lock()
        .await
        .render("index.html", &Context::new())
        .unwrap();

    Ok(html_basic(tera, StatusCode::BAD_REQUEST).unwrap_or_default())
}

async fn login() -> ResponsesHttp {
    let tera = HTTP
        .lock()
        .await
        .render("login.html", &Context::new())
        .unwrap();

    Ok(html_basic(tera, StatusCode::BAD_REQUEST).unwrap_or_default())
}

fn html_basic(body: String, status: StatusCode) -> Result<Response<Full<Bytes>>, http::Error> {
    Response::builder()
        .header(header::CONTENT_TYPE, "text/html")
        .header(header::CONTENT_LENGTH, body.len())
        .status(status)
        .body(Full::new(Bytes::from(body)))
}
