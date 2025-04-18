use sqlx::{postgres::{PgPool, PgPoolOptions, PgRow}, FromRow, Type};
use time::OffsetDateTime;
use uuid::Uuid;


pub struct Repository {
    inner: PgPool,
}

impl Repository {
    pub async fn new(url: &str) -> Result<Self, DbError> {
        Ok(Self { inner: PgPoolOptions::new().max_connections(5).connect(&url).await?, })
    }

    pub async fn insert<'a, T>(&self, new: T) 
        where
            T: FromRow<'a, PgRow> + Table,
    {
        
    }
}

#[derive(Debug)]
pub enum DbError {
    Sqlx(String),
    ColumnNotFound(String),
    RowNotFound,
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Sqlx(e) => write!(f, "sqlx error: {e}"),
            DbError::ColumnNotFound(e) => write!(f, "Column {e} not found"),
            DbError::RowNotFound => write!(f, "Row not found"),
        }
    }
}

impl std::error::Error for DbError {}

impl From<sqlx::Error> for DbError {
    fn from(value: sqlx::Error) -> Self {
        match value {
            sqlx::Error::ColumnNotFound(e) => Self::ColumnNotFound(e),
            sqlx::Error::RowNotFound => Self::RowNotFound,
            e => Self::Sqlx(e.to_string())
        }
    }
}

pub trait Table {
    fn name() -> &'static str;

    fn query_select(&self) -> String {
        format!("SELECT * FROM {}", Self::name())
    }

    fn query_update(&self) -> String {
        format!("UPDATE {}", Self::name())
    }
    fn query_insert() -> String {

        let columns = Self::columns_name();
        let len = columns.len();

        format!("INSERT INTO {} ({}) VALUES ({})",
            Self::name(),
            columns.len(),
            (1..=len)
                .into_iter()
                .map(|x| format!("${}",x))
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn columnds_value(self) -> Vec<Types>;
    fn columns_name() -> Vec<&'static str>;
}
pub enum Types {
    I32(i32),
    String(String),
    UtcWithOffset(time::OffsetDateTime),
    OptionUsize(Option<usize>),
    Uuid(Uuid),
}

impl From<i32> for Types {
    fn from(value: i32) -> Self {
        Self::I32(value)
    }
}

impl From<String> for Types {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<OffsetDateTime> for Types {
    fn from(value: OffsetDateTime) -> Self {
        Self::UtcWithOffset(value)
    }
}

impl From<Option<usize>> for Types {
    fn from(value: Option<usize>) -> Self {
        Self::OptionUsize(value)
    }
}

impl From<Uuid> for Types {
    fn from(value: Uuid) -> Self {
        Self::Uuid(value)
    }
}