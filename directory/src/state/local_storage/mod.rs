use std::collections::{HashMap, HashSet};

use futures::{StreamExt, TryStreamExt};
use mongodb::{
    Client,
    bson::{self, Document, doc},
    options::{ClientOptions, Credential, ServerAddress},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::bucket::{Bucket, bucket_map::{BucketMap, BucketMapType}, key::Key, object::{self, Object}};

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

    pub async fn clear(&self, map: BucketMap) {
        let db = self.pool.default_database().unwrap();
        let mut cursor = db.collection::<AsObjectDeserialize>(COLLECTION).find(doc! {}).await.unwrap();
        let mut tree_aux: BucketMapType = Default::default();

        loop {
            match cursor.next().await {
                Some(Ok(item)) => {
                    let tmp = tree_aux.entry(Bucket::new_unchecked(item.bucket)).or_default();
                    let tmp = tmp.entry(Key::new(item.key)).or_default();
                    tmp.push(item.object);
                },
                Some(Err(er)) => tracing::error!("{er}"),
                None => break,
            }
        }
        let bucket_map = map.keys().map(|x| x.clone()).collect::<HashSet<_>>();
        let bucket_db = tree_aux.keys().map(|x| x.clone()).collect::<HashSet<_>>();
        let dif = bucket_db.difference(&bucket_map).collect::<HashSet<_>>();
        for i in dif {
            if let Err(er) = db.collection::<Document>(COLLECTION).delete_many(doc! {"bucket": i.name()}).await {
                tracing::error!("[LocalStorage] {{ Delete Bucket in Db }} Error: {er}");
            }
            let branch = tree_aux.remove(i);
            tracing::warn!("[LocalStorage] {{ Delete Branch }} bucket: {branch:#?}");
        }
        
        for i in bucket_map {
            let key_map = map.get(&i).unwrap().keys().map(|x| x.clone()).collect::<HashSet<Key>>();
            let key_db = tree_aux.get(&i).unwrap().keys().map(|x| x.clone()).collect::<HashSet<Key>>();
            let dif = key_db.difference(&key_map).collect::<HashSet<_>>();
            for j in dif {
                if let Err(er) = db.collection::<Document>(COLLECTION).delete_many(doc! {"key": j.name()}).await {
                    tracing::error!("[LocalStorage] {{ Delete Bucket in Db }} Error: {er}");
                }
                let branch = tree_aux.get_mut(&i).unwrap();
                branch.remove(&j);
                tracing::warn!("[LocalStorage] {{ Delete Branch }} key: {branch:#?}");
            }
            for m in key_map {
                let vec_map  = map.get(&i).and_then(|x| x.get(&m)).unwrap();
                let vec_db = tree_aux.get(&i).and_then(|x| x.get(&m)).unwrap();
                let to_delete = vec_db.iter().filter(|x| !vec_map.iter().any(|y| y.chechsum == x.chechsum)).collect::<Vec<_>>();
                for n in to_delete {
                    if let Err(er) = db.collection::<Document>(COLLECTION).update_one(doc! {"bucket": i.name(), "key": m.name()} ,doc! {"$pull": { "object.checksum": &n.chechsum }}).await {
                        tracing::error!("[LocalStorage] {{ Delete Object in Db }} Error: {er}");
                    } else {
                        tracing::error!("[LocalStorage] {{ Delete Object in Db }} bucket: {i}, key: {m:?}, object.name: {:?}", n.name);
                    }
                }
            }
        }
        
    }

    pub async fn get_object(&self, bucket: &str, key: &str, name: &str) -> Option<Object> {
        let tmp = self.pool.default_database().unwrap();
        let filter = doc! { "bucket": bucket, "key": key, "object.file_name": name };

        tmp.collection::<AsObjectDeserialize>(COLLECTION)
            .find_one(filter)
            .await
            .ok()
            .flatten()
            .map(|x| x.object)
    }

    pub async fn get_object_hashfile(&self, bucket: &str, key: &str, hash: &str) -> Option<Object> {
        let tmp = self.pool.default_database().unwrap();
        let filter = doc! { "bucket": bucket, "key": key, "object.chechsum": hash };

        tmp.collection::<AsObjectDeserialize>(COLLECTION)
            .find_one(filter)
            .await
            .ok()
            .flatten()
            .map(|x| x.object)
    }

    pub async fn new_object(&self, bucket: &str, key: &str, object: &Object) {
        let tmp = self.pool.default_database().unwrap();
        let new = AsObjectSerialize::new(bucket, key, object);

        _ = tmp
            .collection::<AsObjectSerialize>(COLLECTION)
            .insert_one(new)
            .await;
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
                doc! { "$addToSet": {"seen_by": id.to_string()} },
            )
            .await;
    }

    pub async fn set_name(&self, bucket: &str, key: &str, old_name: &str, new_name: &str) {
        let tmp = self.pool.default_database().unwrap();
        _ = tmp
            .collection::<Object>(COLLECTION)
            .update_one(
                doc! {"bucket": bucket, "key": key, "object.name": old_name },
                doc! { "object.name": new_name },
            )
            .await;
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

    pub fn build(self) -> LocalStorage {
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

        LocalStorage {
            pool: Client::with_options(opts).unwrap(),
        }
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
