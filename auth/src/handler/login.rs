use bcrypt::verify;
use http_body_util::Full;
use hyper::{
    Request, Response, StatusCode,
    body::{Bytes, Incoming},
    header,
};
use serde_json::json;
use utils::{GenTokenFromEcds, JwtHandle, ParseBodyToJson};

use crate::{
    Repository,
    handler::{Login, error::ResponseErr},
    models::user::User,
    repository::QueryResult,
};

pub async fn login(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, ResponseErr> {
    let (parts, body) = req.into_parts();
    let repo = parts.extensions.get::<Repository>().unwrap();

    match ParseBodyToJson::<Login>::get(body).await {
        Ok(login) => {
            let QueryResult::SelectOne(user) = repo
                .get_user::<User, _>(("username", login.username))
                .await?
            else {
                return Err(ResponseErr::status(StatusCode::INTERNAL_SERVER_ERROR));
            };

            match verify(login.password, &user.passwd) {
                Ok(true) => match JwtHandle::gen_token(user) {
                    Ok(e) => Ok(Response::builder()
                        .header(header::CONTENT_TYPE, "application/json")
                        .status(StatusCode::OK)
                        .body(Full::new(Bytes::from(json!({"token": e}).to_string())))
                        .unwrap_or_default()),
                    Err(_e) => Ok(Response::builder()
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
        Err(_e) => Ok(Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Full::default())
            .unwrap_or_default()),
    }
}
