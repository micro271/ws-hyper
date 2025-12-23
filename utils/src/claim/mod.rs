pub mod builder;
pub mod with_metadata;

use serde::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Claim<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    iss: Option<String>,

    sub: T,
    exp: i64,

    #[serde(skip_serializing_if = "Option::is_none")]
    nbf: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    iat: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    jti: Option<uuid::Uuid>,

    #[serde(skip_serializing_if = "Option::is_none")]
    aud: Option<String>,
}

impl<T> Claim<T> {
    pub fn set_iss(&mut self, iss: String)  {
        self.iss = Some(iss);
    }
    pub fn iss(&self) -> Option<&String> {
        self.iss.as_ref()
    }
    pub fn sub(&self) -> &T {
        &self.sub
    }
    pub fn exp(&self) -> i64 {
        self.exp
    }
    pub fn nbf(&self) -> Option<i64> {
        self.nbf
    }
    pub fn iat(&self) -> Option<i64> {
        self.iat
    }
    pub fn jti(&self) -> Option<uuid::Uuid> {
        self.jti
    }
    pub fn aud(&self) -> Option<&String> {
        self.aud.as_ref()
    }
}