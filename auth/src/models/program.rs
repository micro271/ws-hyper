use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
pub struct Programa {
    icon: String,
    id_user: Uuid,
    name: String,
    description: String,
    ch: String,
}
