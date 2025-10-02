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
        },
        _ => Err(ResponseErr::new("Path not found", StatusCode::BAD_REQUEST)),
    };

    match resp {
        Ok(e) => Ok(e),
        Err(err) => Ok(err.into()),
    }
}

pub async fn api(req: Request<Incoming>) -> ResponseHandlers {
    let path = req.uri().path().strip_prefix("/api/v1/").unwrap_or_default();

    match (path, req.method()) {
        ("user", &Method::GET) => {
            let id = req.extensions().get::<Claim>().unwrap().sub;
            user::get(req, id).await
        }
        ("user", method @ ( &Method::POST | &Method::PATCH | &Method::DELETE) ) => {
            let id = req.extensions().get::<Claim>().unwrap().sub;
            let repo = req.extensions().get::<Repo>().unwrap();
            let user = repo.get::<User>(QueryOwn::builder().wh("id", id.into())).await?;
            if !user.is_admin() {
                return Err(ResponseErr::status(StatusCode::UNAUTHORIZED));
            }
            match method {
                &Method::PATCH => user::update(req, id).await,
                &Method::POST => user::new(req).await,
                &Method::DELETE => user::delete(req, id).await,
                _ => Err(ResponseErr::status(StatusCode::INTERNAL_SERVER_ERROR))
            }
        }
        ("users", &Method::GET) => user::get_all(req).await,
        ("users/info", &Method::GET) => user::get_user_info(req, None).await,
        (path,&Method::GET) if path.starts_with("user/info") => match path.strip_prefix("user/info").map(|x| x.parse::<Uuid>()) {
            Some(Ok(id)) => user::get_user_info(req, Some(id)).await,
            Some(Err(err)) => {
                tracing::error!("{err}");
                Err(ResponseErr::status(StatusCode::BAD_REQUEST))
            },
            _ => Err(ResponseErr::status(StatusCode::BAD_REQUEST)),
        }
        _ => Err(ResponseErr::new("Path not found", StatusCode::BAD_REQUEST))
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Login {
    username: String,
    password: String,
}
