pub mod auth_layer;
pub mod error;
use crate::{
    bucket::{Bucket, Cowed, key::Key},
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

pub async fn entry(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();

    if let Some(path) = path.strip_prefix("/tree") {
        if req.method() != http::Method::GET {
            return Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Full::default())
                .unwrap_or_default());
        }

        let state = req.extensions().get::<TypeState>().unwrap().clone();

        let body = if path.is_empty() {
            tracing::error!("I need to check if the user logged have the role Admin");
            Bytes::from(state.tree_as_json().await)
        } else {
            let Some((bucket, key)) = path[1..].split_once('/') else {
                todo!("");
            };
            let bucket = Bucket::new_unchecked(bucket).owned();
            let key = Key::new(key).owned();
            let tmp = state.read().await;
            let tmp = tmp.get_until(bucket, key).collect::<Vec<_>>();

            Bytes::from(json!(tmp).to_string())
        };

        if hyper_tungstenite::is_upgrade_request(&req) {
            let (res, ws) = hyper_tungstenite::upgrade(req, None).unwrap();
            state.add_client(todo!(), todo!(), ws).await;
            Ok(res)
        } else {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Full::new(body))
                .unwrap_or_default())
        }
    } else {
        Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::default())
            .unwrap_or_default())
    }
}

async fn get_path(
    req: Request<Incoming>,
    bucket: Option<Bucket<'_>>,
    key: Option<Key<'_>>,
) -> ResponseHttp {
    let state = req.extensions().get::<TypeState>().unwrap();
    let reader = state.read().await;
    let body = bucket.unwrap();
    let body = reader.get_bucket(body.borrow()).unwrap();

    // TODO: we need to obtain all keys contained whithin a single key

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "application/json")
        .status(StatusCode::OK)
        .body(Full::new(Bytes::from_owner(json!(body).to_string())))
        .unwrap_or_default())
}

pub async fn not_found(req: Request<Incoming>) -> ResponseError {
    ResponseError::new(
        format!("Path {:?} not found", req.uri().path()),
        StatusCode::NOT_FOUND,
    )
}
