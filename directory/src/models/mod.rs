pub mod object;

use serde::{Deserialize, Serialize};
use crate::grpc_v1::Permissions;


#[derive(Debug, Deserialize, Serialize)]
pub enum PermissionsUser {
    Put,
    Get,
    Read,
    Delete
}

impl From<Permissions> for PermissionsUser {
    fn from(value: Permissions) -> Self {
        match value {
            Permissions::Put => Self::Put,
            Permissions::Get => Self::Get,
            Permissions::Delete => Self::Delete,
            Permissions::Read => Self::Read,
        }
    }
}

impl std::fmt::Display for PermissionsUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Put => write!(f, "Put"),
            Self::Get => write!(f, "Get"),
            Self::Delete => write!(f, "Delete"),
            Self::Read => write!(f, "Read"),
        }
    }
}