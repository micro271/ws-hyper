use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::repository::TableName;

#[derive(Debug, Deserialize, Serialize)]
pub struct Programa {
    pub id: Uuid,
    pub icon: String,
    pub user_id: Uuid,
    pub name: String,
    pub description: String,
}

impl TableName for Programa {
    fn name() -> &'static str {
        "programs"
    }
}
