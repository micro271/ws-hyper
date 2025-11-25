mod proto {
    tonic::include_proto!("info");
}

use crate::manager::Change;
pub use proto::{AllowedBucketReq, BucketReq, Permissions, info_client::InfoClient};
use proto::{BucketReply, UserByIdReq, UserReply};
use tokio::sync::mpsc::UnboundedSender;
use tonic::transport::{Channel, Endpoint};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ConnectionAuthMS {
    inner: InfoClient<Channel>,
    notify_changes: UnboundedSender<Change>,
}

impl ConnectionAuthMS {
    pub async fn new(endpoint: Endpoint, tx_shc: UnboundedSender<Change>) -> Self {
        
        Self {
            inner: InfoClient::connect(endpoint).await.unwrap(),
            notify_changes: tx_shc,
        }
    }
    pub async fn get_buckets(&self) -> Option<BucketReply> {
        self.inner
            .clone()
            .get_bucket(BucketReq { name: None })
            .await
            .map(|x| x.into_inner())
            .ok()
    }

    pub async fn allowed(&self, id: Uuid, name: String, permission: Permissions) -> bool {
        self.inner
            .clone()
            .bucket_is_allowed(AllowedBucketReq {
                id: id.as_bytes().to_vec(),
                name,
                permissions: permission as i32,
            })
            .await
            .map(|x| x.into_inner().allowed)
            .unwrap_or_default()
    }

    pub async fn buckets_user(&self, id: Uuid) -> Option<UserReply> {
        self.inner
            .clone()
            .user_by_id(UserByIdReq {
                id: id.as_bytes().to_vec(),
            })
            .await
            .map(|x| x.into_inner())
            .ok()
    }
}
