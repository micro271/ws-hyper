pub mod auth_layer;
pub mod error;
use crate::{
    bucket::{Bucket, bucket_map::AbsoluteKey, fhs_response::FhsResponse},
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

        let pair = if path.is_empty() {
            todo!("I need to check if the user logged is admin");
            None
        } else {
            let (path, key) = path.split_once("/").unwrap_or((path, "."));
            Some((Bucket::new_unchecked(path), AbsoluteKey(key.into())))
        };

        if hyper_tungstenite::is_upgrade_request(&req) {
            let (res, ws) = hyper_tungstenite::upgrade(&mut req, None).unwrap();
            // state.add_client(bucket, todo!(), ws).await;
            Ok(res)
        } else {
            let state = state.read().await;
            let body = match pair.as_ref() {
                Some((bucket, key)) => state.get_entry(bucket, key).unwrap().into(),
                None => {
                    let bks = state
                        .get_buckets()
                        .into_iter()
                        .map(|x| x.name())
                        .collect::<Vec<_>>();

                    FhsResponse::new(bks, None)
                }
            };

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Full::new(Bytes::from(json!(body).to_string())))
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
