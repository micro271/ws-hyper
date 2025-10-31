use crate::{
    bucket::{Bucket, bucket_map::BucketMap},
    grpc_v1::{AllowedBucketReq, InfoClient, Permissions},
    manager::{new_file_tba::CreateRateLimit, websocker::MsgWs},
};
use hyper_tungstenite::HyperWebsocket;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::sync::{RwLock, RwLockReadGuard, mpsc::Sender};
use tonic::transport::Endpoint;
use uuid::Uuid;

#[derive(Debug, Clone)]
enum Connection {
    Not {
        retry_ms: u64,
        attempt: u8,
        endpoint: Endpoint,
    },
    Connected(InfoClient<tonic::transport::Channel>),
}

impl Connection {
    fn default_param(endpoint: Endpoint) -> Self {
        Self::Not {
            retry_ms: 2000,
            attempt: 3,
            endpoint,
        }
    }
}

#[derive(Debug)]
pub struct State {
    tree: Arc<RwLock<BucketMap>>,
    create_limit: CreateRateLimit,
    tx_subs: Sender<MsgWs>,
    info_user_connection: RwLock<Connection>,
}

impl State {
    pub async fn new(
        tree: Arc<RwLock<BucketMap>>,
        new_subs: Sender<MsgWs>,
        endpoint: Endpoint,
    ) -> Self {
        Self {
            tree,
            create_limit: CreateRateLimit::new(),
            tx_subs: new_subs,
            info_user_connection: RwLock::new(
                InfoClient::connect(endpoint.clone())
                    .await
                    .ok()
                    .map(Connection::Connected)
                    .unwrap_or(Connection::default_param(endpoint)),
            ),
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
        let mut clone = self.info_user_connection.read().await.clone();

        while let Connection::Not {
            retry_ms,
            attempt,
            endpoint,
        } = &mut clone
            && *attempt > 0
        {
            tokio::select! {
                () = tokio::time::sleep(Duration::from_millis(*retry_ms)) => {
                    tracing::error!("[Attempt {}] Connection timeout {}", *attempt, *retry_ms);
                    *attempt -= 1;
                },
                conn = InfoClient::connect(endpoint.clone()) => {
                    match conn {
                        Ok(ok) => {
                            *self.info_user_connection.write().await = Connection::Connected(ok.clone());
                            clone = Connection::Connected(ok);
                        },
                        Err(er) => {
                            tracing::error!("[Attempt {}] Connection retry to client grpc fail, error: {er}", *attempt);
                            *attempt -= 1;
                        }
                    };
                },
            };
        }

        if let Connection::Connected(mut con) = clone {
            let req = AllowedBucketReq {
                id: user_id.as_bytes().to_vec(),
                name: bucket_name,
                permissions: i32::try_from(permission).unwrap(),
            };

            match con.bucket(req).await {
                Ok(ok) => Ok(ok.into_inner().allowed),
                Err(err) => Err(err.message().to_string()),
            }
        } else {
            Err("Connection error".to_string())
        }
    }

    pub async fn add_client(&self, subscriber: String, sender: HyperWebsocket) {
        tracing::error!("{subscriber}");
        todo!()
    }

    pub fn create_rate_limit(&self) -> &CreateRateLimit {
        &self.create_limit
    }

    pub async fn tree_as_json(&self) -> Value {
        json!(&*self.tree.read().await)
    }
}
