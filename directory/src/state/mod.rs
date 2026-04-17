pub mod local_storage;

use crate::{
    bucket::{Bucket, Cowed, bucket_map::BucketMap, key::Key},
    grpc_v1::Permissions,
    manager::{ManagerChSenders, websocket::MsgWs},
};
use hyper_tungstenite::HyperWebsocket;
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard};
use uuid::Uuid;

#[derive(Debug)]
pub struct State {
    tree: Arc<RwLock<BucketMap<'static>>>,
    mgr: ManagerChSenders,
}

impl State {
    pub async fn new(tree: Arc<RwLock<BucketMap<'static>>>, mgr: ManagerChSenders) -> Self {
        Self { tree, mgr }
    }

    pub async fn read(&self) -> RwLockReadGuard<'_, BucketMap<'_>> {
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

    pub async fn add_client(
        &self,
        bucket: Option<Bucket<'_>>,
        key: Option<Key<'_>>,
        sender: HyperWebsocket,
    ) {
        if let Err(er) = self
            .mgr
            .ws
            .send(MsgWs::NewUser {
                bucket: bucket.unwrap().owned(),
                key: key.unwrap().owned(),
                sender,
            })
            .await
        {
            tracing::error!("{er}");
        }
    }
}
