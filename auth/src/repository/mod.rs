use std::{collections::HashMap, marker::PhantomData};

use http_body_util::Full;
use hyper::{Response, StatusCode, body::Bytes, header};
use serde::Serialize;
use serde_json::json;
use sqlx::{
    pool::Pool,
    postgres::{PgArguments, PgPoolOptions, PgRow, Postgres},
    query::Query,
};
use uuid::Uuid;

use crate::models::user::{Role, User, UserState};

pub const TABLA_PROGRAMA: &str = "programa";
pub const TABLA_USER: &str = "users";

macro_rules! bind {
    ($q:expr, $type:expr) => {
        match $type {
            Types::Uuid(uuid) => $q.bind(uuid),
            Types::String(string) => $q.bind(string),
            Types::OptString(vec) => $q.bind(vec),
            Types::UserState(state) => $q.bind(state),
            Types::Role(role) => $q.bind(role),
        }
    };
}

#[derive(Debug)]
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

    pub async fn with_default_user(url: String, user: User) -> Result<Self, RepositoryError> {
        let repo = Self::new(url).await?;
        _ = repo.insert_user(InsertOwn::insert(user)).await;

        Ok(repo)
    }

    pub async fn get<T>(&self, mut query: QueryOwn<'_, T>) -> Result<T, RepositoryError>
    where
        T: for<'b> Table<'b> + From<PgRow>,
    {
        Ok(query.build().fetch_one(&self.inner).await?.into())
    }

    pub async fn gets<T>(&self, mut query: QueryOwn<'_, T>) -> Result<Vec<T>, RepositoryError>
    where
        T: for<'b> Table<'b> + From<PgRow>,
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

    pub async fn insert_user<T>(
        &self,
        mut insert: InsertOwn<T>,
    ) -> Result<QueryResult<T>, RepositoryError>
    where
        T: for<'b> Table<'b>,
    {
        let _res = insert.query().execute(&self.inner).await?;

        Ok(QueryResult::Insert(1))
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
    ManyRows,
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "Row not found"),
            Self::ManyRows => write!(f, "Many rows"),
        }
    }
}

impl From<sqlx::error::Error> for RepositoryError {
    fn from(_value: sqlx::error::Error) -> Self {
        Self::NotFound
    }
}

impl std::error::Error for RepositoryError {}

pub enum Types {
    Uuid(Uuid),
    String(String),
    OptString(Option<String>),
    UserState(UserState),
    Role(Role),
}

impl From<Uuid> for Types {
    fn from(value: Uuid) -> Self {
        Self::Uuid(value)
    }
}

impl From<String> for Types {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<Option<String>> for Types {
    fn from(value: Option<String>) -> Self {
        Self::OptString(value)
    }
}

impl From<UserState> for Types {
    fn from(value: UserState) -> Self {
        Self::UserState(value)
    }
}

impl From<Role> for Types {
    fn from(value: Role) -> Self {
        Self::Role(value)
    }
}

type Where<'a> = HashMap<&'a str, Types>;

pub trait Table<'a> {
    fn name() -> &'a str;
    fn columns() -> Vec<&'a str>;
    fn values(self) -> Vec<Types>;
}

pub struct QueryOwn<'a, T> {
    wh: Option<Where<'a>>,
    limit: Option<u32>,
    _priv: PhantomData<T>,
    query: String,
}

impl<'a, T> QueryOwn<'a, T>
where
    T: Table<'a>,
{
    pub fn builder() -> Self {
        Self {
            wh: None,
            limit: None,
            _priv: PhantomData,
            query: String::new(),
        }
    }
    pub fn wh(mut self, index: &'a str, value: Types) -> Self {
        if self.wh.is_none() {
            self.wh = Some(HashMap::from([(index, value)]));
        } else {
            self.wh.as_mut().unwrap().insert(index, value);
        }

        self
    }

    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);

        self
    }

    pub fn build(&'a mut self) -> Query<'a, Postgres, PgArguments> {
        self.query = format!("SELECT * FROM {}", T::name());

        if let Some(wheres) = self.wh.take() {
            let mut aux = Vec::new();
            let mut first = true;
            let mut n = 1;
            for (key, value) in wheres {
                if !first {
                    self.query.push_str(" AND");
                } else {
                    first = false;
                    self.query.push_str(" WHERE");
                }

                self.query.push_str(&format!(" {key} = ${n}"));
                aux.push(value);
                n += 1;
            }

            let mut query = sqlx::query(&self.query);
            for t in aux {
                query = bind!(query, t);
            }
            query
        } else {
            sqlx::query(&self.query)
        }
    }
}

pub struct InsertOwn<T> {
    query: String,
    len: u32,
    item: Option<T>,
}

pub trait Insert<T> {
    fn insert(item: T) -> Self;
    fn query(&mut self) -> Query<'_, Postgres, PgArguments>;
}

impl<T> Insert<T> for InsertOwn<T>
where
    T: for<'a> Table<'a>,
{
    fn insert(item: T) -> Self {
        let columns = T::columns();
        Self {
            query: format!(
                "INSERT INTO {} ({}) VALUES ({})",
                T::name(),
                columns.join(","),
                (1..=columns.len())
                    .map(|x| format!("${x}"))
                    .collect::<Vec<String>>()
                    .join(",")
            ),
            len: 1,
            item: Some(item),
        }
    }

    fn query(&mut self) -> Query<'_, Postgres, PgArguments> {
        let item = self.item.take();

        item.unwrap()
            .values()
            .into_iter()
            .fold(sqlx::query(&self.query), |acc, item| bind!(acc, item))
    }
}

impl<T> Insert<Vec<T>> for InsertOwn<Vec<T>> {
    fn insert(_item: Vec<T>) -> Self {
        todo!()
    }

    fn query(&mut self) -> Query<'_, Postgres, PgArguments> {
        todo!()
    }
}
