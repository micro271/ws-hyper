use mime::Mime;

#[derive(Debug, Clone)]
pub enum UploadError {
    MimeNotAllowed { file: String, mime: Mime },
    UnexpectedEof,
    WriteZero,
    FileNameNotFound,
    MimeNotFound(String),
    Multer(String),
    StorageFull,
    Io(String),
}

impl From<multer::Error> for UploadError {
    fn from(value: multer::Error) -> Self {
        Self::Multer(value.to_string())
    }
}

impl From<std::io::Error> for UploadError {
    fn from(value: std::io::Error) -> Self {
        match value.kind() {
            std::io::ErrorKind::WriteZero => Self::WriteZero,
            std::io::ErrorKind::StorageFull => Self::StorageFull,
            std::io::ErrorKind::UnexpectedEof => Self::UnexpectedEof,
            e => Self::Io(e.to_string()),
        }
    }
}

impl std::fmt::Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UploadError::UnexpectedEof => write!(f, "Unexpected EOF"),
            UploadError::WriteZero => write!(f, "Write 0 bytes"),
            UploadError::FileNameNotFound => write!(f, "File name not found"),
            UploadError::MimeNotFound(file) => {
                write!(f, "File: {file} {{Mime type is not present}}")
            }
            UploadError::MimeNotAllowed { file, mime } => {
                write!(f, "file: {file} {{ Mime {mime} is not allowed }}")
            }
            UploadError::Multer(str) => write!(f, "Multer error: {str}"),
            UploadError::StorageFull => write!(f, "Storage full"),
            UploadError::Io(str) => write!(f, "{str}"),
        }
    }
}

impl std::error::Error for UploadError {}
