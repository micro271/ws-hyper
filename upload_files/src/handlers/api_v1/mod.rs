pub(super) mod data_entry;
pub(super) mod file;
pub(super) mod user;
use http::{Method, Request, StatusCode, header};
use hyper::body::Incoming;
use mongodb::bson::doc;
use utils::{JwtCookie, Token};

use crate::handlers::cors;

use super::{
    ResultResponse,
    error::{ParseError, ResponseError},
};

pub async fn api(req: Request<Incoming>) -> ResultResponse {
    let path = req.uri().path().split("/v1").nth(1).unwrap_or_default();

    let headers = req.headers();

    if let Some(token) = Token::<JwtCookie>::get_token(headers) {
    } else {
        return Err(ResponseError::new(
            StatusCode::UNAUTHORIZED,
            Some("Token is not present"),
        ));
    }

    if req.method() == Method::OPTIONS {
        Ok(cors())
    } else if path.starts_with("/file") {
        file::file(req).await
    } else {
        Err(ResponseError::new(
            StatusCode::NOT_FOUND,
            Some(format!("Entpoint {} not found", req.uri())),
        ))
    }
}
