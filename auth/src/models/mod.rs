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
    id: Uuid,
    username: String,
    email: String,
    phone: String,
    program_icon: String,
    program_name: String,
}

impl From<PgRow> for GetUserPubAdm {
    fn from(value: PgRow) -> Self {
        Self {
            id: value.get("users.id"),
            username: value.get("users.username"),
            email: value.get("users.email"),
            role: value.get("users.role"),
            state: value.get("users.state"),
            phone: value.get("users.phone"),
            verbs: value.get("users.verbs"),
            resources: value.get("users.resources"),
            user_description: value.get("users.description"),
            program: value.get("program.name"),
            program_id: value.get("program.id"),
            program_icon: value.get("program.icon"),
            program_description: value.get("program.description"),
        }
    }
}

impl From<PgRow> for GetUserOwn {
    fn from(value: PgRow) -> Self {
        Self {
            id: value.get("users.id"),
            username: value.get("users.username"),
            email: value.get("users.email"),
            phone: value.get("users.phone"),
            program_icon: value.get("program.icon"),
            program_name: value.get("program.name"),
        }
    }
}
