use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::repository::GetCollection;

#[derive(Debug, Deserialize, Serialize)]
pub struct Files {
    pub _id: ObjectId,
    pub create_at: time::OffsetDateTime,
    pub stem: String,
    pub extension: String,
    pub elapsed_upload: Option<usize>,
    pub id_tvshow: Uuid,
    pub size: i64,
}

impl GetCollection for Files {
    fn collection() -> &'static str {
        "files"
    }
}
