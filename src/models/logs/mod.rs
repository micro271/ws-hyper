pub mod upload;

use mongodb::{
    IndexModel,
    bson::{doc, oid::ObjectId},
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use upload::UploadLog;

use crate::{
    peer::Peer,
    repository::{GetCollection, IndexDB},
};

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

impl IndexDB for Logs {
    fn get_unique_index() -> Vec<mongodb::IndexModel>
    where
        Self: Sized,
    {
        vec![
            IndexModel::builder()
                .keys(doc! {"operation.type": 1})
                .build(),
            IndexModel::builder()
                .keys(doc! {"owner.username":1})
                .build(),
        ]
    }
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

impl GetCollection for Logs {
    fn collection() -> &'static str {
        "logs"
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
