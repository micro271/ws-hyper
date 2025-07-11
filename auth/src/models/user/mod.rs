use bcrypt::{DEFAULT_COST, hash};
use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow, prelude::FromRow};
use utils::GetClaim;
use uuid::Uuid;

use crate::{
    models::program::Programa,
    repository::{InnerJoin, TableName},
};

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct User {
    pub id: Option<Uuid>,
    pub username: String,
    pub passwd: String,
    pub email: Option<String>,
    pub verbos: Vec<Verbs>,
    pub phone: Option<String>,
    pub user_state: UserState,
    pub role: Role,
    pub resources: Option<String>,
    pub description: Option<String>,
}

impl InnerJoin<Programa> for User {
    fn fields() -> String {
        let name = User::name();
        format!(
            "{name}.id, {name}.username, {name}.email, {name}.role, {name}.state, {name}.phone, {name}.verbs, {name}.resource, {name}.descripcion"
        )
    }
}

impl User {
    pub fn encrypt_passwd(&mut self) -> Result<(), EncryptErr> {
        self.passwd = hash(&self.passwd, DEFAULT_COST).map_err(|_| EncryptErr)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize, sqlx::Type, Default)]
#[sqlx(type_name = "ESTADO")]
pub enum UserState {
    Active,
    #[default]
    Inactive,
}

#[derive(Debug, Deserialize, Serialize, sqlx::Type, PartialEq, Eq, Hash, Clone, Copy, Default)]
#[sqlx(type_name = "VERBS")]
pub enum Verbs {
    All,
    ReadFiles,
    PutFile,
    DeleteFIle,
    TakeFile,
    ReadUser,
    ModifyUser,
    CreareUser,
    ReadDirectory,
    ModifyDirectory,
    CreateDirectory,
    #[default]
    None,
}

#[derive(
    Debug, Deserialize, Serialize, sqlx::Type, Default, Clone, Copy, PartialEq, PartialOrd,
)]
#[sqlx(type_name = "ROL")]
pub enum Role {
    Administrator,
    Producer,
    #[default]
    Operator,
}

impl From<PgRow> for User {
    fn from(value: PgRow) -> Self {
        User {
            id: value.get("id"),
            username: value.get("username"),
            passwd: value.get("passwd"),
            description: value.get("description"),
            email: value.get("email"),
            verbos: value.get("verbos"),
            phone: value.get("phone"),
            user_state: value.get("user_state"),
            role: value.get("role"),
            resources: value.get("resources"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claim {
    pub sub: Uuid,
    pub exp: i64,
    pub role: Role,
}

impl GetClaim<Claim> for User {
    fn get_claim(self) -> Claim {
        Claim {
            sub: self.id.unwrap(),
            exp: (time::OffsetDateTime::now_utc() + time::Duration::hours(5)).unix_timestamp(),
            role: self.role,
        }
    }
}

impl TableName for User {
    fn name() -> &'static str {
        "users"
    }
}

#[derive(Debug)]
pub struct EncryptErr;

impl std::fmt::Display for EncryptErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Password encrypt error")
    }
}

impl std::error::Error for EncryptErr {}

#[inline]
pub fn default_account_admin() -> Result<User, Box<dyn std::error::Error>> {
    let mut user = User {
        user_state: UserState::Active,
        id: None,
        username: "admin".to_string(),
        passwd: "admin".to_string(),
        email: None,
        verbos: vec![Verbs::All],
        phone: None,
        role: crate::models::user::Role::Administrator,
        resources: Some("/*".to_string()),
        description: Some("Default account".to_string()),
    };
    user.encrypt_passwd()?;
    Ok(user)
}
