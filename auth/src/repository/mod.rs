use std::{collections::HashMap, fmt::Write as _, marker::PhantomData};

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

pub const TABLA_PROGRAMA: &str = "programas";
pub const TABLA_USER: &str = "users";

macro_rules! bind {
    ($q:expr, $type:expr) => {
        match $type {
            Types::Uuid(uuid) => $q.bind(uuid),
            Types::String(string) => $q.bind(string),
            Types::OptString(vec) => $q.bind(vec),
            Types::UserState(state) => $q.bind(state),
            Types::Role(role) => $q.bind(role),
            Types::OptUuid(uuid) => $q.bind(uuid),
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
            .await?;

        Ok(Self { inner: repo })
    }

    pub async fn with_default_user(url: String, user: User) -> Result<Self, RepositoryError> {
        let repo = Self::new(url).await?;
        if let Err(e) = repo.insert_user(InsertOwn::insert(user)).await {
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

    pub async fn update<T>(
        &self,
        mut updater: UpdateOwn<'_, T>,
    ) -> Result<QueryResult<T>, RepositoryError>
    where
        T: for<'a> Table<'a>,
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

#[derive(Debug)]
pub enum Types {
    Uuid(Uuid),
    String(String),
    OptString(Option<String>),
    OptUuid(Option<Uuid>),
    UserState(UserState),
    Role(Role),
}

impl From<Option<Uuid>> for Types {
    fn from(value: Option<Uuid>) -> Self {
        Self::OptUuid(value)
    }
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

pub trait QuerySelect {
    fn query() -> String;
}

impl<T> QuerySelect for T
where
    T: for<'a> Table<'a>,
{
    fn query() -> String {
        format!("SELECT * FROM {}", T::name())
    }
}

pub struct QueryOwn<'a, T> {
    wh: Option<Where<'a>>,
    limit: Option<u32>,
    _priv: PhantomData<T>,
    query: String,
}

impl<'a, T> QueryOwn<'a, T>
where
    T: QuerySelect,
{
    pub fn builder() -> Self {
        Self {
            wh: None,
            limit: None,
            _priv: PhantomData,
            query: String::new(),
        }
    }
    pub fn wh<U>(mut self, index: &'a str, value: U) -> Self
    where
        U: Into<Types>,
    {
        if self.wh.is_none() {
            self.wh = Some(HashMap::from([(index, value.into())]));
        } else {
            self.wh.as_mut().unwrap().insert(index, value.into());
        }

        self
    }

    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);

        self
    }

    pub fn build(&'a mut self) -> Query<'a, Postgres, PgArguments> {
        self.query = T::query();

        if let Some(wheres) = self.wh.take() {
            let mut aux = Vec::new();
            let mut first = true;
            let mut n = 1;
            for (key, value) in wheres {
                if first {
                    first = false;
                    self.query.push_str(" WHERE");
                } else {
                    self.query.push_str(" AND");
                }

                _ = write!(self.query, " {key} = ${n}");
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

pub struct UpdateOwn<'a, T> {
    pub query: String,
    pub wh: Where<'a>,
    pub items: Vec<Types>,
    _priv: PhantomData<T>,
}

impl<'a, T> UpdateOwn<'a, T>
where
    T: for<'t> Table<'t>,
{
    pub fn new() -> Self {
        Self {
            query: String::new(),
            wh: HashMap::new(),
            items: Vec::new(),
            _priv: PhantomData,
        }
    }

    pub fn from<U>(mut self, items: U) -> Self
    where
        U: Into<HashMap<&'static str, Types>>,
    {
        self.query = format!("UPDATE {} SET", T::name());
        let mut count = 1;
        for (k, v) in <U as Into<HashMap<&'static str, Types>>>::into(items) {
            self.items.push(v);
            _ = write!(
                self.query,
                "{} {k} = ${count}",
                if count > 1 { "," } else { "" }
            );

            count += 1;
        }
        self
    }

    pub fn wh<U>(mut self, index: &'a str, value: U) -> Self
    where
        U: Into<Types>,
    {
        self.wh.insert(index, value.into());
        self
    }

    pub fn query(&mut self) -> Result<Query<'_, Postgres, PgArguments>, UpdateOwnErr> {
        if self.items.is_empty() {
            return Err(UpdateOwnErr);
        }
        let len = self.items.len();
        let count = len + 1;
        for (k, v) in std::mem::take(&mut self.wh) {
            _ = write!(
                self.query,
                "{} {k} = ${count}",
                if count == len + 1 { " WHERE" } else { "," }
            );

            self.items.push(v);
        }

        Ok(std::mem::take(&mut self.items)
            .into_iter()
            .fold(sqlx::query(&self.query), |x, y| bind!(x, y)))
    }
}

#[derive(Debug)]
pub struct UpdateOwnErr;

impl std::fmt::Display for UpdateOwnErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Values not defined")
    }
}

impl std::error::Error for UpdateOwnErr {}
