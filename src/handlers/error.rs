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

    pub fn parse_error(err: ParseError) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            detail: err.to_string(),
        }
    }

    pub fn unimplemented() -> Self {
        Self {
            status: StatusCode::NOT_IMPLEMENTED,
            detail: "This function is not implemented yet".to_string(),
        }
    }
}

#[derive(Debug)]
pub enum ParseError {
    Path,
    Param,
    Json,
    Query,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Path => write!(f, "Error parsing from entry valur to Path"),
            ParseError::Param => write!(f, "Error parsing from entry valur to Param"),
            ParseError::Json => write!(f, "Error parsing from entry valur to Json"),
            ParseError::Query => write!(f, "Error parsing from entry valur to Query"),
        }
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
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error".to_string(),
                )
            }
            e @ DbError::ColumnNotFound(_) => {
                let err = e.to_string();
                tracing::error!("{}", err);
                (StatusCode::BAD_REQUEST, err)
            }
            e @ DbError::RowNotFound => {
                let err = e.to_string();
                tracing::error!("{}", err);
                (StatusCode::BAD_REQUEST, err)
            }
        };

        Self { status, detail }
    }
}

impl From<ResponseError> for Response<Full<Bytes>> {
    fn from(value: ResponseError) -> Self {
        let body = Full::new(Bytes::from(
            serde_json::json!({
                "detail": value.detail
            })
            .to_string(),
        ));

        Response::builder()
            .status(value.status)
            .body(body)
            .unwrap_or_default()
    }
}

impl std::error::Error for ResponseError {}
