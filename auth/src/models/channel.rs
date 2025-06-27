use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
pub struct Channel {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub icon: String,
}
