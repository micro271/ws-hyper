use mongodb::{
    Client,
    options::{ClientOptions, Credential},
};
use time::OffsetDateTime;
use uuid::Uuid;

pub struct Repository {
    inner: Client,
}

impl Repository {
    pub async fn new(url: String, user: String, pass: String, db: String) -> Result<Self, DbError> {
        let cred = Credential::builder().username(user).password(pass).build();
        let mut opt = ClientOptions::parse(url).await?;
        opt.max_connecting = Some(5);
        opt.default_database = Some(db);
        opt.credential = Some(cred);

        Ok(Self {
            inner: Client::with_options(opt).unwrap(),
        })
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

impl From<mongodb::error::Error> for DbError {
    fn from(value: mongodb::error::Error) -> Self {
        Self::Sqlx(value.to_string())
    }
}
