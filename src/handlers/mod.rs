pub mod api;
pub mod error;

use std::{convert::Infallible, net::SocketAddr, sync::{Arc, LazyLock}};
use bytes::Bytes;
use error::ResponseError;
use http::{header, Request, Response, StatusCode};
use http_body_util::Full;
use hyper::body::Incoming;
use tokio::sync::Mutex;
use tera::{Context, Tera};

use crate::repository::Repository;

type ResponsesHttp = Result<Response<Full<Bytes>>, Infallible>;
type ResponseWithError = Result<Response<Full<Bytes>>, ResponseError>;

static HTTP: LazyLock<Mutex<Tera>> = LazyLock::new(|| 
    Mutex::new(Tera::new("www/**/*").expect("Dir error"))
);

pub async fn middlewares(req: Request<Incoming>, peer: Option<SocketAddr>, repository: Arc<Repository>) -> ResponseWithError {
    let duration = std::time::Instant::now();
    let user_agent = http::header::USER_AGENT;
    let user_agent_value = req.headers().get(&user_agent).and_then(|x| x.to_str().map(ToString::to_string).ok()).unwrap_or("".to_string());
    let user_agent = user_agent.to_string();
    
    let path = req.uri().path().to_string();

    let tmp = hello(req, repository).await;

    let duration = duration.elapsed().as_millis();

    match tmp {
        Ok(e) => {
            tracing::info!("Request [ Path={}, duration: {}ms, Peer: {:?}, {}: {} ]", path, duration, peer, user_agent, user_agent_value);
            Ok(e)
        },
        Err(e) => {
            tracing::error!("Request [ Path={}, error: {:?}, duration: {}ms, Peer: {:?}, {}: {}]", path, e, duration, peer, user_agent, user_agent_value);
            Err(e)
        }
    }

}

pub async fn hello(req: Request<Incoming>, repository: Arc<Repository>) -> ResponseWithError {
    match req.uri().path() {
        "/" => Ok(great().await.unwrap()),
        "/login" => Ok(login().await.unwrap()),
        e if e.starts_with("/api/v1/") => api::api(req, repository).await,
        _ => Ok(fallback().await.unwrap()),
    }
}


pub async fn fallback() -> ResponsesHttp {
    let tera = HTTP.lock().await.render("fallback.html", &Context::new()).unwrap();

    Ok(html_basic(tera, StatusCode::BAD_REQUEST).await.unwrap_or_default())
}


pub async fn great() -> ResponsesHttp {
    let tera = HTTP.lock().await.render("index.html", &Context::new()).unwrap();

    Ok(html_basic(tera, StatusCode::BAD_REQUEST).await.unwrap_or_default())
}

pub async fn login() -> ResponsesHttp {
    let tera = HTTP.lock().await.render("login.html", &Context::new()).unwrap();

    Ok(html_basic(tera, StatusCode::BAD_REQUEST).await.unwrap_or_default())
}


async fn html_basic(body: String, status: StatusCode) -> Result<Response<Full<Bytes>>, http::Error> {
    Response::builder()
        .header(header::CONTENT_TYPE, "text/html")
        .header(header::CONTENT_LENGTH, body.len())
        .status(status)
        .body(Full::new(Bytes::from(body)))
}