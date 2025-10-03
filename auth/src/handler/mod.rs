mod entry;
mod error;
pub mod login;
pub mod user;

use http_body_util::Full;
use hyper::{
    Method, Request, Response, StatusCode,
    body::{Bytes, Incoming},
    http::Extensions,
};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc};
use utils::{JwtHandle, JwtHeader, Token, VerifyTokenEcdsa};
use uuid::Uuid;

use crate::{
    handler::error::ResponseErr,
    models::user::{Claim, User},
    repository::{PgRepository, QueryOwn},
};

type ResponseHandlers = Result<Response<Full<Bytes>>, ResponseErr>;
type Repo = Arc<PgRepository>;

pub async fn entry(mut req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let url = req.uri().path();

    let resp = match (url, req.method()) {
        ("/login", &Method::POST) => login::login(req).await,
        (path, _) if path.starts_with("/api/v1/") => {
            let Some(token) = Token::<JwtHeader>::get_token(req.headers()) else {
                return Ok(ResponseErr::status(StatusCode::UNAUTHORIZED).into());
            };

            let claim = match JwtHandle::verify_token::<Claim>(&token) {
                Ok(claim) => claim,
                Err(err) => return Ok(ResponseErr::new(err, StatusCode::UNAUTHORIZED).into()),
            };

            req.extensions_mut().insert(claim);
            api(req).await
        }
        _ => Err(ResponseErr::new("Path not found", StatusCode::BAD_REQUEST)),
    };

    match resp {
        Ok(e) => Ok(e),
        Err(err) => Ok(err.into()),
    }
}

pub async fn api(req: Request<Incoming>) -> ResponseHandlers {
    let path = req
        .uri()
        .path()
        .strip_prefix("/api/v1/")
        .unwrap_or_default();

    match (path, req.method().clone()) {
        ("user/self", Method::GET) => {
            let id = req.extensions().get::<Claim>().unwrap().sub;
            user::get(req, id).await
        }
        ("user", Method::POST) => user::new(req).await,
        (path, method @ (Method::PATCH | Method::DELETE | Method::GET)) if path.starts_with("user") => {
            let id_to_modify = path
                .strip_prefix("user/")
                .ok_or(ResponseErr::status(StatusCode::BAD_REQUEST))
                .and_then(|x| {
                    x.parse::<Uuid>()
                        .map_err(|_| ResponseErr::new("Invalid param", StatusCode::BAD_REQUEST))
                })?;
            let id = req.extensions().get::<Claim>().unwrap().sub;
            let repo = req.extensions().get::<Repo>().unwrap();
            let user = repo
                .get::<User>(QueryOwn::builder().wh("id", id.into()))
                .await?;
            if !user.is_admin() {
                return Err(ResponseErr::status(StatusCode::UNAUTHORIZED));
            }
            match method {
                Method::GET => user::get(req, id).await,
                Method::PATCH => user::update(req, id_to_modify).await,
                Method::DELETE => user::delete(req, id_to_modify).await,
                _ => Err(ResponseErr::status(StatusCode::INTERNAL_SERVER_ERROR)),
            }
        }
        ("users", Method::GET) => user::get_all(req).await,
        ("users/info", Method::GET) => user::get_user_info(req, None).await,
        (path, Method::GET) if path.starts_with("user/info") => {
            match path.strip_prefix("user/info").map(|x| x.parse::<Uuid>()) {
                Some(Ok(id)) => user::get_user_info(req, Some(id)).await,
                Some(Err(err)) => {
                    tracing::error!("{err}");
                    Err(ResponseErr::status(StatusCode::BAD_REQUEST))
                }
                _ => Err(ResponseErr::status(StatusCode::BAD_REQUEST)),
            }
        }
        _ => Err(ResponseErr::new("Path not found", StatusCode::BAD_REQUEST)),
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Login {
    username: String,
    password: String,
}

struct GetRepo;

impl GetRepo {
    pub fn get(ext: &Extensions) -> Result<&Repo, ResponseErr> {
        ext
        .get::<Repo>()
        .ok_or(ResponseErr::status(StatusCode::INTERNAL_SERVER_ERROR))
    }
}