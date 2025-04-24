use futures::StreamExt;
use mongodb::{
    Client,
    bson::{Document, doc},
    options::{ClientOptions, Credential},
};
use serde::{Serialize, de::DeserializeOwned};

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

    pub async fn get_one<T>(&self, filter: Document) -> Option<T>
    where
        T: Send + Sync + DeserializeOwned + GetCollection,
    {
        self.inner
            .default_database()?
            .collection::<T>(T::collection())
            .find_one(filter)
            .await
            .unwrap()
    }

    pub async fn get<T>(&self, collection: &str, filter: Document) -> Option<Vec<T>>
    where
        T: Send + Sync + DeserializeOwned,
    {
        let mut cursor = self
            .inner
            .default_database()?
            .collection::<T>(collection)
            .find(filter)
            .await
            .unwrap();
        let mut resp = Vec::new();

        loop {
            match cursor.next().await {
                Some(Ok(value)) => {
                    resp.push(value);
                }
                Some(Err(e)) => {
                    tracing::error!("Error to get the vlue from database - Error {e}");
                    break None;
                }
                _ => break Some(resp),
            }
        }
    }

    pub async fn insert<T>(&self, new: T) -> Result<String, String>
    where
        T: Serialize + Send + Sync + GetCollection,
    {
        let db = self
            .inner
            .default_database()
            .unwrap_or_else(|| panic!("Default database was not define"));

        db.collection::<T>(T::collection())
            .insert_one(new)
            .await
            .map(|x| x.inserted_id.to_string())
            .map_err(|e| e.to_string())
    }

    pub async fn update<T>(&self, new: T, filter: Document) -> Result<(), String>
    where
        T: Send + Sync + GetCollection,
    {
        let tmp = self
            .inner
            .default_database()
            .unwrap()
            .collection::<T>(T::collection())
            .update_one(doc! {}, filter)
            .await
            .map_err(|x| x.to_string())?;
        Ok(())
    }
}

pub trait GetCollection {
    fn collection() -> &'static str;
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
