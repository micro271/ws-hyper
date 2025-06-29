pub(super) mod data_entry;
pub(super) mod file;
use http::{Method, Request, StatusCode, header};
use hyper::body::Incoming;
use mongodb::bson::doc;

use crate::handlers::cors;

use super::{
    ResultResponse,
    error::{ParseError, ResponseError},
};

pub async fn api(req: Request<Incoming>) -> ResultResponse {
    let path = req.uri().path().split("/v1").nth(1).unwrap_or_default();

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
