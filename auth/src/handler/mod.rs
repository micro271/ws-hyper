use http_body_util::Full;
use hyper::{
    Method, Request, Response, StatusCode,
    body::{Bytes, Incoming},
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

pub mod login;

pub async fn entry(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let url = req.uri().path();

    match (url, req.method()) {
        ("/login", &Method::POST) => login::login(req).await,
        _ => Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::new(Bytes::from_static(b"Not Found")))
            .unwrap_or_default()),
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Login {
    username: String,
    password: String,
}
