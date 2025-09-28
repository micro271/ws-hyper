use bcrypt::{DEFAULT_COST, hash};
use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow, prelude::FromRow};
use utils::GetClaim;
use uuid::Uuid;

use crate::{
    models::program::Programa,
    repository::{InnerJoin, InsertPg, TableName},
};

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct User {
    pub id: Option<Uuid>,
    pub username: String,
    pub passwd: String,
    pub email: Option<String>,
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
    pub fn is_admin(&self) -> bool {
        self.role == Role::Administrator
    }
}

impl std::cmp::PartialEq<Uuid> for User {
    fn eq(&self, other: &Uuid) -> bool {
        self.id.map(|x| x.eq(other)).unwrap_or_default()
    }
}

impl std::cmp::PartialEq<str> for User {
    fn eq(&self, other: &str) -> bool {
        self.username == other
    }
}

#[derive(Debug, Deserialize, Serialize, sqlx::Type, Default)]
#[sqlx(type_name = "ESTADO")]
pub enum UserState {
    Active,
    #[default]
    Inactive,
}

#[derive(
    Debug, Deserialize, Serialize, sqlx::Type, Default, Clone, Copy, PartialEq, PartialOrd,
)]
#[sqlx(type_name = "ROL")]
pub enum Role {
    Administrator,
    Productor,
    #[default]
    Operador,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Administrator => write!(f, "Admin"),
            Role::Productor => write!(f, "Productor"),
            Role::Operador => write!(f, "Operador"),
        }
    }
}

impl From<PgRow> for User {
    fn from(value: PgRow) -> Self {
        User {
            id: value.get("id"),
            username: value.get("username"),
            passwd: value.get("passwd"),
            description: value.get("description"),
            email: value.get("email"),
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
        phone: None,
        role: crate::models::user::Role::Administrator,
        resources: Some("/*".to_string()),
        description: Some("Default account".to_string()),
    };
    user.encrypt_passwd()?;
    Ok(user)
}

impl InsertPg for User {
    fn get_fields(self) -> Vec<crate::repository::Types> {
        vec![
            self.username.into(),
            self.passwd.into(),
            self.email.into(),
            self.user_state.into(),
            self.phone.into(),
            self.role.into(),
            self.resources.into(),
        ]
    }
    fn get_fields_name() -> Vec<&'static str> {
        vec![
            "username",
            "passwd",
            "email",
            "verbs",
            "user_state",
            "phone",
            "role",
            "resources",
        ]
    }
}
