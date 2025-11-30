pub mod error;
pub mod utils;
use mongodb::{
    Client, Database, IndexModel,
    bson::{self, doc},
    options::{ClientOptions, Credential, IndexOptions, ServerAddress},
    results::{InsertOneResult, UpdateResult},
};
use serde::{Deserialize, Serialize};

use uuid::Uuid;

use crate::{bucket::object::Object, state::local_storage::error::LsError};

macro_rules! diff {
    ($t1: expr, $t2: expr $(,$field: ident)+) => {{
        let mut doc = doc!{};
        $(
            if $t1.$field.change(&$t2.$field) {
                doc.insert(concat!("object.", stringify!($field)), bson::to_bson(&$t2.$field).unwrap());
            }
        )+
        doc
    }};
}

const COLLECTION: &str = "objects";

#[derive(Debug, Serialize)]
struct AsObjectSerialize<'a> {
    bucket: &'a str,
    key: &'a str,
    object: &'a Object,
}

impl<'a> AsObjectSerialize<'a> {
    fn new(bucket: &'a str, key: &'a str, obj: &'a Object) -> Self {
        Self {
            bucket,
            key,
            object: obj,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AsObjectDeserialize {
    bucket: String,
    key: String,
    object: Object,
}

#[derive(Debug, Default)]
pub struct LocalStorageBuild {
    password: Option<String>,
    database: Option<String>,
    username: Option<String>,
    host: Option<String>,
    port: Option<u16>,
}

#[derive(Debug)]
pub struct LocalStorage {
    pool: Client,
}

impl LocalStorage {
    pub fn raw(&self) -> Database {
        self.pool.default_database().unwrap()
    }

    pub async fn sync_object(&self, bucket: &str, key: &str, obj: &Object) {
        let db = self.pool.default_database().unwrap();
        let tmp = db
            .collection::<AsObjectDeserialize>(COLLECTION)
            .find_one(doc! {"bucket": bucket, "key": key, "object.file_name": &obj.file_name})
            .await
            .unwrap()
            .unwrap()
            .object;

        let to_update = diff!(
            tmp, obj, name, seen_by, taken_by, modified, accessed, created
        );

        tracing::warn!("{:?}", to_update);
        _ = db
            .collection::<AsObjectSerialize>(COLLECTION)
            .update_one(
                doc! {"bucket": bucket, "key": key, "object.file_name": &obj.file_name },
                doc! {"$set": to_update},
            )
            .await
            .unwrap();
    }

    pub async fn get_object_filename(
        &self,
        bucket: &str,
        key: &str,
        filename: &str,
    ) -> Result<Option<Object>, LsError> {
        let tmp = self.pool.default_database().unwrap();
        let filter = doc! { "bucket": bucket, "key": key, "object.file_name": filename };

        Ok(tmp
            .collection::<AsObjectDeserialize>(COLLECTION)
            .find_one(filter)
            .await?
            .map(|x| x.object))
    }

    pub async fn get_object_name(
        &self,
        bucket: &str,
        key: &str,
        name: &str,
    ) -> Result<Option<Object>, LsError> {
        let tmp = self.pool.default_database().unwrap();
        let filter = doc! { "bucket": bucket, "key": key, "object.name": name };

        Ok(tmp
            .collection::<AsObjectDeserialize>(COLLECTION)
            .find_one(filter)
            .await?
            .map(|x| x.object))
    }

    pub async fn new_object(
        &self,
        bucket: &str,
        key: &str,
        object: &Object,
    ) -> Result<InsertOneResult, LsError> {
        let tmp = self.pool.default_database().unwrap();
        let new = AsObjectSerialize::new(bucket, key, object);

        Ok(tmp
            .collection::<AsObjectSerialize>(COLLECTION)
            .insert_one(new)
            .await?)
    }

    pub async fn delete_object(&self, bucket: &str, key: &str, object: &Object) {
        let tmp = self.pool.default_database().unwrap();
        _ = tmp
            .collection::<&Object>(COLLECTION)
            .delete_one(doc! {"bucket": bucket, "key": key, "object.file_name": &object.file_name})
            .await;
    }

    pub async fn seen_by(&self, bucket: &str, key: &str, obj: &Object, id: Uuid) {
        let tmp = self.pool.default_database().unwrap();
        _ = tmp
            .collection::<AsObjectSerialize>(COLLECTION)
            .update_one(
                doc! {"bucket": bucket, "key": key, "object.file_name": &obj.file_name },
                doc! { "$addToSet": {"object.seen_by": id.to_string()} },
            )
            .await;
    }

    pub async fn set_name(
        &self,
        bucket: &str,
        key: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<UpdateResult, LsError> {
        let tmp = self.pool.default_database().unwrap();
        Ok(tmp
            .collection::<Object>(COLLECTION)
            .update_one(
                doc! {"bucket": bucket, "key": key, "object.name": old_name },
                doc! { "$set": { "object.name": new_name } },
            )
            .await?)
    }
}

impl LocalStorageBuild {
    pub fn username(mut self, username: String) -> Self {
        self.username = Some(username);
        self
    }

    pub fn password(mut self, pass: String) -> Self {
        self.password = Some(pass);
        self
    }

    pub fn host(mut self, host: String) -> Self {
        self.host = Some(host);
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn database(mut self, db: String) -> Self {
        self.database = Some(db);
        self
    }

    pub async fn build(self) -> LocalStorage {
        let Self {
            password,
            database,
            username,
            host,
            port,
        } = self;
        let mut cred = Credential::default();
        cred.username = username;
        cred.password = password;
        let mut opts = ClientOptions::default();
        opts.default_database = database;
        opts.hosts = vec![ServerAddress::Tcp {
            host: host.expect("Mongodb host not defined"),
            port,
        }];
        opts.credential = Some(cred);

        let ls = LocalStorage {
            pool: Client::with_options(opts).unwrap(),
        };

        let index_opts = IndexOptions::builder().unique(true).build();
        let index = IndexModel::builder()
            .keys(doc! { "key": 1, "bucket": 1, "object.name": 1 })
            .options(index_opts)
            .build();
        let db = ls.pool.default_database().unwrap();

        db.collection::<AsObjectSerialize>(COLLECTION)
            .create_index(index)
            .await
            .unwrap();
        ls
    }
}

pub trait Changed<Rhs = Self> {
    fn change(&self, other: &Rhs) -> bool;
}

impl<T, K: PartialEq<T>> Changed<T> for K {
    fn change(&self, other: &T) -> bool {
        self.ne(other)
    }
}
