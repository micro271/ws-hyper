use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct Usuarios {
    pub id: uuid::Uuid,
    pub user: String,
    pub passwd: String,
    pub email: String,
    pub verbos: Verbs,
    pub user_state: UserState,
    pub role: Role,
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
