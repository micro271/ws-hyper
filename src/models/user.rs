use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Claims;

#[derive(Debug, Deserialize)]
pub struct UserEntry {
    pub username: String,
    pub password: String,
}
