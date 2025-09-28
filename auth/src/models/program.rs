use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;

use crate::repository::{TABLA_PROGRAMA, Table};

#[derive(Debug, Deserialize, Serialize)]
pub struct Programa {
    pub id: Uuid,
    pub icon: Option<String>,
    pub user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

impl From<PgRow> for Programa {
    fn from(value: PgRow) -> Self {
        Self {
            id: value.get("id"),
            icon: value.get("icon"),
            user_id: value.get("user_id"),
            name: value.get("name"),
            description: value.get("description"),
        }
    }
}

impl<'a> Table<'a> for Programa {
    fn name() -> &'a str {
        TABLA_PROGRAMA
    }

    fn columns() -> Vec<&'a str> {
        vec!["id", "icon", "user_id", "name", "description"]
    }

    fn values(self) -> Vec<crate::repository::Types> {
        vec![
            self.id.into(),
            self.icon.into(),
            self.user_id.into(),
            self.name.into(),
            self.description.into(),
        ]
    }
}
