pub mod upload;

use mongodb::{
    bson::{doc, oid::ObjectId},
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use upload::UploadLog;
use utils::Peer;


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

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Operation {
    Login,
    Upload {
        result: ResultOperation,
        detail: UploadLog,
    },
    Download {
        result: ResultOperation,
        detail: String,
    },
    Rename {
        result: ResultOperation,
        detail: String,
    },
    Delete {
        result: ResultOperation,
        detail: String,
    },
    Take {
        result: ResultOperation,
        detail: String,
    },

    #[default]
    Any,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Owner {
    pub username: String,
    pub src: String,
    pub role: Role,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ResultOperation {
    Success,
    Fail(String),
}
