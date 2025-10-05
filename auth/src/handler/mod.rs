mod entry;
pub mod error;
pub mod login;
mod programas;
pub mod user;

use http_body_util::Full;
use hyper::{
    Method, Request, Response, StatusCode,
    body::{Bytes, Incoming},
    http::Extensions,
};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc, time::Instant};
use utils::{JwtHandle, JwtHeader, Peer, Token, VerifyTokenEcdsa};
use uuid::Uuid;

use crate::{
    handler::{
        error::ResponseErr,
        user::{delete, get, get_user_info, update},
    },
    models::user::{
        Claim, User,
        update::{UpdateSelf, UpdateUser},
    },
    repository::{PgRepository, QueryOwn},
};

type ResponseHandlers = Result<Response<Full<Bytes>>, ResponseErr>;
type Repo = Arc<PgRepository>;

const PREFIX_PATH: &str = "/api/v1";

pub async fn entry(mut req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let url = req.uri().path();

    let instant = Instant::now();

    tracing::info!(
        "{{ Request HTTP  }} [ version {:?}, method {}, path {}, peer {}, content-lenth: {:?} ]",
        req.version(),
        req.method(),
        url,
        req.extensions().get::<Peer>().unwrap().get_ip_or_unknown(),
        req.headers().get(hyper::http::header::CONTENT_LENGTH),
    );

    let resp = match (url, req.method()) {
        ("/login", &Method::POST) => login::login(req).await,
        (path, _) if path.starts_with(PREFIX_PATH) => {
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
        Ok(e) => {
            tracing::info!(
                "{{ Response HTTP }} [ STATUS {}, latency {}]",
                e.status(),
                instant.elapsed().as_millis()
            );
            Ok(e)
        }
        Err(err) => {
            tracing::error!(
                "{{ Response HTTP }} [ {}, latency {} ms ]",
                err,
                instant.elapsed().as_millis()
            );
            Ok(err.into())
        }
    }
}

pub async fn api(req: Request<Incoming>) -> ResponseHandlers {
    let path = req.uri().path().strip_prefix(PREFIX_PATH).unwrap();
    if path == "/user/self" {
        let id = req.extensions().get::<Claim>().unwrap().sub;

        return match req.method().clone() {
            Method::PATCH => user::update::<UpdateSelf>(req, id).await,
            Method::GET => user::get_user_info(req, Some(id)).await,
            _ => Err(ResponseErr::new("Path not found", StatusCode::BAD_REQUEST)),
        };
    }

    middleware_user_admin(req, endpoint_admin).await
}

pub async fn endpoint_admin(req: Request<Incoming>) -> ResponseHandlers {
    let path = req.uri().path().strip_prefix(PREFIX_PATH).unwrap();

    match (path, req.method().clone()) {
        ("/user", Method::POST) => user::new(req).await,
        ("/users", Method::GET) => user::get_all(req).await,
        (path, Method::GET) if path.starts_with("user/") && path.ends_with("/detail") => {
            let path = path
                .split("/user/")
                .nth(1)
                .and_then(|x| x.strip_suffix("/detail").and_then(|x| x.parse().ok()))
                .ok_or(ResponseErr::new("Invalid format", StatusCode::BAD_REQUEST))?;
            user::get_user_info(req, Some(path)).await
        }
        ("/users/detail", Method::GET) => get_user_info(req, None).await,
        (path, method @ (Method::DELETE | Method::PATCH | Method::GET))
            if path.starts_with("/user/") =>
        {
            let id = path
                .split("/user/")
                .nth(1)
                .and_then(|x| x.parse::<Uuid>().ok())
                .ok_or(ResponseErr::new("Invalid format", StatusCode::BAD_REQUEST))?;
            if Method::DELETE == method {
                delete(req, id).await
            } else if Method::PATCH == method {
                update::<UpdateUser>(req, id).await
            } else {
                get(req, id).await
            }
        }
        ("/programa", Method::POST) => programas::new(req).await, /* To excecute this action, it is required que the microservice directory is ready */
        ("/programas", Method::GET) => programas::get_all(req).await,
        (program, method @ (Method::PATCH | Method::GET | Method::DELETE))
            if path.starts_with("/programa") =>
        {
            let program = program
                .strip_prefix("/programa/")
                .ok_or(ResponseErr::status(StatusCode::BAD_REQUEST))
                .and_then(|x| {
                    x.parse::<Uuid>()
                        .map_err(|_| ResponseErr::new("Invalid format", StatusCode::BAD_REQUEST))
                })?;
            if Method::PATCH == method {
                programas::update(req, program).await
            } else if Method::DELETE == method {
                programas::delete(req, program).await
            } else {
                programas::get(req, program).await
            }
        }
        _ => Err(ResponseErr::new("Path Not Found", StatusCode::BAD_REQUEST)),
    }
}

pub async fn middleware_user_admin<F>(req: Request<Incoming>, next: F) -> ResponseHandlers
where
    F: AsyncFnOnce(Request<Incoming>) -> ResponseHandlers,
{
    let id = req.extensions().get::<Claim>().unwrap().sub;
    let repo = GetRepo::get(req.extensions())?;

    if !repo
        .get::<User>(QueryOwn::builder().wh("id", id))
        .await?
        .is_admin()
    {
        return Err(ResponseErr::status(StatusCode::UNAUTHORIZED));
    }

    next(req).await
}

#[derive(Debug, Deserialize, Serialize)]
struct Login {
    username: String,
    password: String,
}

struct GetRepo;

impl GetRepo {
    pub fn get(ext: &Extensions) -> Result<&Repo, ResponseErr> {
        ext.get::<Repo>().ok_or(ResponseErr::new(
            "State not found",
            StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}
