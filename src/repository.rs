use futures::TryStreamExt;
use mongodb::{
    Client, Database, IndexModel,
    bson::{Bson, Document, doc},
    options::{ClientOptions, Credential},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::models::user::{Encrypt, User};

pub struct Repository {
    inner: Client,
}

impl Repository {
    pub async fn new(
        url: String,
        user: String,
        pass: String,
        db: String,
    ) -> Result<Self, RepositoryError> {
        let cred = Credential::builder().username(user).password(pass).build();
        let mut opt = ClientOptions::parse(url).await?;
        opt.max_connecting = Some(5);
        opt.default_database = Some(db);
        opt.credential = Some(cred);

        let tmp = Self {
            inner: Client::with_options(opt)?,
        };

        let username = "admin".to_string();

        match tmp
            .insert(User {
                id: None,
                username: username.clone(),
                password: "prueba".to_string().encrypt().unwrap(),
                email: None,
                phone: None,
                role: crate::models::user::Role::Admin,
                ch: None,
            })
            .await
        {
            Err(RepositoryError::DuplicateKey) => {
                tracing::warn!("The default username: \"{username}\" already exists")
            }
            Ok(id) => tracing::info!("created default user, _id: {id}"),
            Err(err) => tracing::error!("Unexpected error: {err}"),
        }

        Ok(tmp)
    }

    pub async fn create_index<T>(&self) -> Result<(), RepositoryError>
    where
        T: IndexDB + GetCollection + Send + Sync,
    {
        let collection = T::collection();

        let db = self.get_db()?;

        for index in T::get_unique_index() {
            if let Err(e) = db.collection::<T>(collection).create_index(index).await {
                tracing::error!("{e:?}");
                continue;
            }
        }

        Ok(())
    }

    pub async fn get_one<T>(&self, filter: Document) -> Result<T, RepositoryError>
    where
        T: Send + Sync + DeserializeOwned + GetCollection,
    {
        match self.inner.default_database() {
            Some(db) => match db.collection::<T>(T::collection()).find_one(filter).await {
                Ok(e) => e.ok_or(RepositoryError::DocumentNotFound),
                Err(e) => {
                    tracing::debug!("Error to obtainer one element - Err: {}", e);
                    Err(e.into())
                }
            },
            None => {
                tracing::error!("database not found");
                Err(RepositoryError::DatabaseDefault)
            }
        }
    }

    pub async fn get<T>(&self, filter: Document) -> Result<Vec<T>, RepositoryError>
    where
        T: Send + Sync + DeserializeOwned + GetCollection,
    {
        let collection = T::collection();
        match self
            .get_db()?
            .collection::<T>(collection)
            .find(filter)
            .await
        {
            Ok(e) => match e.try_collect::<Vec<T>>().await {
                Ok(e) => {
                    if e.is_empty() {
                        tracing::debug!("Have not documents in the collection {collection}");
                        Err(RepositoryError::DocumentNotFound)
                    } else {
                        Ok(e)
                    }
                }
                Err(e) => {
                    tracing::error!("error to create the vector - Err: {e}");
                    Err(e.into())
                }
            },
            Err(e) => {
                tracing::error!("Error to obtaine the cursor - Err: {e}");
                Err(e.into())
            }
        }
    }

    pub async fn insert<T>(&self, new: T) -> Result<Bson, RepositoryError>
    where
        T: Serialize + Send + Sync + GetCollection,
    {
        match self
            .get_db()?
            .collection::<T>(T::collection())
            .insert_one(new)
            .await
        {
            Ok(e) => Ok(e.inserted_id),
            Err(e) => {
                tracing::error!("Insert new element fail - Error: {e}");
                Err(e.into())
            }
        }
    }

    pub async fn update<T>(&self, new: Bson, filter: Document) -> Result<u64, RepositoryError>
    where
        T: Send + Sync + GetCollection,
    {
        match self
            .get_db()?
            .collection::<T>(T::collection())
            .update_one(doc! {"$set": new}, filter)
            .await
        {
            Ok(e) => Ok(e.modified_count),
            Err(e) => {
                tracing::error!("Error to update one document");
                Err(e.into())
            }
        }
    }

    pub async fn delete<T>(&self, query: Document) -> Result<u64, RepositoryError>
    where
        T: Send + Sync + GetCollection,
    {
        match self
            .get_db()?
            .collection::<T>(T::collection())
            .delete_one(query)
            .await
        {
            Ok(e) => Ok(e.deleted_count),
            Err(e) => {
                tracing::error!("Error to update one document");
                Err(e.into())
            }
        }
    }

    fn get_db(&self) -> Result<Database, RepositoryError> {
        match self.inner.default_database() {
            Some(e) => Ok(e),
            None => {
                tracing::error!("Default database not found");
                Err(RepositoryError::DatabaseDefault)
            }
        }
    }
}

pub trait GetCollection {
    fn collection() -> &'static str;
}

#[derive(Debug)]
pub enum RepositoryError {
    MongoDb(String),
    DocumentNotFound,
    DatabaseDefault,
    CollectionNotFound,
    DuplicateKey,
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepositoryError::MongoDb(e) => write!(f, "Database Error: {e}"),
            RepositoryError::DocumentNotFound => write!(f, "Document not found"),
            RepositoryError::DatabaseDefault => write!(f, "Database default not defined"),
            RepositoryError::CollectionNotFound => write!(f, "Collection not found"),
            RepositoryError::DuplicateKey => write!(f, "Duplicate key"),
        }
    }
}

impl std::error::Error for RepositoryError {}

impl From<mongodb::error::Error> for RepositoryError {
    fn from(value: mongodb::error::Error) -> Self {
        match *value.kind {
            mongodb::error::ErrorKind::Write(mongodb::error::WriteFailure::WriteError(err))
                if err.code == 11000 =>
            {
                Self::DuplicateKey
            }
            err => Self::MongoDb(err.to_string()),
        }
    }
}

pub trait IndexDB: std::fmt::Debug + GetCollection {
    fn get_unique_index() -> Vec<IndexModel>
    where
        Self: Sized;
}
