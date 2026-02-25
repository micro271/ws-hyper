pub mod error;

use crate::{
    bucket::{Bucket, key::Key},
    handlers::error::ResponseError,
    state::State,
    user::Claim,
};

use http::{StatusCode, header};
use http_body_util::Full;
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
};
use serde_json::json;
use std::{convert::Infallible, sync::Arc};
use utils::{JwtBoth, JwtHandle, Token, VerifyTokenEcdsa};

type TypeState = Arc<State>;

pub type ResponseHttp = Result<Response<Full<Bytes>>, ResponseError>;

pub async fn entry(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();

    let response = if path == "login" {
        Ok(not_found(req).await.into())
    } else if path.starts_with("/monitor") {
        middleware_jwt(req, server_upgrade).await
    } else if path.starts_with("/tree") {
        let mut path = path.split("/").skip(2).collect::<Vec<_>>();
        let (bucket, key) = if path.is_empty() {
            let tmp = req.extensions().get::<TypeState>().unwrap();
            return Ok(Response::new(Full::new(Bytes::from(
                tmp.tree_as_json().await,
            ))));
            //return Ok(not_found(req).await.into());
        } else if path.len() == 1 {
            (
                Some(Bucket::new_unchecked(path.pop().unwrap()).owned()),
                None,
            )
        } else {
            (
                Some(Bucket::new_unchecked(path.pop().unwrap()).owned()),
                Some(Key::from(path.drain(1..).collect::<Vec<_>>().join("/"))),
            )
        };

        middleware_jwt(req, async |x| get_path(x, bucket, key).await).await
    } else {
        Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::default())
            .unwrap_or_default())
    };

    match response {
        Err(er) => Ok(er.into()),
        Ok(ok) => Ok(ok),
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

async fn middleware_jwt<T>(mut req: Request<Incoming>, next: T) -> ResponseHttp
where
    T: AsyncFnOnce(Request<Incoming>) -> ResponseHttp,
{
    let Some(token) = Token::<JwtBoth>::get_token(req.headers()) else {
        return Ok(Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Full::default())
            .unwrap_or_default());
    };

    let claims = match JwtHandle::verify_token::<Claim>(&token) {
        Ok(claims) => claims,
        Err(err) => {
            tracing::error!("[Midleware jwt] {err}");
            return Ok(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Full::default())
                .unwrap_or_default());
        }
    };
    req.extensions_mut().insert(claims);

    next(req).await
}

pub async fn server_upgrade(
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, ResponseError> {
    let state = req.extensions().get::<TypeState>().unwrap().clone();
    let path = req
        .uri()
        .path()
        .strip_prefix("/monitor/")
        .ok_or(ResponseError::new(
            format!("{} not found", req.uri().path()),
            StatusCode::BAD_REQUEST,
        ))?;

    let mut path = path.split("/");
    let bucket = Bucket::new_unchecked(
        path.next()
            .ok_or(ResponseError::status(StatusCode::BAD_REQUEST))?,
    )
    .owned();
    let key = Key::new(path.next().unwrap_or_default().to_string());
    if hyper_tungstenite::is_upgrade_request(&req) {
        let (res, ws) = hyper_tungstenite::upgrade(req, None).unwrap();
        state.add_client(bucket, key, ws).await;
        Ok(res)
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
