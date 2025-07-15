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

            let path = req.uri().path().strip_prefix("/api/v1/user/");

            match (req.method().clone(), path) {
                (Method::POST, None) => user::new(req).await,
                (Method::DELETE, Some(uuid)) => {
                    if let Ok(uuid) = uuid.parse() {
                        user::delete(req, uuid).await
                    } else {
                        Err(ResponseErr::status(StatusCode::BAD_REQUEST))
                    }
                }
                (Method::PATCH, Some(uuid)) => {
                    if let Ok(uuid) = uuid.parse() {
                        user::update(req, uuid).await
                    } else {
                        Err(ResponseErr::status(StatusCode::BAD_REQUEST))
                    }
                }
                (Method::GET, Some(path)) => {
                    println!("SI");
                    let mut path = path.split(':');
                    let uuid = path.next().and_then(|x| x.parse().ok());
                    let extend = path.next().map(|x| x.eq("extend")).unwrap_or_default();
                    get(req, uuid, extend).await
                }
                _ => Err(ResponseErr::status(StatusCode::BAD_REQUEST)),
            }
        }
        (path, &Method::GET) if path.starts_with("/api/v1/users") => {
            Err(ResponseErr::status(StatusCode::NOT_IMPLEMENTED))
        }
        _ => Err(ResponseErr::status(StatusCode::BAD_REQUEST)),
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
