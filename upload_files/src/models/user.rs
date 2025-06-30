use mongodb::bson::{doc, oid::ObjectId};
use serde::{Deserialize, Serialize};
use utils::GetClaim;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Claim {
    pub sub: String,
    pub exp: i64,
    pub role: Role,
}

impl GetClaim<Claim> for User {
    fn get_claim(self) -> Claim {
        Claim {
            sub: "".to_string(),
            exp: 1,
            role: Role::Admin,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserEntry {
    pub username: String,
    pub password: String,
}

impl From<&User> for Claim {
    fn from(value: &User) -> Self {
        Self {
            sub: value.id.unwrap().to_string(),
            exp: (time::OffsetDateTime::now_utc() + time::Duration::hours(2)).unix_timestamp(),
            role: value.role,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none", rename = "_id")]
    pub id: Option<ObjectId>,

    pub username: String,
    pub password: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub role: Role,
    pub ch: Option<Ch>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq)]
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Program {
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Ch {
    pub name: String,
    pub number: i8,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub program: Vec<Program>,
}
