use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claim {
    pub sub: Uuid,
    pub exp: i64,
}
