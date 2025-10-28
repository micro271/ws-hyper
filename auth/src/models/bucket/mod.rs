pub mod update;

use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow};

use crate::repository::{TABLA_BUCKET, Table};

#[derive(Debug, Deserialize, Serialize)]
pub struct Buckets {
    pub name: String,
    pub description: Option<String>,
}

impl From<PgRow> for Buckets {
    fn from(value: PgRow) -> Self {
        Self {
            name: value.get("name"),
            description: value.get("description"),
        }
    }
}

impl<'a> Table<'a> for Buckets {
    fn name() -> &'a str {
        TABLA_BUCKET
    }

    fn columns() -> Vec<&'a str> {
        vec!["id", "icon", "user_id", "name", "description"]
    }

    fn values(self) -> Vec<crate::repository::Types> {
        vec![
            self.name.into(),
            self.description.into(),
        ]
    }
}
