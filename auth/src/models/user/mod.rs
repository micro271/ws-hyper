pub mod builder;
pub mod resourece;

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
    pub email: Option<String>,
    pub verbos: Vec<Verbs>,
    pub phone: Option<String>,
    pub user_state: UserState,
    pub role: Role,
    pub resources: Option<String>,
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
    ReadFile,
    PutFile,
    DeleteFIle,
    TakeFile,
    ReadUser,
    ModifyUser,
    CreareUser,
    ReadCh,
    ModifyCh,
    CreateCh,
    CreateProgram,
    ModifyProgram,
    ReadProgram,
    #[default]
    None,
}

#[derive(Debug, Deserialize, Serialize, sqlx::Type, Default)]
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

impl Verbs {
    fn level(&self) -> LevelPermission {
        let level = match self {
            Verbs::All => 0,
            Verbs::ReadFile => 1,
            Verbs::PutFile => 2,
            Verbs::DeleteFIle => 3,
            Verbs::TakeFile => 4,
            Verbs::ReadUser => 5,
            Verbs::ModifyUser => 6,
            Verbs::CreareUser => 7,
            Verbs::ReadCh => 8,
            Verbs::ModifyCh => 9,
            Verbs::CreateCh => 10,
            Verbs::ReadProgram => 11,
            Verbs::ModifyProgram => 12,
            Verbs::CreateProgram => 13,
            Verbs::None => 255,
        };

        LevelPermission::new_unck(level)
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LevelPermission(u8);

pub struct LevelError;

impl LevelPermission {
    fn new(level: u8) -> Result<Self, LevelError> {
        match level {
            0..=13 => Ok(Self(level)),
            _ => Err(LevelError),
        }
    }

    fn new_unck(level: u8) -> Self {
        Self(level)
    }

    fn verb(self) -> Verbs {
        match self.0 {
            0 => Verbs::All,
            1 => Verbs::ReadFile,
            2 => Verbs::PutFile,
            3 => Verbs::DeleteFIle,
            4 => Verbs::TakeFile,
            5 => Verbs::ReadUser,
            6 => Verbs::ModifyUser,
            7 => Verbs::CreareUser,
            8 => Verbs::ReadCh,
            9 => Verbs::ModifyCh,
            10 => Verbs::CreateCh,
            11 => Verbs::ReadProgram,
            12 => Verbs::ModifyProgram,
            13 => Verbs::CreateProgram,
            _ => Verbs::None,
        }
    }

    fn get_normalize(self) -> Vec<Verbs> {
        match self.0 {
            0 => vec![Verbs::All],
            1 => vec![Verbs::ReadFile],
            2 => vec![Verbs::ReadFile, Verbs::PutFile],
            3 => vec![Verbs::ReadFile, Verbs::DeleteFIle],
            4 => vec![Verbs::ReadFile, Verbs::TakeFile],
            5 => vec![Verbs::ReadUser],
            6 => vec![Verbs::ModifyUser, Verbs::ReadUser],
            7 => vec![Verbs::CreareUser],
            8 => vec![Verbs::ReadCh],
            9 => vec![Verbs::ModifyCh, Verbs::ReadCh],
            10 => vec![Verbs::CreateCh],
            11 => vec![Verbs::ReadProgram],
            12 => vec![Verbs::ModifyProgram, Verbs::ReadProgram],
            13 => vec![Verbs::CreateProgram],
            _ => vec![Verbs::None],
        }
    }
}
