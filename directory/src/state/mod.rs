pub mod cache;
pub mod local_storage;
pub mod pg_listen;

use crate::{
    bucket::{Bucket, bucket_map::BucketMap, key::Key},
    grpc_v1::Permissions,
    manager::{ManagerChSenders, new_file_tba::CreateRateLimit},
};
use hyper_tungstenite::HyperWebsocket;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard};
use uuid::Uuid;

#[derive(Debug)]
pub struct State {
    tree: Arc<RwLock<BucketMap>>,
    create_limit: CreateRateLimit,
    mgr: ManagerChSenders,
}

impl State {
    pub async fn new(tree: Arc<RwLock<BucketMap>>, mgr: ManagerChSenders) -> Self {
        Self {
            tree,
            create_limit: CreateRateLimit::new(),
            mgr,
        }
    }

    pub async fn read(&self) -> RwLockReadGuard<'_, BucketMap> {
        self.tree.read().await
    }

    pub async fn bucket(
        &self,
        user_id: Uuid,
        bucket_name: String,
        permission: Permissions,
    ) -> Result<bool, String> {
        todo!()
    }

    pub async fn add_client(&self, bucket: Bucket, key: Key, sender: HyperWebsocket) {
        /*
        if let Err(er) = self
            .tx_subs
            .send(MsgWs::NewUser {
                bucket,
                key,
                sender,
            })
            .await
        {
            tracing::error!("{er}");
        } */
    }

    pub fn create_rate_limit(&self) -> &CreateRateLimit {
        &self.create_limit
    }

    pub async fn tree_as_json(&self) -> Value {
        json!(&*self.tree.read().await)
    }
}
