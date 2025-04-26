use serde::{Deserialize, Serialize};

use crate::models::user::{Role, User};

#[derive(Debug, Deserialize, Serialize)]
pub struct NewUser {
    pub username: String,
    pub password: String,
    pub role: Role,
    pub email: String,
    pub phone: String,
}

impl NewUser {
    pub fn encrypt(&mut self) -> Result<(), &'static str> {
        match bcrypt::hash(self.password.as_bytes(), bcrypt::DEFAULT_COST) {
            Ok(e) => {
                self.password = e;
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    "Error to pass from password simple to hash - user: {} - Err: {}",
                    self.username,
                    e
                );
                Err("Error to create the user")
            }
        }
    }
}

impl From<NewUser> for User {
    fn from(value: NewUser) -> Self {
        Self {
            _id: None,
            username: value.username,
            password: value.password,
            email: value.email,
            phone: value.phone,
            role: Role::Productor,
            ch: None,
        }
    }
}
