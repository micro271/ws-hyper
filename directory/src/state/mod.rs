pub mod local_storage;

use crate::{
    bucket::{Bucket, bucket_map::BucketMap, key::Key},
    grpc_v1::Permissions,
    manager::ManagerChSenders,
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
}
