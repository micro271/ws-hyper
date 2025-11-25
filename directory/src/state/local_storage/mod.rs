use mongodb::{
    Client,
    bson::{self, doc},
    options::{ClientOptions, Credential, ServerAddress},
};
use uuid::Uuid;

use crate::bucket::object::Object;

macro_rules! diff {
    ($t1: expr, $t2: expr $(,$field: ident)+) => {{
        let mut doc = doc!{};
        $(
            if $t1.$field.change(&$t2.$field) {
                doc.insert(stringify!($field), bson::to_bson(&$t2.$field).unwrap());
            }
        )+
        doc
    }};
}

const COLLECTION: &str = "objects";

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
    pub async fn sync_object(&self, obj: &Object) {
        let db = self.pool.default_database().unwrap();
        let tmp = db
            .collection::<Object>(COLLECTION)
            .find_one(doc! {"_id": &obj._id})
            .await
            .unwrap()
            .unwrap();
        let to_update = diff!(
            tmp, obj, size, name, seen_by, taken_by, modified, accessed, created
        );
        _ = db.collection::<Object>(COLLECTION)
            .update_one(doc! {"_id": &obj._id}, to_update)
            .await;
    }

    pub async fn new_object(&self, object: &Object) {
        let tmp = self.pool.default_database().unwrap();
        _ = tmp.collection::<&Object>(COLLECTION)
            .insert_one(object)
            .await;
    }

    pub async fn delete_object(&self, obj: &Object) {
        let tmp = self.pool.default_database().unwrap();
        _ = tmp.collection::<&Object>(COLLECTION)
            .delete_one(doc! {"_id": &obj._id})
            .await;
    }

    pub async fn seen_by(&self, obj: &Object, id: Uuid) {
        let tmp = self.pool.default_database().unwrap();
        _ = tmp.collection::<&Object>(COLLECTION)
            .update_one(
                doc! {"_id": &obj._id },
                doc! { "$addToSet": {"seen_by": id.to_string()} },
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