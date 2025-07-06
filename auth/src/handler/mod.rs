mod entry;
mod error;
pub mod login;
pub mod user;

use http_body_util::Full;
use hyper::{
    Method, Request, Response, StatusCode,
    body::{Bytes, Incoming},
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

use crate::handler::{error::ResponseErr, user::get};

type ResponseHandlers = Result<Response<Full<Bytes>>, ResponseErr>;

pub async fn entry(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let url = req.uri().path();

    let resp = match (url, req.method()) {
        ("/login", &Method::POST) => login::login(req).await,
        ("/api/v1/user", _) => {
            let path = req.uri().path().strip_prefix("/api/v1/user").unwrap();
            let uuid = path.parse().ok();
            match *req.method() {
                Method::POST if path.is_empty() => user::new(req).await,
                Method::GET => get(req, uuid).await,
                Method::DELETE => user::delete(req, uuid.unwrap()).await,
                Method::PATCH => user::update(req, uuid.unwrap()).await,
                _ => Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Full::new(Bytes::from_static(b"Not Found")))
                    .unwrap_or_default()),
            }
        }
        _ => Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::new(Bytes::from_static(b"Not Found")))
            .unwrap_or_default()),
    };

    match resp {
        Ok(e) => Ok(e),
        Err(err) => Ok(err.into()),
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Login {
    username: String,
    password: String,
}
