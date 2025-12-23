pub mod update;

use std::time::Duration;

use bcrypt::{DEFAULT_COST, hash};
use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow, prelude::FromRow};
use utils::claim::{Claim, builder::ClaimBuilder};
use uuid::Uuid;

use crate::state::{TABLA_USER, Table, Types};

impl From<User> for Claim<Uuid> {
    fn from(value: User) -> Self {
        ClaimBuilder::default().sub(value.id.unwrap()).exp(Duration::from_hours(6)).iat(true).build()
    }
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct User {
    pub id: Option<Uuid>,
    pub username: String,
    pub passwd: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub user_state: UserState,
    pub role: Role,
    pub description: Option<String>,
}

impl User {
    pub fn is_admin(&self) -> bool {
        self.role == Role::Administrator || self.role == Role::SuperUser
    }
}

impl std::cmp::PartialEq<Uuid> for User {
    fn eq(&self, other: &Uuid) -> bool {
        self.id.is_some_and(|x| x.eq(other))
    }
}

impl std::cmp::PartialEq<str> for User {
    fn eq(&self, other: &str) -> bool {
        self.username == other
    }
}

#[derive(Debug, Deserialize, Serialize, sqlx::Type, Default)]
#[sqlx(type_name = "USER_STATE")]
pub enum UserState {
    Active,
    #[default]
    Inactive,
}

#[derive(Debug, Deserialize, Serialize, sqlx::Type, Clone, Copy, PartialEq, PartialOrd)]
#[sqlx(type_name = "ROLE")]
pub enum Role {
    SuperUser,
    Administrator,
    Productor,
    Operador,
}

impl From<Role> for i32 {
    fn from(value: Role) -> Self {
        match value {
            Role::SuperUser => 0,
            Role::Administrator => 1,
            Role::Productor => 2,
            Role::Operador => 3,
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::SuperUser => write!(f, "SuperUsuario"),
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
        }
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

pub fn default_account_admin() -> Result<User, Box<dyn std::error::Error>> {
    let mut user = User {
        user_state: UserState::Active,
        id: None,
        username: "admin".to_string(),
        passwd: "admin".to_string(),
        email: None,
        phone: None,
        role: Role::SuperUser,
        description: Some("Default account".to_string()),
    };

    user.passwd = Encrypt::from(&user.passwd)?;

    Ok(user)
}

impl Table for User {
    type ValuesOutput = [Types; 8];
    fn columns() -> &'static [&'static str] {
        &[
            "id",
            "username",
            "passwd",
            "email",
            "phone",
            "user_state",
            "role",
            "description",
        ]
    }
    fn name() -> &'static str {
        TABLA_USER
    }
    fn values(self) -> Self::ValuesOutput {
        [
            self.id.unwrap_or_default().into(),
            self.username.into(),
            self.passwd.into(),
            self.email.into(),
            self.phone.into(),
            self.user_state.into(),
            self.role.into(),
            self.description.into(),
        ]
    }
}

pub struct Encrypt;

impl Encrypt {
    pub fn from(str: &str) -> Result<String, EncryptErr> {
        hash(str, DEFAULT_COST).map_err(|_| EncryptErr)
    }
}
