pub mod upload;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use upload::UploadLog;

use super::user::Role;

#[derive(Debug, Deserialize, Serialize)]
pub struct Logs {
    #[serde(with = "time::serde::rfc3339")]
    pub at: OffsetDateTime,
    pub operation: Operation,

    pub owner: Owner,
}

#[derive(Debug, Default, Deserialize, Serialize)]
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
