use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::repository::TableName;

#[derive(Debug, Deserialize, Serialize)]
pub struct Programa {
    icon: String,
    id_user: Uuid,
    name: String,
    description: String,
    ch: String,
}

impl TableName for Programa {
    fn name() -> &'static str {
        "programs"
    }
}
