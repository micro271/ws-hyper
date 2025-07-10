use std::fmt::Display;

use bcrypt::{DEFAULT_COST, hash};
use http_body_util::Full;
use hyper::{Response, StatusCode, body::Bytes, header};
use serde::Serialize;
use serde_json::json;
use sqlx::{
    Encode, Type,
    pool::Pool,
    postgres::{PgPoolOptions, PgRow, Postgres},
};

use crate::models::user::{User, UserState, Verbs};

macro_rules! get {
    (one, $pool:expr, $t:ty $(, where = [$($cond:expr),+])? $(,)?) => {
        async {
            let mut query = format!("SELECT * FROM {}", <$t>::name());
            $(
                let wheres = vec![$($cond),+].join(" AND ");
                query.push_str(&format!(" WHERE {}", wheres));
            )?

            sqlx::query(&query).fetch_one($pool).await.unwrap()
        }
    };

    (many, $pool:expr, $t:ty $(, where = [$($cond:expr),+])?) => {
        async {
            let query = format!("SELECT * FROM {}", <$t>::name());

            $(
                flag_vec = true;
                let wheres = vec![$($cond),+].join(" AND ");
                query.push_str(&format!(" WHERE {}", wheres));
            )?

            let resp = sqlx::query(&query).fetch_all($pool).await.unwrap();
            resp.into_iter().map(|x| <$t>::from(x)).collect::<Vec<$t>>()
        }
    };

    ($pool:expr, $t:ty, from => $from:ty $(, inner_join => $join1:ty : $key_join1:literal, $join2:ty : $key_join2:literal)+ $(, _where => [ $($cond:expr),+ ])?) => {
        async {
            let mut query = format!("SELECT * FROM {}", <$from>::name());
            $(
                let key1 = format!("{}.{}", <$join1>::name(), $key_join1);
                let key2 = format!("{}.{}", <$join2>::name(), $key_join2);
                query.push_str(&format!(" INNET JOIN {} ON {} = {}", <$join2>::name(), key1, key2));
            )*

            $(
                let wheres = vec![$($cond),+].join(" AND ");
                query.push_str(&format!(" WHERE {}", wheres));
            )?

            let resp = sqlx::query(&query).fetch_all($pool).await.unwrap();
            resp.into_iter().map(|x| <$t>::from(x)).collect::<Vec<$t>>()
        }
    };
}

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
            passwd: hash("admin", DEFAULT_COST).unwrap(),
            email: None,
            verbos: vec![Verbs::All],
            phone: None,
            role: crate::models::user::Role::Administrator,
            resources: Some("/*".to_string()),
        };

        _ = repo.insert_user(user).await;
        Ok(repo)
    }

    pub async fn get_users(&self) -> Result<Vec<User>, RepositoryError> {
        let get = get!(many, &self.inner, User).await;
        Ok(get)
    }

    pub async fn get_user<T, Pk>(&self, pk: (&str, Pk)) -> Result<QueryResult<T>, RepositoryError>
    where
        Pk: for<'q> Encode<'q, Postgres> + Type<Postgres> + Display,
        T: TableName + From<PgRow>,
    {
        let tmp = QueryResult::SelectOne(
            get!(one, &self.inner, T, where = [ format!("{} = {}", pk.0, pk.1) ])
                .await
                .into(),
        );
        Ok(tmp)
    }

    pub async fn delete<T, Pk>(&self, pk: Pk) -> Result<QueryResult<T>, RepositoryError>
    where
        T: TableName,
        Pk: for<'q> sqlx::Encode<'q, Postgres> + Type<Postgres>,
    {
        let query = format!("DELETE FROM {} WHERE id = $1", T::name());
        let tmp = sqlx::query(&query).bind(pk).execute(&self.inner).await?;

        Ok(QueryResult::Delete(tmp.rows_affected()))
    }

    pub async fn insert_user(&self, user: User) -> Result<QueryResult<User>, RepositoryError> {
        let query = format!(
            "INSERT INTO {} (username, passwd, email, verbos, user_state, phone, role, resources) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            User::name()
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
            .map_err(|_x| RepositoryError::NotFound)?;
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

impl<T: Serialize> From<QueryResult<T>> for Response<Full<Bytes>> {
    fn from(value: QueryResult<T>) -> Self {
        match value {
            QueryResult::SelectOne(item) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Full::new(Bytes::from(json!({"data": item}).to_string())))
                .unwrap_or_default(),
            QueryResult::Select(items) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Full::new(Bytes::from(
                    json!({"length": items.len(), "data": items}).to_string(),
                )))
                .unwrap_or_default(),
            QueryResult::Insert(n) | QueryResult::Delete(n) | QueryResult::Update(n) => {
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Full::new(Bytes::from(json!({"row_affect": n}).to_string())))
                    .unwrap_or_default()
            }
        }
    }
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

impl From<sqlx::error::Error> for RepositoryError {
    fn from(value: sqlx::error::Error) -> Self {
        Self::NotFound
    }
}

impl std::error::Error for RepositoryError {}

pub trait TableName {
    fn name() -> &'static str;
}
