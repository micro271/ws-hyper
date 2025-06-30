pub mod api_v1;
pub mod error;
pub mod program;
pub mod utils;

use crate::{handlers::api_v1::api, models::user::Claim};

use super::repository::Repository;
use ::utils::{JwtCookie, JwtHandle, Peer, Token, VerifyTokenEcdsa};
use bytes::Bytes;
use error::ResponseError;
use http::{Request, Response, StatusCode, header};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use serde::de::DeserializeOwned;
use std::{convert::Infallible, sync::Arc};
use utils::get_extention;

type ResultResponse = Result<Response<Full<Bytes>>, ResponseError>;
pub type State = Arc<Repository>;

pub async fn entry(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let method = req.method().clone();
    let peer = get_extention::<Peer>(req.extensions())
        .map(Peer::get_socket_or_unknown)
        .unwrap_or_default();

    tracing::debug!(
        "Request:  {{ Method: {}, Uri: {}, Src: {}, Version: {:#?}, Headers: {:#?} }}",
        method,
        req.uri(),
        peer,
        req.version(),
        req.headers()
    );

    let duration = std::time::Instant::now();
    let path = req.uri().path().to_string();

    let duration = duration.elapsed().as_millis();

    match midlleware_token(req).await {
        Ok(r) => {
            tracing::info!(
                "Response {{ Method: {}, Status: {}, Path={}, duration: {}ms, Src_req: {} }}",
                method,
                r.status(),
                path,
                duration,
                peer,
            );
            Ok(r)
        }
        Err(e) => {
            tracing::error!(
                "Response {{ Method: {}, Status: {}, Path={}, duration: {}ms, Src_req: {}, error: {:?} }}",
                method,
                e.status(),
                path,
                duration,
                peer,
                e.detail(),
            );
            Ok(e.into())
        }
    }
}

pub async fn midlleware_token(mut req: Request<Incoming>) -> ResultResponse {
    if let Some(token) = Token::<JwtCookie>::get_token(req.headers()) {
        let tmp = JwtHandle::verify_token::<Claim>(&token)
            .map_err(|_| ResponseError::new::<&str>(StatusCode::UNAUTHORIZED, None))?;
        req.extensions_mut().insert(tmp);

        api(req).await
    } else {
        Err(ResponseError::new(
            StatusCode::UNAUTHORIZED,
            Some("Token is not present"),
        ))
    }
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
        .body(Full::default())
        .unwrap_or_default()
}
