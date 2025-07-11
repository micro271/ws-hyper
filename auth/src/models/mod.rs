pub mod program;
pub mod user;

use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;

use crate::models::user::{UserState, Verbs};

#[derive(Debug, Deserialize, Serialize)]
pub struct GetUserPubAdm {
    id: Uuid,
    username: String,
    email: String,
    role: String,
    state: UserState,
    phone: String,
    verbs: Verbs,
    resources: String,
    user_description: String,
    program_id: Uuid,
    program: String,
    program_icon: String,
    program_description: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetUserOwn {
    username: String,
    email: String,
    phone: String,
    program_icon: String,
    program_name: String,
    channel: String,
}

impl From<PgRow> for GetUserPubAdm {
    fn from(value: PgRow) -> Self {
        Self {
            id: value.get("id"),
            username: value.get("username"),
            email: value.get("email"),
            role: value.get("role"),
            state: value.get("state"),
            phone: value.get("phone"),
            verbs: value.get("verbs"),
            resources: value.get("resources"),
            user_description: value.get("user_description"),
            program: value.get("program"),
            program_id: value.get("program_id"),
            program_icon: value.get("program_icon"),
            program_description: value.get("program_description"),
        }
    }
}

impl From<PgRow> for GetUserOwn {
    fn from(value: PgRow) -> Self {
        Self {
            username: value.get("username"),
            email: value.get("email"),
            phone: value.get("phone"),
            program_icon: value.get("program_icon"),
            program_name: value.get("program_name"),
            channel: value.get("channel"),
        }
    }
}
