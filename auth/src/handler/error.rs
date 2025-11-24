use std::fmt::{Debug, Display};

use http_body_util::Full;
use hyper::{Response, StatusCode, Version, body::Bytes};

use crate::{models::user::EncryptErr, state::error::RepositoryError};

#[derive(Debug)]
pub struct ResponseErr {
    detail: Option<String>,
    status: StatusCode,
}

impl ResponseErr {
    pub fn new<T: Display>(detail: T, status_code: StatusCode) -> Self {
        Self {
            detail: detail.to_string().into(),
            status: status_code,
        }
    }

    pub fn status(status: StatusCode) -> Self {
        Self {
            detail: None,
            status,
        }
    }
}

impl std::fmt::Display for ResponseErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "STATUS {}, detail: {:?}", self.status, self.detail)
    }
}

impl From<ResponseErr> for Response<Full<Bytes>> {
    fn from(value: ResponseErr) -> Self {
        Response::builder()
            .status(value.status)
            .version(Version::HTTP_2)
            .body(match value.detail {
                Some(e) => Full::new(Bytes::from(e)),
                None => Full::default(),
            })
            .unwrap_or_default()
    }
}

impl From<RepositoryError> for ResponseErr {
    fn from(value: RepositoryError) -> Self {
        ResponseErr::new(value, StatusCode::BAD_REQUEST)
    }
}

impl From<EncryptErr> for ResponseErr {
    fn from(value: EncryptErr) -> Self {
        ResponseErr::new(value, StatusCode::INTERNAL_SERVER_ERROR)
    }
}
