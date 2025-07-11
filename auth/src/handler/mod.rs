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
use utils::{JwtHandle, JwtHeader, Token, VerifyTokenEcdsa};

use crate::{
    handler::{error::ResponseErr, user::get},
    models::user::Claim,
};

type ResponseHandlers = Result<Response<Full<Bytes>>, ResponseErr>;

pub async fn entry(mut req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let url = req.uri().path();

    let resp = match (url, req.method()) {
        ("/login", &Method::POST) => login::login(req).await,
        (path, _) if path.starts_with("/api/v1/user") => {
            let Some(token) = Token::<JwtHeader>::get_token(req.headers()) else {
                return Ok(ResponseErr::status(StatusCode::UNAUTHORIZED).into());
            };
            let claim = match JwtHandle::verify_token::<Claim>(&token) {
                Ok(claim) => claim,
                Err(err) => return Ok(ResponseErr::new(err, StatusCode::UNAUTHORIZED).into()),
            };
            req.extensions_mut().insert(claim);

            let uuid = req
                .uri()
                .path()
                .strip_prefix("/api/v1/user/")
                .and_then(|x| x.parse().ok());

            match *req.method() {
                Method::POST if uuid.is_none() => user::new(req).await,
                Method::GET => get(req, uuid).await,
                Method::DELETE if uuid.is_some() => user::delete(req, uuid.unwrap()).await,
                Method::PATCH if uuid.is_some() => user::update(req, uuid.unwrap()).await,
                _ => Ok(ResponseErr::status(StatusCode::BAD_REQUEST).into()),
            }
        }
        _ => Ok(ResponseErr::status(StatusCode::BAD_REQUEST).into()),
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
