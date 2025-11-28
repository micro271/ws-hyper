use mongodb::error::{Error, WriteFailure};

#[derive(Debug)]
pub enum LsError {
    DuplicateKey,
    MongoDb(Box<dyn std::error::Error>),
}

impl From<Error> for LsError {
    fn from(value: Error) -> Self {
        match *value.kind {
            mongodb::error::ErrorKind::Write(WriteFailure::WriteError(er)) if er.code == 1100 => {
                Self::DuplicateKey
            }
            e => Self::MongoDb(Box::new(e)),
        }
    }
}

impl std::fmt::Display for LsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LsError::DuplicateKey => write!(f, "Duplicate Key"),
            LsError::MongoDb(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for LsError {}