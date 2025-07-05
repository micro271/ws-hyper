use sqlx::{
    pool::Pool,
    postgres::{PgPoolOptions, Postgres},
};
use uuid::Uuid;

use crate::models::user::{User, UserState, Verbs};

macro_rules! get {
    ($pool:expr, $t:ty, $key:expr, $value:expr) => {
        async {
            let query = format!("SELECT FROM {} WHERE {} = {}", <$t>::name(), $key, $value);
            let resp = sqlx::query(&query).fetch_one($pool).await.unwrap();
            <$t>::from(resp)
        }
    };
    ($pool:expr, $t:ty) => {
        async {
            let query = format!("SELECT * FROM {}", <$t>::name());
            let resp = sqlx::query(&query).fetch_all($pool).await.unwrap();

            resp.into_iter().map(|x| <$t>::from(x)).collect::<Vec<$t>>()
        }
    };

    ($pool:expr, $t:ty, $(inner_join => $join1:ty, $join2:ty, $key_join1:expr, $key_join2:expr),+ ) => {
        async {
            let mut query = format!("SELECT * FROM {}", <$join1>::name());
            $(
                let key1 = format!("{}.{}", <$join1>::name(), $key_join1);
                let key2 = format!("{}.{}", <$join2>::name(), $key_join2);
                query.push_str(&format!(" INNET JOIN {} ON {} = {}", key1, key2));
            )*

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
            passwd: "admin".to_string(),
            email: None,
            verbos: vec![Verbs::All],
            phone: None,
            role: crate::models::user::Role::Administrator,
            resources: Some("/*".to_string()),
        };
        _ = repo.insert_one_user(user).await;
        Ok(repo)
    }

    pub async fn get_users(&self) -> Result<Vec<User>, RepositoryError> {
        let get = get!(&self.inner, User).await;
        Ok(get)
    }

    pub async fn get_user(&self, username: &str) -> Result<User, RepositoryError> {
        let tmp = get!(&self.inner, User, "username", username).await;

        Ok(tmp)
    }

    pub async fn get_user_with_id(&self, id: Uuid) -> Result<User, RepositoryError> {
        let tmp = get!(&self.inner, User, "id", id).await;
        Ok(tmp)
    }

    pub async fn insert_one_user(&self, user: User) -> Result<QueryResult<User>, RepositoryError> {
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
    fn name() -> &'static str;
}
