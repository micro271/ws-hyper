pub mod file;
pub mod user;

use std::sync::Arc;

use crate::{
    models::user::{Claims, UserEntry},
    repository::Repository,
};
use bcrypt::verify;
use bytes::Bytes;
use http::{HeaderMap, Request, Response, StatusCode, header};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};

use super::{
    ResponseWithError,
    error::{ParseError, ResponseError},
};

type Res = Result<Response<Full<Bytes>>, ResponseError>;

const JWT_IDENTIFIED: &str = "JWT";

pub async fn api(req: Request<Incoming>, repository: Arc<Repository>) -> Res {
    let path = req.uri().path().split("/api/v1").nth(1).unwrap_or_default();

    if path.starts_with("/file") {
        return file::file(req, repository).await;
    } else if path == "/login" {
        return login(req, repository).await;
    } else if path.starts_with("/user") {
        return user::user(req, repository).await;
    }

    Err(ResponseError::new(
        StatusCode::NOT_FOUND,
        format!("Entpoint {} not found", req.uri()),
    ))
}

pub async fn login(req: Request<Incoming>, _repository: Arc<Repository>) -> Res {
    let body = req.into_body();
    let check_user = body
        .collect()
        .await
        .map(|x| serde_json::from_slice::<'_, UserEntry>(&x.to_bytes()));

    match check_user {
        Ok(Ok(e)) => {
            if verify(e.password, "prueba").unwrap_or(false) {
                tracing::info!("Login succesful: [username: {}]", e.username);
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(header::SET_COOKIE, "algo")
                    .header(header::LOCATION, "/")
                    .body(Full::new(Bytes::new()))
                    .unwrap_or_default())
            } else {
                tracing::error!("Login failure: [username: {}]", e.username);
                Err(ResponseError {
                    status: StatusCode::UNAUTHORIZED,
                    detail: "Username or password error".to_string(),
                })
            }
        }
        Ok(Err(e)) => {
            tracing::error!("Bcrypt Err: {e}");
            Err(ResponseError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                detail: e.to_string(),
            })
        }
        Err(e) => {
            tracing::info!("UserEntry is not present - Error: {}", e);
            Err(ResponseError {
                status: StatusCode::BAD_REQUEST,
                detail: "User's values is not present".to_string(),
            })
        }
    }
}

pub async fn verifi_token_from_cookie(headers: &HeaderMap) -> Option<Claims> {
    headers
        .get(http::header::COOKIE)
        .and_then(|x| x.to_str().ok())
        .and_then(|x| {
            x.split(';')
                .find(|x| x.starts_with(JWT_IDENTIFIED))
                .and_then(|x| x.split('=').nth(1))
        })
        .and_then(|x| {
            decode::<Claims>(
                x,
                &DecodingKey::from_secret("SECRET".as_ref()),
                &Validation::new(Algorithm::ES256),
            )
            .ok()
            .map(|x| x.claims)
        })
}
