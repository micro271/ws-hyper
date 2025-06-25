use crate::{repository::RepositoryError, stream_upload::error::UploadError};
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
    pub fn status(&self) -> StatusCode {
        self.status
    }

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
    pub fn detail(&self) -> &Full<Bytes> {
        &self.body
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

impl From<RepositoryError> for ResponseError {
    fn from(value: RepositoryError) -> Self {
        let mut flag_not_detail = true;
        let status = match value {
            RepositoryError::MongoDb(_) | RepositoryError::DatabaseDefault => {
                flag_not_detail = false;
                StatusCode::INTERNAL_SERVER_ERROR
            }
            _ => StatusCode::BAD_REQUEST,
        };

        Self::new(status, (flag_not_detail).then_some(value.to_string()))
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

impl From<UploadError> for ResponseError {
    fn from(value: UploadError) -> Self {
        let (status, detail) = match value {
            UploadError::MimeNotAllowed { file, mime } => (
                StatusCode::BAD_REQUEST,
                Some(format!("The file {file} have an unallower mime ({mime})")),
            ),
            UploadError::FileNameNotFound => todo!(),
            UploadError::MimeNotFound { file } => (
                StatusCode::BAD_REQUEST,
                Some(format!("The file {file} dont have an mime type")),
            ),
            UploadError::Multer(_) => (StatusCode::INTERNAL_SERVER_ERROR, None),
            UploadError::StorageFull => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Some("You file is very large, i dont have anough space".to_string()),
            ),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, None),
        };

        ResponseError::new(status, detail)
    }
}
