pub mod error;

use crate::{
    handlers::error::ResponseError,
    state::{self, State},
    user::Claim,
};

use http::{StatusCode, header};
use http_body_util::Full;
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
};
use std::{convert::Infallible, sync::Arc};
use utils::{JwtHandle, JwtHeader, Token, VerifyTokenEcdsa};

type TypeState = Arc<State>;

pub async fn entry(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let path = req.uri().path();
    let repo = req.extensions().get::<TypeState>().unwrap();

    let response = if path.starts_with("/monitor") {
        server_upgrade(req).await
    } else if path == "/tree" {
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Full::new(Bytes::from(
                repo.tree_as_json().await.to_string(),
            )))
            .unwrap_or_default())
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

pub async fn middleware_jwt<T>(
    mut req: Request<Incoming>,
    next: T,
) -> Result<Response<Full<Bytes>>, Infallible>
where
    T: AsyncFn(Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible>,
{
    /*
    let Some(token) = Token::<JwtHeader>::get_token(req.headers()) else {
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
    */

    next(req).await
}

pub async fn server_upgrade(
    req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, ResponseError> {
    let state = req.extensions().get::<TypeState>().unwrap().clone();
    let path = req
        .uri()
        .path()
        .strip_prefix("/monitor")
        .ok_or(ResponseError::new(
            format!("{} not found", req.uri().path()),
            StatusCode::BAD_REQUEST,
        ))?
        .to_string();

    if hyper_tungstenite::is_upgrade_request(&req) {
        let (res, ws) = hyper_tungstenite::upgrade(req, None).unwrap();
        state.add_client(path, ws).await;
        Ok(res)
    } else {
        Ok(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Full::default())
            .unwrap_or_default())
    }
}
