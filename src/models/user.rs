use mongodb::{
    IndexModel,
    bson::{doc, oid::ObjectId},
    options::IndexOptions,
};
use serde::{Deserialize, Serialize};

use crate::repository::{GetCollection, IndexDB};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Claims {
    sub: ObjectId,
    exp: i64,
    role: Role,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserEntry {
    pub username: String,
    pub password: String,
}

impl From<User> for Claims {
    fn from(value: User) -> Self {
        Self {
            sub: value.id.unwrap(),
            exp: (time::OffsetDateTime::now_utc() + time::Duration::hours(2)).unix_timestamp(),
            role: value.role,
        }
    }
}

impl IndexDB for User {
    fn get_unique_index() -> Vec<IndexModel> {
        vec![
            IndexModel::builder()
                .keys(doc! {"username": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        ]
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none", rename = "_id")]
    pub id: Option<ObjectId>,

    pub username: String,
    pub password: String,
    pub email: String,
    pub phone: String,
    pub role: Role,
    pub ch: Option<Ch>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum Role {
    Admin,
    Productor,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admin => write!(f, "Admin"),
            Self::Productor => write!(f, "Productor"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Program {
    pub name: String,
    pub path: String,
    pub icon_path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Ch {
    pub name: String,
    pub number: i8,
    pub icon_path: Option<String>,
    pub description: Option<String>,
    pub program: Program,
}

impl GetCollection for User {
    fn collection() -> &'static str {
        "users"
    }
}
