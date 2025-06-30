use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow, prelude::FromRow};
use utils::GetClaim;
use uuid::Uuid;

use crate::repository::TableName;

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct User {
    pub id: Option<Uuid>,
    pub username: String,
    pub passwd: String,
    pub email: String,
    pub verbos: Vec<Verbs>,
    pub phone: String,
    pub user_state: UserState,
    pub role: Role,
    pub resources: String,
}

#[derive(Debug, Deserialize, Serialize, sqlx::Type)]
#[sqlx(type_name = "ESTADO")]
pub enum UserState {
    Active,
    Inactive,
}

#[derive(Debug, Deserialize, Serialize, sqlx::Type)]
#[sqlx(type_name = "VERBS")]
pub enum Verbs {
    PutFIle,
    DeleteFIle,
    Read,
    CreareUser,
    ModifyUser,
    CreateCh,
    ModifyCh,
    CreateProgram,
    ModifyProgram,
    All,
}

#[derive(Debug, Deserialize, Serialize, sqlx::Type)]
#[sqlx(type_name = "ROL")]
pub enum Role {
    Administrator,
    Producer,
    Operator,
}

impl From<PgRow> for User {
    fn from(value: PgRow) -> Self {
        User {
            id: value.get("id"),
            username: value.get("username"),
            passwd: value.get("passwd"),
            email: value.get("email"),
            verbos: value.get("verbos"),
            phone: value.get("phone"),
            user_state: value.get("user_state"),
            role: value.get("role"),
            resources: value.get("resource"),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Claim {
    pub sub: String,
    pub exp: i64,
    pub role: Role,
}

impl GetClaim<Claim> for User {
    fn get_claim(self) -> Claim {
        Claim {
            sub: self.username,
            exp: (time::OffsetDateTime::now_utc() + time::Duration::hours(5)).unix_timestamp(),
            role: self.role,
        }
    }
}

impl TableName for User {
    fn name(&self) -> &str {
        "users"
    }
}
