pub mod auth_layer;
pub mod error;
use crate::{
    bucket::{Bucket, key::Key},
    handlers::error::ResponseError,
    state::State,
};

use http::{StatusCode, header};
use http_body_util::Full;
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
};
use serde_json::json;
use std::{convert::Infallible, sync::Arc};

type TypeState = Arc<State>;

pub type ResponseHttp = Result<Response<Full<Bytes>>, ResponseError>;

pub async fn entry(mut req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path().to_string();

    if let Some(path) = path
        .strip_prefix("/tree")
        .map(|path| path.strip_prefix("/").unwrap_or(path))
    {
        if req.method() != http::Method::GET {
            return Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Full::default())
                .unwrap_or_default());
        }

        let state = req.extensions().get::<TypeState>().unwrap().clone();

        let (bucket, key) = if path.is_empty() {
            todo!("I need to check if the user logged is admin");
            (None, None)
        } else {
            let (path, key) = path.split_once("/").unwrap_or((path, ""));
            (
                Some(Bucket::new_unchecked(path)),
                (!key.is_empty()).then_some(Key::new(key.strip_suffix("/").unwrap_or(key))),
            )
        };

        if hyper_tungstenite::is_upgrade_request(&req) {
            let (res, ws) = hyper_tungstenite::upgrade(&mut req, None).unwrap();
            state.add_client(bucket, key, ws).await;
            Ok(res)
        } else {
            let state = state.read().await;
            let body = match (bucket, key) {
                (Some(bucket), Some(key)) => json!(state.get_response(&bucket, &key)).to_string(),
                (Some(bucket), None) => {
                    json!(state.get_response(&bucket, &Key::root())).to_string()
                }
                (None, None) => json!(state.get_buckets()).to_string(),
                (None, _) => {
                    unreachable!()
                }
            };

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Full::new(Bytes::from(body)))
                .unwrap_or_default())
        }
    } else {
        Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::default())
            .unwrap_or_default())
    }
}

pub async fn not_found(req: Request<Incoming>) -> ResponseError {
    ResponseError::new(
        format!("Path {:?} not found", req.uri().path()),
        StatusCode::NOT_FOUND,
    )
}
