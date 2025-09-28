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
use uuid::Uuid;

use crate::{
    handler::{error::ResponseErr, user::get},
    models::user::{Claim, Role, User},
    repository::{PgRepository, QueryOwn},
};

type ResponseHandlers = Result<Response<Full<Bytes>>, ResponseErr>;

pub async fn entry(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let url = req.uri().path();

    let resp = match (url, req.method()) {
        ("/login", &Method::POST) => login::login(req).await,

        (path, _) if path.starts_with("/api/v1/user") => {
            let Some(token) = Token::<JwtHeader>::get_token(req.headers()) else {
                return Ok(ResponseErr::status(StatusCode::UNAUTHORIZED).into());
            };

            let id = match JwtHandle::verify_token::<Claim>(&token) {
                Ok(claim) => claim.sub,
                Err(err) => return Ok(ResponseErr::new(err, StatusCode::UNAUTHORIZED).into()),
            };

            let path = req
                .uri()
                .path()
                .strip_prefix("/api/v1/user/")
                .map(ToString::to_string);
            let repo = req.extensions().get::<PgRepository>().unwrap();
            let Ok(user) = repo
                .get(QueryOwn::<User>::builder().wh("id", id.into()))
                .await
            else {
                return Ok(ResponseErr::status(StatusCode::BAD_REQUEST).into());
            };

            match (req.method().clone(), path) {
                (Method::POST, None) => {
                    if user.role != Role::Administrator {
                        return Ok(ResponseErr::status(StatusCode::UNAUTHORIZED).into());
                    }
                    user::new(req).await
                }
                (Method::DELETE, Some(uuid)) => {
                    let uuid = uuid.parse::<Uuid>().unwrap();

                    user::delete(req, uuid).await
                }
                (Method::PATCH, Some(uuid)) => {
                    let uuid = uuid.parse::<Uuid>().unwrap();

                    if uuid != user.id.unwrap() && user.role != Role::Administrator {
                        return Ok(ResponseErr::status(StatusCode::UNAUTHORIZED).into());
                    }

                    user::update(req, uuid).await
                }
                (Method::GET, some) => get(req, some.unwrap().parse::<Uuid>().ok()).await,
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
