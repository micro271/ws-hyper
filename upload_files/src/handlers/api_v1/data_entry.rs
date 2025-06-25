use serde::{Deserialize, Serialize};

use crate::models::user::{Role, User};

#[derive(Debug, Deserialize, Serialize)]
pub struct NewUser {
    pub username: String,
    pub password: String,
    pub role: Role,
    pub email: Option<String>,
    pub phone: Option<String>,
}

impl From<NewUser> for User {
    fn from(value: NewUser) -> Self {
        Self {
            id: None,
            username: value.username,
            password: value.password,
            email: value.email,
            phone: value.phone,
            role: Role::Productor,
            ch: None,
        }
    }
}
