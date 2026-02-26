use std::{convert::Infallible, str::FromStr};

use redis::{AsyncCommands, Client, aio::MultiplexedConnection};
use serde_json::json;
use uuid::Uuid;

use crate::{grpc_v1::Permissions, models::PermissionsUser};

pub struct Cache {
    pool: MultiplexedConnection,
}

impl Cache {
    async fn new(path: &str) -> Self {
        Self {
            pool: Client::open(path)
                .unwrap()
                .get_multiplexed_async_connection()
                .await
                .unwrap(),
        }
    }

    async fn get_permissions(&mut self, user: Uuid, bucket: &str) -> Vec<PermissionsUser> {
        let resp: String = self
            .pool
            .get(format!("user:{}:bucket:{}", user, bucket))
            .await
            .unwrap();
        serde_json::from_str::<Vec<PermissionsUser>>(resp.as_str()).unwrap()
    }

    async fn set_permissions(&mut self, user: Uuid, bucket: &str, perm: Vec<PermissionsUser>) {
        self.pool
            .set::<'_, _, _, String>(
                format!("user:{}:bucket:{}", user, bucket),
                json!(perm).to_string(),
            )
            .await
            .unwrap();
    }

    async fn get_role(&mut self, user: Uuid) -> Vec<PermissionsUser> {
        let resp: String = self.pool.get(format!("user:{}:role", user)).await.unwrap();
        serde_json::from_str::<Vec<PermissionsUser>>(resp.as_str()).unwrap()
    }

    async fn set_role(
        &mut self,
        user: Uuid,
        bucket: &str,
        perm: Vec<PermissionsUser>,
    ) -> Vec<Permissions> {
        let resp: Vec<String> = self
            .pool
            .set(
                format!("user:{}:bucket:{}", user, bucket),
                json!(perm).to_string(),
            )
            .await
            .unwrap();
        resp.into_iter()
            .map(|x| Permissions::from(x.as_ref()))
            .collect::<Vec<Permissions>>()
    }
}

impl From<&str> for Permissions {
    fn from(value: &str) -> Self {
        match value {
            "Put" => Self::Put,
            "Get" => Self::Get,
            "Delete" => Self::Delete,
            _ => Self::Read,
        }
    }
}
