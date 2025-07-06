use std::fmt::{Debug, Display};

use http_body_util::Full;
use hyper::{Response, StatusCode, Version, body::Bytes};

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
