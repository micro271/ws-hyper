use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
