use crate::{
    handlers::error::ResponseError,
    repository::{GetCollection, IndexDB},
};
use http::StatusCode;
use mongodb::{
    IndexModel,
    bson::{Bson, doc, oid::ObjectId},
    options::IndexOptions,
};
use serde::{Deserialize, Serialize};

pub trait Encrypt {
    type Error;
    type Response;
    fn encrypt(&self) -> Result<Self::Response, Self::Error>;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub role: Role,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserEntry {
    pub username: String,
    pub password: String,
}

impl From<User> for Claims {
    fn from(value: User) -> Self {
        Self {
            sub: value.id.unwrap().to_string(),
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
    pub path: String,
    pub icon_path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
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

#[derive(Debug, Deserialize)]
pub struct UpdateUser {
    pub username: Option<String>,
    pub password: Option<String>,
    pub role: Option<Role>,
}

impl TryFrom<UpdateUser> for Bson {
    type Error = ResponseError;

    fn try_from(value: UpdateUser) -> Result<Self, Self::Error> {
        let mut doc = doc! {};
        if let Some(pass) = value.password {
            doc.insert(
                "password",
                pass.encrypt().map_err(|_| {
                    ResponseError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Some("error to hashing the password"),
                    )
                })?,
            );
        }
        if let Some(username) = value.username {
            doc.insert("username", username);
        }

        Ok(Bson::from(doc))
    }
}

impl Encrypt for String {
    type Error = ResponseError;
    type Response = String;
    fn encrypt(&self) -> Result<Self::Response, Self::Error> {
        match bcrypt::hash(self.as_bytes(), bcrypt::DEFAULT_COST) {
            Ok(e) => Ok(e),
            Err(e) => {
                tracing::error!(
                    "Error to pass from password in text plain to hash Err: {}",
                    e
                );
                Err(ResponseError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Some("Error to hashing the value"),
                ))
            }
        }
    }
}
