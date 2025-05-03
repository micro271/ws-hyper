pub mod upload;

use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use upload::UploadLog;

use crate::{peer::Peer, repository::GetCollection};

use super::user::{Role, User};

#[derive(Debug, Deserialize, Serialize)]
pub struct Logs {
    #[serde(skip_serializing_if = "Option::is_none", rename = "_id")]
    pub id: Option<ObjectId>,

    #[serde(with = "time::serde::rfc3339")]
    pub at: OffsetDateTime,
    pub operation: Operation,

    pub owner: Owner,
}

impl Logs {
    pub fn new(src: String, username: String, role: Role, operation: Operation) -> Self {
        Self {
            id: None,
            at: OffsetDateTime::now_local().unwrap(),
            operation,
            owner: Owner {
                username,
                role,
                src,
            },
        }
    }
    pub fn new_from_user(user: &User, src: &Peer, operation: Operation) -> Self {
        Self {
            owner: Owner {
                username: user.username.clone(),
                src: src.get_ip_or_unknown(),
                role: user.role,
            },
            id: None,
            at: OffsetDateTime::now_local().unwrap(),
            operation,
        }
    }
}

impl GetCollection for Logs {
    fn collection() -> &'static str {
        "logs"
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub enum Operation {
    Login,
    Upload(UploadLog),
    Download(String),
    Rename(String),
    Delete(String),
    Take(String),

    #[default]
    Any,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Owner {
    pub username: String,
    pub src: String,
    pub role: Role,
}
