pub mod programas;
pub mod user;

use crate::repository::{QuerySelect, TABLA_PROGRAMA, TABLA_USER};
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

impl QuerySelect for UserAllInfo {
    fn query() -> String {
        format!(
            "SELECT {TABLA_PROGRAMA}.name, {TABLA_PROGRAMA}.icon, {TABLA_PROGRAMA}.id as programa_id, {TABLA_PROGRAMA}.description as programa_description, {TABLA_USER}.* FROM users FULL JOIN {TABLA_PROGRAMA} ON ({TABLA_USER}.programa = {TABLA_PROGRAMA}.id)"
        )
    }
}
