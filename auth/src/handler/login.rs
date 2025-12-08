use bcrypt::verify;
use cookie::CookieBuilder;
use http_body_util::Full;
use hyper::{
    Request, Response, StatusCode,
    body::{Bytes, Incoming},
    header,
};
use serde_json::json;
use time::Duration;
use utils::{GenTokenFromEcds, JWT_IDENTIFIED, JwtHandle, ParseBodyToStruct};

use crate::{
    Repository,
    handler::{Login, error::ResponseErr},
    models::user::User,
    state::QueryOwn,
};

#[tracing::instrument]
pub async fn login(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, ResponseErr> {
    let (parts, body) = req.into_parts();
    let repo = parts.extensions.get::<Repository>().unwrap();
    match ParseBodyToStruct::<Login>::get(body).await {
        Ok(login) => {
            let Ok(user) = repo
                .get(QueryOwn::<User>::builder().wh("username", login.username))
                .await
            else {
                tracing::error!("no entro");
                return Err(ResponseErr::new("Parse error", StatusCode::NOT_FOUND));
            };

            match verify(login.password, &user.passwd) {
                Ok(true) => match JwtHandle::gen_token(user) {
                    Ok(e) => {
                        let cookie = CookieBuilder::new(JWT_IDENTIFIED, e.clone())
                            .path("/")
                            .max_age(Duration::hours(6))
                            .http_only(false)
                            .same_site(cookie::SameSite::Lax)
                            .secure(false)
                            .build();
                        tracing::info!("Login successfull");
                        Ok(Response::builder()
                            .header(header::CONTENT_TYPE, "application/json")
                            .header(header::SET_COOKIE, cookie.to_string())
                            .status(StatusCode::OK)
                            .body(Full::new(Bytes::from(json!({"token": e}).to_string())))
                            .unwrap_or_default())
                    }
                    Err(err) => Err(ResponseErr::new(err, StatusCode::BAD_REQUEST)),
                },
                _ => Err(ResponseErr::status(StatusCode::UNAUTHORIZED)),
            }
        }
        Err(e) => Err(ResponseErr::new(e, StatusCode::UNAUTHORIZED)),
    }
}
