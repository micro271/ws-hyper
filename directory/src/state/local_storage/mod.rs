use uuid::Uuid;
use mongodb::{Client, options::{ClientOptions, Credential}};

const DOC: &str = "objects";

#[derive(Debug)]
pub struct LocalStorage {
    pool: Client,
}

impl LocalStorage {
    pub async fn new(path: &str,username: String, passwd: String, db: String) -> Self {
        
        let mut cred = Credential::default();
        cred.username = Some(username);
        cred.password = Some(passwd);
        let mut opts = ClientOptions::parse(path).await.unwrap();
        opts.default_database = Some(db);
        opts.credential = Some(cred);
        
        Self {
            pool: Client::with_options(opts).unwrap(),
        }
    }
}