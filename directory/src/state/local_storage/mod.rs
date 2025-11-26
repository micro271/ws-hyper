use mongodb::{
    Client,
    bson::{self, doc},
    options::{ClientOptions, Credential, ServerAddress},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::bucket::object::Object;

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
    key: &'a str,
    object: &'a Object,
}

impl<'a> AsObjectSerialize<'a> {
    fn new(key: &'a str, obj: &'a Object) -> Self {
        Self { key, object: obj }
    }
}

#[derive(Debug, Deserialize)]
pub struct AsObjectDeserialize {
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
    pub async fn sync_object(&self, key: &str, obj: &Object) {
        let db = self.pool.default_database().unwrap();
        let tmp = db
            .collection::<AsObjectDeserialize>(COLLECTION)
            .find_one(doc! {"key": key, "object.file_name": &obj.file_name})
            .await
            .unwrap()
            .unwrap().object;

        let to_update = diff!(
            tmp, obj, name, seen_by, taken_by, modified, accessed, created
        );
        tracing::warn!("{:?}", to_update);
        _ = db.collection::<AsObjectSerialize>(COLLECTION)
            .update_one(doc! {"key": key, "object.file_name": &obj.file_name }, doc!{"$set": to_update})
            .await.unwrap();
    }

    pub async fn get_object(&self, key: &str, name: &str) -> Option<Object> {
        let tmp = self.pool.default_database().unwrap();
        let filter = doc! { "key": key, "object.file_name": name };

        tmp.collection::<AsObjectDeserialize>(COLLECTION)
            .find_one(filter)
            .await.ok().flatten().map(|x| x.object)
    }

    pub async fn new_object(&self, object: &Object, key: &str) {
        let tmp = self.pool.default_database().unwrap();
        let new = AsObjectSerialize::new(key, object);

        _ = tmp.collection::<AsObjectSerialize>(COLLECTION)
            .insert_one(new)
            .await;
    }

    pub async fn delete_object(&self, obj: &Object) {
        let tmp = self.pool.default_database().unwrap();
        _ = tmp.collection::<&Object>(COLLECTION)
            .delete_one(doc! {"hash": &obj.file_name})
            .await;
    }

    pub async fn seen_by(&self, obj: &Object, id: Uuid) {
        let tmp = self.pool.default_database().unwrap();
        _ = tmp.collection::<&Object>(COLLECTION)
            .update_one(
                doc! {"hash": &obj.file_name },
                doc! { "$addToSet": {"seen_by": id.to_string()} },
            )
            .await;
    }

    pub async fn set_name(&self, key: &str, old_name: &str, new_name: &str) {
        let tmp = self.pool.default_database().unwrap();
        _ = tmp.collection::<Object>(COLLECTION)
            .update_one(
                doc! {"key": key, "object.name": old_name },
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

pub trait Changed<Rhs=Self> {
    fn change(&self, other: &Rhs) -> bool;
}

impl<T, K: PartialEq<T>> Changed<T> for K {
    fn change(&self, other: &T) -> bool {
        self.ne(other)
    }
}