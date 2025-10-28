pub mod bucket;
pub mod user;

use crate::{grpc_v1::user_control::UserInfoReply, repository::{QuerySelect, TABLA_BUCKET, TABLA_USER}};
use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow, prelude::Type};
use uuid::Uuid;

use crate::models::user::{Role, UserState};

#[derive(Debug, Serialize, Deserialize)]
pub struct UsersBuckets {
    pub bucket: String,
    pub user_id: Uuid,
    pub permissions: Vec<Permissions>,
}

#[derive(Debug, Serialize, Deserialize, Type, PartialEq)]
pub enum Permissions {
    Put,
    Get,
    Delete
}

#[derive(Debug, Serialize)]
pub struct UserAllInfo {
    pub id: Uuid,
    pub username: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub user_state: UserState,
    pub role: Role,
    pub resources: Option<String>,
    pub description: Option<String>,
    pub program: Option<ProgramInfo>,
}

#[derive(Debug, Serialize)]
pub struct ProgramInfo {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
}

impl From<PgRow> for UserAllInfo {
    fn from(value: PgRow) -> Self {
        Self {
            id: value.get("id"),
            username: value.get("username"),
            email: value.get("email"),
            phone: value.get("phone"),
            user_state: value.get("user_state"),
            role: value.get("role"),
            resources: value.get("resources"),
            description: value.get("description"),
            program: value
                .get::<'_, Option<Uuid>, _>("programa_description")
                .map(|id| ProgramInfo {
                    id,
                    name: value.get("name"),
                    description: value.get("description"),
                    icon: value.get("icon"),
                }),
        }
    }
}

impl From<PgRow> for UserInfoReply {
    fn from(value: PgRow) -> Self {
        Self { username: value.get("username"), role: value.get("role"), buckets: value.get("buckets") }
    }
}

impl From<PgRow> for UsersBuckets {
    fn from(value: PgRow) -> Self {
        Self { user_id: value.get("user_id"), permissions: value.get("permissions"), bucket: value.get("buckets") }
    }
}

impl QuerySelect for UserAllInfo {
    fn query() -> String {
        format!(
            "SELECT {TABLA_BUCKET}.name, {TABLA_BUCKET}.icon, {TABLA_BUCKET}.id as programa_id, {TABLA_BUCKET}.description as programa_description, {TABLA_USER}.* FROM users FULL JOIN {TABLA_BUCKET} ON ({TABLA_USER}.programa = {TABLA_BUCKET}.id)"
        )
    }
}

impl QuerySelect for UserInfoReply {
    fn query() -> String {
        format!(
            "SELECT user_id, permissions, array_agg(bucket) as buckets FROM {TABLA_BUCKET} INNER JOIN users ON ({TABLA_USER}.id = {TABLA_BUCKET}.user_id) GROUB BY buckets"
        )
    }
}

impl QuerySelect for UsersBuckets {
    fn query() -> String {
        format!(
            "SELECT * FROM {TABLA_BUCKET}"
        )
    }
}