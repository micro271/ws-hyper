pub mod update;

use serde::{Deserialize, Serialize};
use sqlx::{Row, postgres::PgRow};

use crate::state::{TABLA_BUCKET, Table, Types};

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

impl Table for Buckets {
    type ValuesOutput = [Types; 2];
    fn name() -> &'static str {
        TABLA_BUCKET
    }

    fn columns() -> &'static[&'static str] {
        &["id", "icon", "user_id", "name", "description"]
    }

    fn values(self) -> Self::ValuesOutput {
        [
            self.name.into(),
            self.description.into(),
        ]
    }
}
