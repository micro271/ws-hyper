use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Claims;

#[derive(Debug, Deserialize)]
pub struct UserEntry {
    pub username: String,
    pub password: String,
}
