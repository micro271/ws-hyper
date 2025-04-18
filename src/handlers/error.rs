use bytes::Bytes;
use http::{Response, StatusCode};
use http_body_util::Full;

use crate::repository::DbError;

#[derive(Debug, Clone)]
pub struct ResponseError {
    pub status: StatusCode,
    pub detail: String,
}

impl ResponseError {
    pub fn new(status: StatusCode, detail: String) -> Self {
        Self { status, detail }
    }
}

impl std::fmt::Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Statud: {} - {}", self.status, self.detail)
    }
}


impl From<DbError> for ResponseError {
    fn from(value: DbError) -> Self {
        let (status, detail) = match value {
            DbError::Sqlx(e) => {
                tracing::error!("{e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string())
            },
            e@ DbError::ColumnNotFound(_) => {
                let err = e.to_string();
                tracing::error!("{}", err);
                (StatusCode::BAD_REQUEST, err)
            },
            e @ DbError::RowNotFound => {
                let err = e.to_string();
                tracing::error!("{}", err);
                (StatusCode::BAD_REQUEST, err)
            },
        };

        Self {
            status,
            detail
        }
    }
}

impl From<ResponseError> for Response<Full<Bytes>> {
    fn from(value: ResponseError) -> Self {
        let body = Full::new(Bytes::from(
            serde_json::json!({
                "detail": value.detail
            }).to_string()));

        Response::builder()
            .status(value.status)
            .body(body)
            .unwrap_or_default()
    }
}

impl std::error::Error for ResponseError {}