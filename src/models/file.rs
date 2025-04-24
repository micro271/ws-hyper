use std::net::IpAddr;

use crate::repository::GetCollection;
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct FileLog {
    pub _id: Option<ObjectId>,
    pub create_at: time::OffsetDateTime,
    pub name: String,
    pub elapsed_upload: Option<usize>,
    pub owner: Owner,
    pub size: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Owner {
    pub username: String,
    pub ip_src: IpAddr,
    pub email: String,
}

impl GetCollection for FileLog {
    fn collection() -> &'static str {
        "files"
    }
}
