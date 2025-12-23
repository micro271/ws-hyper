use serde::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ClaimWithMetadata<T, M> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) iss: Option<String>,

    pub(super) sub: T,
    pub(super) exp: i64,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) nbf: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) iat: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) jti: Option<uuid::Uuid>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) aud: Option<String>,

    pub(super) metadata: M,
}

impl<T, M> ClaimWithMetadata<T, M> {
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
    pub fn metadata(&self) -> &M {
        &self.metadata
    }
}