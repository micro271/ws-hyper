use http::StatusCode;
use http_body_util::Full;
use hyper::{Response, body::Bytes};

pub struct ResponseError {
    detail: String,
    status: StatusCode,
}

impl ResponseError {
    pub fn new(detail: String, status: StatusCode) -> Self {
        Self { detail, status }
    }
}

impl From<ResponseError> for Response<Full<Bytes>> {
    fn from(value: ResponseError) -> Self {
        Response::builder()
            .status(value.status)
            .body(Full::new(Bytes::copy_from_slice(value.detail.as_ref())))
            .unwrap_or_default()
    }
}
