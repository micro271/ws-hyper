mod bucket;
pub mod entry;
pub mod error;
pub mod login;
pub mod user;

use http_body_util::Full;
use hyper::{
    Method, Request, Response, StatusCode,
    body::{Bytes, Incoming},
    http::Extensions,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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
    state::{PgRepository, QueryOwn},
};

type ResponseHandlers = Result<Response<Full<Bytes>>, ResponseErr>;
type Repo = Arc<PgRepository>;

const PREFIX_PATH: &str = "/api/v1";

pub async fn api(req: Request<Incoming>) -> ResponseHandlers {
    let path = req.uri().path().strip_prefix(PREFIX_PATH).unwrap();
    if path == "/user" {
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
        ("/bucket", Method::POST) => bucket::new(req).await,
        ("/bucket", Method::GET) => bucket::get_all(req).await,
        (program, method @ (Method::PATCH | Method::GET | Method::DELETE))
            if path.starts_with("/bucket") =>
        {
            let program = program
                .strip_prefix("/bucket/")
                .ok_or(ResponseErr::status(StatusCode::BAD_REQUEST))
                .and_then(|x| {
                    x.parse::<Uuid>()
                        .map_err(|_| ResponseErr::new("Invalid format", StatusCode::BAD_REQUEST))
                })?;
            if Method::PATCH == method {
                bucket::update(req, program).await
            } else if Method::DELETE == method {
                bucket::delete(req, program).await
            } else {
                bucket::get(req, program).await
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
