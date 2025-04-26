use crate::repository::DbError;
use bytes::Bytes;
use http::{Response, StatusCode, header};
use http_body_util::Full;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct ResponseError {
    status: StatusCode,
    body: Full<Bytes>,
}

impl ResponseError {
    pub fn new<T>(status: StatusCode, detail: Option<T>) -> Self
    where
        T: AsRef<str>,
    {
        Self {
            status,
            body: Full::new(match detail {
                Some(e) => Bytes::from(json!({"detail": e.as_ref()}).to_string()),
                None => Bytes::new(),
            }),
        }
    }

    pub fn parse_error(err: ParseError) -> Self {
        Self::new(StatusCode::BAD_REQUEST, err.into())
    }

    pub fn unimplemented() -> Self {
        Self::new(
            StatusCode::NOT_IMPLEMENTED,
            "This function is not implemented yet".into(),
        )
    }
}

#[derive(Debug)]
pub enum ParseError {
    Path,
    Param,
    Json,
    Query,
}

impl AsRef<str> for ParseError {
    fn as_ref(&self) -> &str {
        match self {
            ParseError::Path => "Path",
            ParseError::Param => "Param",
            ParseError::Json => "Json",
            ParseError::Query => "Query",
        }
    }
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
        write!(f, "Status: {} - {:?}", self.status, self.body)
    }
}

impl From<DbError> for ResponseError {
    fn from(value: DbError) -> Self {
        let (status, detail) = match value {
            DbError::MongoDb(e) => {
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

        Self::new(status, detail.into())
    }
}

impl From<ResponseError> for Response<Full<Bytes>> {
    fn from(value: ResponseError) -> Self {
        Response::builder()
            .status(value.status)
            .header(header::CONTENT_TYPE, "application/json")
            .body(value.body)
            .unwrap_or_default()
    }
}

impl std::error::Error for ResponseError {}
