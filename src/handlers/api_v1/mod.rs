pub mod data_entry;
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
    ResultResponse,
    error::{ParseError, ResponseError},
};

const JWT_IDENTIFIED: &str = "JWT";
const ALGORITHM: Algorithm = Algorithm::HS256;

pub async fn api(req: Request<Incoming>) -> ResultResponse {
    let path = req.uri().path().split("/api/v1").nth(1).unwrap_or_default();

    if path.starts_with("/file") {
        file::file(req).await
    } else if path.starts_with("/user") {
        user::user(req).await
    } else {
        Err(ResponseError::new(
            StatusCode::NOT_FOUND,
            Some(format!("Entpoint {} not found", req.uri())),
        ))
    }
}

pub async fn login(req: Request<Incoming>) -> ResultResponse {
    let (parts, body) = req.into_parts();
    let repository = parts.extensions.get::<Arc<Repository>>().unwrap();
    let check_user = match body.collect().await {
        Ok(e) => match serde_json::from_slice::<'_, UserEntry>(&e.to_bytes()) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("Fail to deserialize the data entry - Err: {e}");
                return Err(ResponseError::new(
                    StatusCode::BAD_REQUEST,
                    Some("Fail to deserialize the data entry"),
                ));
            }
        },
        Err(e) => {
            tracing::error!("Error to obtain the UserEntry - Error: {e}");
            return Err(ResponseError::new(
                StatusCode::BAD_REQUEST,
                Some("Credential is not present"),
            ));
        }
    };

    let user = repository
        .get_one::<User>(doc! {"username": check_user.username})
        .await?;

    if verify(check_user.password, &user.password).unwrap_or(false) {
        tracing::info!(
            "Login succesful: [ _id: {}, username: {}, role: {} ]",
            user.id.unwrap(),
            user.username,
            user.role,
        );
        let claims = Claims::from(user);
        let header = Header::new(ALGORITHM);
        tracing::debug!("{claims:?}");
        match encode(
            &header,
            &claims,
            &EncodingKey::from_secret("SECRET".as_ref()),
        ) {
            Ok(token) => {
                let age = time::Duration::hours(2).whole_seconds();
                let same_site = "Strict";
                let cookie = format!(
                    "{JWT_IDENTIFIED}={token}; HttpOnly; Secure; SameSite={same_site}; Path=/; Max-Age={age}"
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
                    Some(e.to_string()),
                ))
            }
        }
    } else {
        tracing::error!(
            "Login failure: [ _id: {}, username: {} ]",
            user.id.unwrap(),
            user.username
        );
        Err(ResponseError::new(
            StatusCode::UNAUTHORIZED,
            Some("Username or password error"),
        ))
    }
}

pub async fn verifi_token_from_cookie(headers: &HeaderMap) -> Option<Claims> {
    match headers.get(http::header::COOKIE).map(|x| x.to_str()) {
        Some(Ok(pair)) => {
            let Some(token_pair) = pair.split(';').find(|x| x.starts_with(JWT_IDENTIFIED)) else {
                tracing::error!("Cookie key {} not found", JWT_IDENTIFIED);
                return None;
            };

            let Some(token) = token_pair.split('=').nth(1) else {
                tracing::error!(
                    "Cookie key {} present but it have not value",
                    JWT_IDENTIFIED
                );
                return None;
            };
            tracing::debug!("{token}");

            match decode::<Claims>(
                token,
                &DecodingKey::from_secret("SECRET".as_ref()),
                &Validation::new(ALGORITHM),
            ) {
                Ok(e) => {
                    tracing::debug!("Claims obtains: {:?}", e.claims);
                    Some(e.claims)
                }
                Err(e) => {
                    tracing::error!("Fail authentication: {e}");
                    None
                }
            }
        }
        Some(Err(e)) => {
            tracing::error!("Error to pasing to str - Err: {}", e);
            None
        }
        None => {
            tracing::error!("Header COOKIE is not present");
            None
        }
    }
}
