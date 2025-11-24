pub mod bucket;
pub mod user;

use crate::grpc_v1::user_control::AllowedBucketReply;
use crate::models::user::{Role, UserState};
use crate::{
    grpc_v1::user_control::{BucketUserProto, UserReply},
    models::user::User,
    state::{QuerySelect, TABLA_BUCKET, TABLA_USER, Table, Types},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Row, postgres::PgRow, prelude::Type};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct BucketUser {
    pub bucket: String,
    pub user_id: Uuid,
    pub permissions: Vec<Permissions>,
}

#[derive(Debug, Serialize, Deserialize, Type, PartialEq)]
pub enum Permissions {
    Put,
    Get,
    Delete,
    Read,
}

#[derive(Debug)]
pub struct PermissionsOutOfRange;

impl TryFrom<i32> for Permissions {
    type Error = PermissionsOutOfRange;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Ok(match value {
            1 => Self::Put,
            2 => Self::Get,
            3 => Self::Delete,
            4 => Self::Read,
            _ => return Err(PermissionsOutOfRange),
        })
    }
}

impl AsRef<str> for Permissions {
    fn as_ref(&self) -> &str {
        match self {
            Permissions::Put => "Put",
            Permissions::Get => "Get",
            Permissions::Delete => "Delete",
            Permissions::Read => "Read",
        }
    }
}

impl std::fmt::Display for Permissions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Permissions::Put => write!(f, "Put"),
            Permissions::Get => write!(f, "Get"),
            Permissions::Delete => write!(f, "Delete"),
            Permissions::Read => write!(f, "Read"),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UserAllInfo {
    pub id: Uuid,
    pub username: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub user_state: UserState,
    pub role: Role,
    pub resources: Option<String>,
    pub description: Option<String>,
    pub program: Option<ProgramInfo>,
}

#[derive(Debug, Serialize)]
pub struct ProgramInfo {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
}

impl From<PgRow> for UserAllInfo {
    fn from(value: PgRow) -> Self {
        Self {
            id: value.get("id"),
            username: value.get("username"),
            email: value.get("email"),
            phone: value.get("phone"),
            user_state: value.get("user_state"),
            role: value.get("role"),
            resources: value.get("resources"),
            description: value.get("description"),
            program: value
                .get::<'_, Option<Uuid>, _>("programa_description")
                .map(|id| ProgramInfo {
                    id,
                    name: value.get("name"),
                    description: value.get("description"),
                    icon: value.get("icon"),
                }),
        }
    }
}

impl From<PgRow> for UserReply {
    fn from(value: PgRow) -> Self {
        let buckets = serde_json::from_str::<Vec<Value>>(value.get("buckets"))
            .map(|x| {
                x.into_iter()
                    .map(|x| BucketUserProto {
                        name: x.get("bucket").unwrap().as_str().unwrap().to_string(),
                        permissions: x
                            .get("permissions")
                            .and_then(|x| x.as_array())
                            .map(|x| {
                                x.into_iter()
                                    .filter_map(|x| x.as_i64().map(|x| x as i32))
                                    .collect::<Vec<i32>>()
                            })
                            .unwrap_or_default(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Self {
            id: value.get("user_id"),
            role: value.get("role"),
            buckets: buckets,
        }
    }
}

impl From<PgRow> for BucketUser {
    fn from(value: PgRow) -> Self {
        Self {
            user_id: value.get("user_id"),
            permissions: value.get("permissions"),
            bucket: value.get("buckets"),
        }
    }
}

impl From<PgRow> for AllowedBucketReply {
    fn from(_: PgRow) -> Self {
        AllowedBucketReply { allowed: true }
    }
}

impl QuerySelect for UserAllInfo {
    fn query() -> String {
        format!(
            "SELECT {TABLA_BUCKET}.name, {TABLA_BUCKET}.icon, {TABLA_BUCKET}.id as programa_id, {TABLA_BUCKET}.description as programa_description, {TABLA_USER}.* FROM users FULL JOIN {TABLA_BUCKET} ON ({TABLA_USER}.programa = {TABLA_BUCKET}.id)"
        )
    }
}

impl QuerySelect for UserReply {
    fn query() -> String {
        let user_bucket_table = BucketUser::name();
        let user_table = User::name();
        format!(
            "SELECT array_agg(json_build_object('bucket', bucket.{user_bucket_table} as buckets, 'permissions', permissions.{user_bucket_table})), user_id,  FROM {user_bucket_table} INNER JOIN {user_table} ON (user_id.{user_bucket_table} = id.{user_table})"
        )
    }
}

impl QuerySelect for AllowedBucketReply {
    fn query() -> String {
        format!("SELECT * FROM {}", BucketUser::name())
    }
}

impl Table for BucketUser {
    type ValuesOutput = [Types; 2];

    fn name() -> &'static str {
        "users_buckets"
    }

    fn columns() -> &'static [&'static str] {
        &["name", "description"]
    }

    fn values(self) -> Self::ValuesOutput {
        [self.bucket.into(), self.user_id.into()]
    }
}
