use bcrypt::verify;
use cookie::CookieBuilder;
use http_body_util::Full;
use hyper::{
    Method, Response, StatusCode,
    body::{Bytes, Incoming},
    header,
};
use serde_json::json;
use std::convert::Infallible;
use time::Duration;
use utils::{
    GenTokenFromEcds as _, JWT_IDENTIFIED, JwtBoth, JwtHandle, ParseBodyToStruct, Token,
    VerifyTokenEcdsa, claim::Claim, middleware::Layer,
};

use crate::{
    HOST,
    handler::{Login, PREFIX_PATH, Repo, api, error::ResponseErr},
    models::user::User,
    state::QueryOwn,
};

#[derive(Debug, Clone)]
pub struct EPoint;

impl Layer<Incoming> for EPoint {
    type Error = Infallible;
    type Response = Full<Bytes>;
    async fn call(
        &self,
        mut req: hyper::Request<Incoming>,
    ) -> Result<hyper::Response<Self::Response>, Self::Error> {
        let url = req.uri().path();
        let resp = match (url, req.method()) {
            ("/login", &Method::POST) => {
                let (parts, body) = req.into_parts();
                let Some(repo) = parts.extensions.get::<Repo>() else {
                    tracing::error!("Repository not found");
                    return Ok(ResponseErr::status(StatusCode::INTERNAL_SERVER_ERROR).into());
                };
                match ParseBodyToStruct::<Login>::get(body).await {
                    Ok(login) => {
                        let Ok(user) = repo
                            .get(QueryOwn::<User>::builder().wh("username", login.username.clone()))
                            .await
                        else {
                            tracing::warn!("Username Not found - Data: {:?}", login);
                            return Ok(
                                ResponseErr::new("User not found", StatusCode::NOT_FOUND).into()
                            );
                        };

                        match verify(login.password, &user.passwd) {
                            Ok(true) => {
                                let mut claim: Claim<uuid::Uuid> = user.into();
                                claim.set_iss(HOST.host());
                                match JwtHandle::gen_token(claim) {
                                    Ok(e) => {
                                        let cookie = CookieBuilder::new(JWT_IDENTIFIED, e.clone())
                                            .path("/")
                                            .max_age(Duration::hours(6))
                                            .http_only(true)
                                            .same_site(cookie::SameSite::Lax)
                                            .secure(true)
                                            .build();
                                        tracing::info!("Login successfull");
                                        Ok(Response::builder()
                                            .header(header::CONTENT_TYPE, "application/json")
                                            .header(header::SET_COOKIE, cookie.to_string())
                                            .status(StatusCode::OK)
                                            .body(Full::new(Bytes::from(
                                                json!({"token": e}).to_string(),
                                            )))
                                            .unwrap_or_default())
                                    }
                                    Err(err) => {
                                        tracing::error!(
                                            "[ Entry ] JwtHandleError Parse error: {err}"
                                        );
                                        Err(ResponseErr::new(err, StatusCode::BAD_REQUEST))
                                    }
                                }
                            }
                            _ => Err(ResponseErr::status(StatusCode::UNAUTHORIZED)),
                        }
                    }
                    Err(e) => {
                        tracing::error!("Parse error {e}");
                        Err(ResponseErr::new(e, StatusCode::UNAUTHORIZED))
                    }
                }
            }
            (path, _) if path.starts_with(PREFIX_PATH) => {
                let Some(token) = Token::<JwtBoth>::get_token(req.headers()) else {
                    return Ok(
                        ResponseErr::new("Token not found", StatusCode::UNAUTHORIZED).into(),
                    );
                };

                let claim = match JwtHandle::verify_token::<Claim<uuid::Uuid>>(&token) {
                    Ok(claim) => claim,
                    Err(err) => return Ok(ResponseErr::new(err, StatusCode::UNAUTHORIZED).into()),
                };

                req.extensions_mut().insert(claim);
                api(req).await
            }
            _ => Err(ResponseErr::new("Path not found", StatusCode::BAD_REQUEST)),
        };

        Ok(match resp {
            Ok(e) => e,
            Err(er) => er.into(),
        })
    }
}
