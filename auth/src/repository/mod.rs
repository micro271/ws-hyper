use http_body_util::Full;
use hyper::{Response, StatusCode, body::Bytes, header};
use serde::Serialize;
use serde_json::json;
use sqlx::{
    pool::Pool,
    postgres::{PgPoolOptions, Postgres},
};
use uuid::Uuid;

use crate::models::{
    GetUserOwn, GetUserPubAdm,
    program::Programa,
    user::{Role, User, UserState, Verbs},
};

macro_rules! bind {
    ($q:expr, $type:expr) => {
        match $type {
            Types::Uuid(uuid) => $q.bind(uuid),
            Types::String(string) => $q.bind(string),
            Types::OptString(vec) => $q.bind(vec),
            Types::Verbs(vec) => $q.bind(vec),
            Types::UserState(state) => $q.bind(state),
            Types::Role(role) => $q.bind(role),
        }
    };
}

macro_rules! get_one {
    ($pool:expr, $t:ty $(, where = [$($key:expr, $value:expr);+])? $(,)?) => {
        async {
            let mut query = format!("SELECT * FROM {}", <$t>::name());

            let mut list: Vec<Types> = Vec::new();
            $(
                let mut count = 0;
                let mut wheres = vec![];
                $(
                    count += 1;
                    wheres.push(format!(" {} = ${} ", $key, count));
                    list.push($value);
                )+
                query.push_str(&format!(" WHERE {}", wheres.join(" AND ")));
            )?

            let acc = list.into_iter().fold(sqlx::query(&query), |acc, value | bind!(acc, value));

            acc.fetch_one($pool).await.map(<$t>::from)
        }
    };

    ($pool:expr, $t:ty, from => $from:ty $(, left_join => $join1:ty : $key_join1:literal, $join2:ty : $key_join2:literal)+, _where => [ $($key:expr, $value:expr);+ ]) => {
        async {
            let resp = get_many($poo, $ty, from => $from, $(left_join => $join: $ket_join1, $join2: $key_join2)+, _where => [$($key:expr, $value:expr);+]).await;
            resp.filter(|x| x.len() == 1).map(|x| x.pop().unwrap())
        }
    };
}

macro_rules! get_many {
    ($pool:expr, $t:ty $(, where = [$($key:expr, $value:expr);+])?) => {
        async {
            let query = format!("SELECT * FROM {}", <$t>::name());

            #[allow(unused_mut)] let mut types: Vec<Types> = vec![];
            $(
                let count = 0;
                let wheres = vec![];
                $(
                    count += 1;
                    wheres.push(format!("{} = ${}", $key, count));
                    types.push($value)
                )*
                query.push_str(&format!(" WHERE {}", wheres.join(" AND ")));
            )?

            let resp = types.into_iter().fold(sqlx::query(&query), |acc, value| bind!(acc, value));

            resp.fetch_all($pool).await.map(|x| x.into_iter().map(|x| <$t>::from(x)).collect::<Vec<$t>>() )
        }
    };

    ($pool:expr, $t:ty, from => $from:ty $(, left_join => $join1:ty : $key_join1:literal, $join2:ty : $key_join2:literal)+ $(, _where => [ $($key:expr, $value:expr);+ ])?) => {
        async {

            let mut inners = String::new();
            let selects = vec![$(<$join1 as InnerJoin<$join2>>::fields())+].join(",");

            $(
                let key1 = format!("{}.{}",<$join1>::name(), $key_join1);
                let key2 = format!("{}.{}",<$join2>::name(), $key_join2);
                inners.push_str(&format!(" LEFT JOIN {} ON {} = {}", <$join2>::name(), key1, key2));
            )+


            #[allow(unused_mut)] let mut query = format!("SELECT {} FROM {}", selects, <$from>::name());
            #[allow(unused_mut)] let mut values: Vec<Types> = Vec::new();

            $(
                let mut wheres = String::new();
                let mut count = 0;
                $(
                    count += 1;
                    wheres.push_str(&format!("{} = ${}", $key, count));
                    values.push($value);
                )+

                query.push_str(&format!(" WHERE {}", wheres));

            )?

            let resp = values.into_iter().fold(sqlx::query(&query),|acc, value| bind!(acc, value));

            resp.fetch_all($pool).await.map(|x| x.into_iter().map(|x| <$t>::from(x)).collect::<Vec<$t>>())
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

    pub async fn with_default_user(url: String, user: User) -> Result<Self, RepositoryError> {
        let repo = Self::new(url).await?;
        _ = repo.insert_user(user).await;

        Ok(repo)
    }

    pub async fn get_user(
        &self,
        key: &str,
        value: Types,
    ) -> Result<QueryResult<User>, RepositoryError> {
        Ok(QueryResult::SelectOne(
            get_one!(&self.inner, User, where = [ key, value ]).await?,
        ))
    }

    pub async fn get_users_pub(&self) -> Result<QueryResult<GetUserOwn>, RepositoryError> {
        let mut user = get_many!(&self.inner, GetUserOwn, from => User, left_join => User: "id", Programa: "user_id" ).await?;

        if user.len() > 1 {
            return Err(RepositoryError::ManyRows);
        }

        Ok(QueryResult::SelectOne(
            user.pop().ok_or(RepositoryError::NotFound)?,
        ))
    }

    pub async fn get_users_pub_extend(
        &self,
    ) -> Result<QueryResult<GetUserPubAdm>, RepositoryError> {
        let mut user = get_many!(&self.inner, GetUserPubAdm, from => User, left_join => User: "id", Programa: "user_id" ).await?;

        if user.len() > 1 {
            return Err(RepositoryError::ManyRows);
        }

        Ok(QueryResult::SelectOne(
            user.pop().ok_or(RepositoryError::NotFound)?,
        ))
    }

    pub async fn get_user_pub(&self) -> Result<QueryResult<GetUserOwn>, RepositoryError> {
        let mut user = get_many!(&self.inner, GetUserOwn, from => User, left_join => User: "id", Programa: "user_id" ).await?;

        if user.len() > 1 {
            return Err(RepositoryError::ManyRows);
        }

        Ok(QueryResult::SelectOne(
            user.pop().ok_or(RepositoryError::NotFound)?,
        ))
    }

    pub async fn delete<T>(
        &self,
        key: &str,
        value: Types,
    ) -> Result<QueryResult<T>, RepositoryError>
    where
        T: TableName,
    {
        let query = format!("DELETE FROM {} WHERE {} = $1", T::name(), key);
        let ex = sqlx::query(&query);
        let ex = bind!(ex, value);

        let ex = ex.execute(&self.inner).await?;

        Ok(QueryResult::Delete(ex.rows_affected()))
    }

    pub async fn insert_user<T>(&self, user: T) -> Result<QueryResult<T>, RepositoryError>
    where
        T: TableName + InsertPg,
    {
        let cols = T::get_fields_name();
        let len = cols.len();
        let query = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            User::name(),
            cols.join(", "),
            (1..=len)
                .map(|x| format!("${x}"))
                .collect::<Vec<String>>()
                .join(", ")
        );
        let res = T::get_fields(user)
            .into_iter()
            .fold(sqlx::query(&query), |acc, value| bind!(acc, value))
            .execute(&self.inner)
            .await?;

        Ok(QueryResult::Insert(res.rows_affected()))
    }
}

impl std::ops::Deref for PgRepository {
    type Target = Pool<Postgres>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub trait InsertPg: Sized {
    fn get_fields(self) -> Vec<Types>;
    fn get_fields_name() -> Vec<&'static str>;
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

pub trait TableName {
    fn name() -> &'static str;
}

pub trait InnerJoin<T>: TableName
where
    T: InnerJoin<Self>,
    Self: Sized,
{
    fn fields() -> String;
}

pub enum Types {
    Uuid(Uuid),
    String(String),
    OptString(Option<String>),
    Verbs(Vec<Verbs>),
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

impl From<Vec<Verbs>> for Types {
    fn from(value: Vec<Verbs>) -> Self {
        Self::Verbs(value)
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
