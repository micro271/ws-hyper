use sqlx::{
    pool::Pool,
    postgres::{PgPoolOptions, Postgres},
};

use crate::models::user::{User, UserState, Verbs};

pub struct PgRepository {
    inner: Pool<Postgres>,
}

impl PgRepository {
    pub async fn new(url: String) -> Result<Self, RepositoryError> {
        let repo = PgPoolOptions::new()
            .max_connections(10)
            .connect(&url)
            .await
            .map_err(|_| RepositoryError::NotFound)?;

        Ok(Self { inner: repo })
    }

    pub async fn with_default_user(url: String) -> Result<Self, RepositoryError> {
        let repo = Self::new(url).await?;
        let user = User {
            user_state: UserState::Active,
            id: None,
            username: "admin".to_string(),
            passwd: "admin".to_string(),
            email: "email-falso@domain-dalse.com".to_string(),
            verbos: vec![Verbs::All],
            phone: "545464654".to_string(),
            role: crate::models::user::Role::Administrator,
            resources: "*".to_string(),
        };
        _ = repo.insert_one_user(user).await;
        Ok(repo)
    }

    pub async fn get_users(&self, username: &str) -> Result<Vec<User>, RepositoryError> {
        let tmp = sqlx::query("SELECT * FROM users")
            .bind(username)
            .fetch_all(&self.inner)
            .await
            .map_err(|_| RepositoryError::NotFound)?;
        Ok(tmp.into_iter().map(User::from).collect::<Vec<User>>())
    }

    pub async fn get_user(&self, username: &str) -> Result<User, RepositoryError> {
        let tmp = sqlx::query("SELECT * FROM users WHERE username = $1")
            .bind(username)
            .fetch_one(&self.inner)
            .await
            .map_err(|_| RepositoryError::NotFound)?;
        Ok(tmp.into())
    }

    pub async fn insert_one_user(&self, user: User) -> Result<QueryResult<User>, RepositoryError> {
        let query = format!(
            "INSERT INTO {} (username, passwd, email, verbos, user_state, phone, role, resources) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            user.name()
        );
        let res = sqlx::query(&query)
            .bind(user.username)
            .bind(user.passwd)
            .bind(user.email)
            .bind(user.verbos)
            .bind(user.user_state)
            .bind(user.phone)
            .bind(user.role)
            .bind(user.resources)
            .execute(&self.inner)
            .await
            .map_err(|x| RepositoryError::NotFound)?;
        Ok(QueryResult::Insert(res.rows_affected()))
    }
}

impl std::ops::Deref for PgRepository {
    type Target = Pool<Postgres>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub enum QueryResult<T> {
    SelectOne(T),
    Select(Vec<T>),
    Insert(u64),
    Delete(u64),
    Update(u64),
}

#[derive(Debug)]
pub enum RepositoryError {
    NotFound,
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "Row not found"),
        }
    }
}

impl std::error::Error for RepositoryError {}

pub trait TableName {
    fn name(&self) -> &str;
}
