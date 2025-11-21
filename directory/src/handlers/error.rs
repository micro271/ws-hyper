use http::StatusCode;
use http_body_util::Full;
use hyper::{Response, body::Bytes};

pub struct ResponseError {
    detail: Option<String>,
    status: StatusCode,
}

impl ResponseError {
    pub fn new(detail: String, status: StatusCode) -> Self {
        Self {
            detail: Some(detail),
            status,
        }
    }

    pub fn status(status: StatusCode) -> Self {
        Self {
            detail: None,
            status,
        }
    }
}

impl From<ResponseError> for Response<Full<Bytes>> {
    fn from(value: ResponseError) -> Self {
        Response::builder()
            .status(value.status)
            .body(
                value
                    .detail
                    .map(|x| Full::new(Bytes::from(x)))
                    .unwrap_or_default(),
            )
            .unwrap_or_default()
    }
}
