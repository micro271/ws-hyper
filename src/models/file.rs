use crate::repository::GetCollection;
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

use super::user::Role;

#[derive(Debug, Deserialize, Serialize)]
pub struct FileLog {
    pub _id: Option<ObjectId>,
    pub create_at: time::OffsetDateTime,
    pub file_name: String,
    pub channel: String,
    pub program_tv: String,
    pub elapsed_upload: Option<usize>,
    pub owner: Owner,
    pub size: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Owner {
    pub username: String,
    pub ip_src: String,
    pub role: Role,
}

impl GetCollection for FileLog {
    fn collection() -> &'static str {
        "files"
    }
}
