pub mod api_v1;
pub mod error;
pub mod program;

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

type ResponseWithError = Result<Response<Full<Bytes>>, ResponseError>;
type ResponsesHttp = Result<Response<Full<Bytes>>, Infallible>;

pub type State = Arc<Repository>;

static HTTP: LazyLock<Mutex<Tera>> =
    LazyLock::new(|| Mutex::new(Tera::new("www/**/*").expect("Dir error")));

pub async fn entry(req: Request<Incoming>, peer: Option<SocketAddr>) -> ResponseWithError {
    let duration = std::time::Instant::now();
    let user_agent = http::header::USER_AGENT;
    let user_agent_value = req
        .headers()
        .get(&user_agent)
        .and_then(|x| x.to_str().map(ToString::to_string).ok())
        .unwrap_or_default();
    let user_agent = user_agent.to_string();
    let path = req.uri().path().to_string();
    let tmp = hello(req).await;
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

pub async fn hello(req: Request<Incoming>) -> ResponseWithError {
    let protected = ["/", "/api/v1"];
    match (req.uri().path(), req.method()) {
        (_, &Method::OPTIONS) => Ok(cors()),
        ("/login", &Method::POST) => Ok(login().await.unwrap()),
        ("/login", &Method::GET) => api_v1::login(req).await,
        (path, _) if protected.iter().any(|x| path.starts_with(x)) => {
            let Some(claims) = api_v1::verifi_token_from_cookie(req.headers()).await else {
                return Ok(Redirect::to("/login").into());
            };

            match path {
                "/" => Ok(great().await.unwrap()),
                path if path.starts_with("/api/v1") => api_v1::api(req, claims).await,
                _ => Ok(fallback().await.unwrap()),
            }
        }
        _ => Ok(fallback().await.unwrap()),
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

pub async fn from_incoming_to<T>(body: Incoming) -> Result<T, ResponseError>
where
    T: DeserializeOwned,
{
    match body.collect().await {
        Ok(e) => match serde_json::from_slice::<'_, T>(&e.to_bytes()) {
            Ok(e) => Ok(e),
            _ => Err(ResponseError::new(
                StatusCode::BAD_REQUEST,
                Some("Parsing data entry error"),
            )),
        },
        Err(e) => {
            tracing::error!("Error to deserialize the body - {e}");
            Err(ResponseError::new(
                StatusCode::BAD_REQUEST,
                Some("Data entry error"),
            ))
        }
    }
}
