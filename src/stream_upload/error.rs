use mime::Mime;

#[derive(Debug)]
pub enum StreamUploadError {
    MimeNotAllowed(Mime),
    UnexpectedEof,
    WriteZero,
    FineNameNotFound,
    MimeNotFound,
    Field(String),
    StorageFull,
    Io(std::io::Error),
}

impl From<multer::Error> for StreamUploadError {
    fn from(value: multer::Error) -> Self {
        Self::Field(value.to_string())
    }
}

impl From<std::io::Error> for StreamUploadError {
    fn from(value: std::io::Error) -> Self {
        match value.kind() {
            std::io::ErrorKind::WriteZero => Self::WriteZero,
            std::io::ErrorKind::StorageFull => Self::StorageFull,
            std::io::ErrorKind::UnexpectedEof => Self::UnexpectedEof,
            e => Self::Io(e.into()),
        }
    }
}

impl std::fmt::Display for StreamUploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamUploadError::MimeNotAllowed(mime) => {
                write!(f, "the mime {} is not allowed", mime)
            }
            StreamUploadError::UnexpectedEof => write!(f, "Unexpected EOF"),
            StreamUploadError::WriteZero => write!(f, "Write 0 bytes"),
            StreamUploadError::FineNameNotFound => write!(f, "File name not found"),
            StreamUploadError::MimeNotFound => write!(f, "Mime type is not present"),
            StreamUploadError::Field(e) => write!(f, "{e}"),
            StreamUploadError::StorageFull => write!(f, "Storage full"),
            StreamUploadError::Io(error) => write!(f, "{error}"),
        }
    }
}
