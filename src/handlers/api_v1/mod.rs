pub mod file;
pub mod user;

use std::sync::Arc;

use crate::{
    models::user::{Claims, User, UserEntry},
    repository::Repository,
};
use bcrypt::verify;
use bytes::Bytes;
use http::{HeaderMap, Request, Response, StatusCode, header};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use mongodb::bson::doc;

use super::{
    ResponseWithError,
    error::{ParseError, ResponseError},
};

type Res = Result<Response<Full<Bytes>>, ResponseError>;

const JWT_IDENTIFIED: &str = "JWT";

pub async fn api(req: Request<Incoming>, repository: Arc<Repository>, claim: Claims) -> Res {
    let path = req.uri().path().split("/api/v1").nth(1).unwrap_or_default();

    if path.starts_with("/file") {
        return file::file(req, repository).await;
    } else if path.starts_with("/user") {
        return user::user(req, repository).await;
    }

    Err(ResponseError::new(
        StatusCode::NOT_FOUND,
        format!("Entpoint {} not found", req.uri()),
    ))
}

pub async fn login(req: Request<Incoming>, repository: Arc<Repository>) -> Res {
    let body = req.into_body();
    let check_user = match body.collect().await {
        Ok(e) => match serde_json::from_slice::<'_, UserEntry>(&e.to_bytes()) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("Fail to deserialize the data entry - Err: {e}");
                return Err(ResponseError::new(
                    StatusCode::BAD_REQUEST,
                    "Fail to deserialize the data entry".to_string(),
                ));
            }
        },
        Err(e) => {
            tracing::error!("Error to obtain the UserEntry - Error: {e}");
            return Err(ResponseError::new(
                StatusCode::BAD_REQUEST,
                "Credential is not present".to_string(),
            ));
        }
    };

    let user = repository
        .get_one::<User>(doc! {"username": check_user.username})
        .await
        .ok_or(ResponseError::new(
            StatusCode::BAD_REQUEST,
            "username not exists".to_string(),
        ))?;

    if verify(check_user.password, &user.password).unwrap_or(false) {
        tracing::info!(
            "Login succesful: [ _id: {}, username: {}, role: {} ]",
            user._id.unwrap(),
            user.username,
            user.role,
        );

        match encode(
            &Header::default(),
            &Claims::from(user),
            &EncodingKey::from_secret("SECRET".as_ref()),
        ) {
            Ok(token) => {
                let age = time::Duration::hours(2).whole_hours();
                let same_site = "Strict";
                let cookie = format!(
                    "jwt={token}; HttpOnly; Secure; SameSite={same_site}; Path=/; Max-Age={age}"
                );
                Ok(Response::builder()
                    .status(StatusCode::SEE_OTHER)
                    .header(header::SET_COOKIE, cookie)
                    .header(header::LOCATION, "/")
                    .body(Full::new(Bytes::new()))
                    .unwrap_or_default())
            }
            Err(e) => {
                tracing::error!("Fail to create the token - Err: {e}");
                Err(ResponseError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failt to create the token".to_string(),
                ))
            }
        }
    } else {
        tracing::error!(
            "Login failure: [ _id: {}, username: {} ]",
            user._id.unwrap(),
            user.username
        );
        Err(ResponseError {
            status: StatusCode::UNAUTHORIZED,
            detail: "Username or password error".to_string(),
        })
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
