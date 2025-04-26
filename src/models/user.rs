use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

use crate::repository::GetCollection;

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
            sub: value._id.unwrap(),
            exp: (time::OffsetDateTime::now_utc() + time::Duration::hours(2)).unix_timestamp(),
            role: value.role,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    pub _id: Option<ObjectId>,
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
