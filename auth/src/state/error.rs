

#[derive(Debug)]
pub enum RepositoryError {
    RowNotFound,
    ManyRows,
    ColumnNotFound(String),
    TypeNotFound(String),
    AlreadyExist(String),
    SqlxErr(sqlx::error::Error),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RowNotFound => write!(f, "Row not found"),
            Self::ManyRows => write!(f, "Many rows"),
            Self::TypeNotFound(ty) => write!(f, "Type {ty} not found"),
            Self::ColumnNotFound(e) => write!(f, "Column {e} not found"),
            Self::AlreadyExist(e) => write!(f, "{e}"),
            Self::SqlxErr(e) => write!(f, "{e}"),
        }
    }
}

impl From<sqlx::error::Error> for RepositoryError {
    fn from(value: sqlx::error::Error) -> Self {
        match value {
            sqlx::Error::RowNotFound => Self::RowNotFound,
            sqlx::Error::TypeNotFound { type_name } => Self::TypeNotFound(type_name),
            sqlx::Error::ColumnNotFound(e) => Self::ColumnNotFound(e),
            sqlx::Error::Database(err) if err.code().as_deref() == Some("23505") => {
                Self::AlreadyExist(err.message().to_string())
            }
            e => Self::SqlxErr(e),
        }
    }
}

impl std::error::Error for RepositoryError {}