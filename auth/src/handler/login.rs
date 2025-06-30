use bcrypt::verify;
use http_body_util::Full;
use hyper::{
    Request, Response, StatusCode,
    body::{Bytes, Incoming},
    header,
};
use jsonwebtoken::Header;
use serde_json::json;
use std::convert::Infallible;
use utils::{GenTokenFromEcds, JwtHandle, ParseBodyToJson};

use crate::{Repository, handler::Login};

pub async fn login(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let (parts, body) = req.into_parts();
    let repo = parts.extensions.get::<Repository>().unwrap();

    match ParseBodyToJson::<Login>::get(body).await {
        Ok(e) => {
            let user = repo.get_user(&e.username).await.unwrap();
            match verify(e.password, &user.passwd) {
                Ok(true) => match JwtHandle::gen_token(user) {
                    Ok(e) => Ok(Response::builder()
                        .header(header::CONTENT_TYPE, "application/json")
                        .status(StatusCode::OK)
                        .body(Full::new(Bytes::from(json!({"token": e}).to_string())))
                        .unwrap_or_default()),
                    Err(e) => Ok(Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .body(Full::default())
                        .unwrap_or_default()),
                },
                _ => Ok(Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body(Full::default())
                    .unwrap_or_default()),
            }
        }
        Err(e) => Ok(Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Full::default())
            .unwrap_or_default()),
    }
}
