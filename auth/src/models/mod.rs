pub mod program;
pub mod user;

use crate::repository::{TABLA_PROGRAMA, TABLA_USER};
use serde::Serialize;
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;

use crate::models::user::{Role, UserState};

#[derive(Debug, Serialize)]
pub struct UserAllInfo {
    pub id: Uuid,
    pub username: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub user_state: UserState,
    pub role: Role,
    pub resources: Option<String>,
    pub desc: Option<String>,
    pub program: Option<ProgramInfo>,
}

#[derive(Debug, Serialize)]
pub struct ProgramInfo {
    pub id: Uuid,
    pub name: String,
    pub desc: Option<String>,
    pub icon: Option<String>,
}

impl From<PgRow> for UserAllInfo {
    fn from(value: PgRow) -> Self {
        Self {
            id: value.get(format!("{TABLA_USER}.id").as_str()),
            username: value.get(format!("{TABLA_USER}.username").as_str()),
            email: value.get(format!("{TABLA_USER}.email").as_str()),
            phone: value.get(format!("{TABLA_USER}.phone").as_str()),
            user_state: value.get(format!("{TABLA_USER}.user_state").as_str()),
            role: value.get(format!("{TABLA_USER}.role").as_str()),
            resources: value.get(format!("{TABLA_USER}.resources").as_str()),
            desc: value.get(format!("{TABLA_USER}.description").as_str()),
            program: if let Some(id) =
                value.get::<'_, Option<Uuid>, _>(format!("{TABLA_PROGRAMA}.id").as_str())
            {
                Some(ProgramInfo {
                    id: id,
                    name: value.get(format!("{TABLA_PROGRAMA}.name").as_str()),
                    desc: value.get(format!("{TABLA_PROGRAMA}.description").as_str()),
                    icon: value.get(format!("{TABLA_PROGRAMA}.icon").as_str()),
                })
            } else {
                None
            },
        }
    }
}
