use super::{queries::*, error::RepositoryError, Types};

use http_body_util::Full;
use hyper::{Response, StatusCode, body::Bytes, header};
use serde::Serialize;
use serde_json::json;
use sqlx::{
    pool::Pool,
    postgres::{PgPoolOptions, PgRow, Postgres},
};
use uuid::Uuid;

use crate::models::user::User;

pub const TABLA_BUCKET: &str = "buckets";
pub const TABLA_USER: &str = "users";

#[derive(Debug)]
pub struct PgRepository {
    inner: Pool<Postgres>,
}

impl PgRepository {
    pub async fn new(url: String) -> Result<Self, RepositoryError> {
        let repo = PgPoolOptions::new()
            .max_connections(10)
            .connect(&url)
            .await?;

        Ok(Self { inner: repo })
    }

    pub async fn with_default_user(url: String, user: User) -> Result<Self, RepositoryError> {
        let repo = Self::new(url).await?;
        if let Err(e) = repo.insert(InsertOwn::insert(user)).await {
            tracing::error!("{{ default user creation }} {e}");
        }

        Ok(repo)
    }

    pub async fn get<T>(&self, mut query: QueryOwn<'_, T>) -> Result<T, RepositoryError>
    where
        T: QuerySelect + From<PgRow>,
    {
        Ok(query.build().fetch_one(&self.inner).await?.into())
    }

    pub async fn gets<T>(&self, mut query: QueryOwn<'_, T>) -> Result<Vec<T>, RepositoryError>
    where
        T: QuerySelect + From<PgRow>,
    {
        Ok(query
            .build()
            .fetch_all(&self.inner)
            .await?
            .into_iter()
            .map(T::from)
            .collect::<Vec<T>>())
    }

    pub async fn delete(&self, id: Uuid) -> Result<QueryResult<User>, RepositoryError> {
        let query = format!("DELETE FROM {TABLA_USER} WHERE id = $1");
        let ex = sqlx::query(&query);
        let ex = bind!(ex, Types::Uuid(id));

        let ex = ex.execute(&self.inner).await?;

        Ok(QueryResult::Delete(ex.rows_affected()))
    }

    pub async fn insert<T>(
        &self,
        mut insert: InsertOwn<T>,
    ) -> Result<QueryResult<T>, RepositoryError>
    where
        T: Table,
    {
        let res = insert.query().execute(&self.inner).await?;

        Ok(QueryResult::Insert(res.rows_affected()))
    }

    pub async fn update<T>(
        &self,
        mut updater: UpdateOwn<'_, T>,
    ) -> Result<QueryResult<T>, RepositoryError>
    where
        T: Table,
    {
        Ok(QueryResult::Update(
            updater
                .query()
                .unwrap()
                .execute(&self.inner)
                .await?
                .rows_affected(),
        ))
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

pub trait Table {
    type ValuesOutput: IntoIterator<Item = Types>;
    fn name() -> &'static str;
    fn columns() -> &'static [&'static str];
    fn values(self) -> Self::ValuesOutput;
}

