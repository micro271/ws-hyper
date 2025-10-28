use crate::{
    directory::{Directory, tree_dir::TreeDir}, grpc_v1::{InfoClient, ProgramInfoRequest}, manager::{new_file_tba::CreateRateLimit, websocker::MsgWs}
};
use hyper_tungstenite::HyperWebsocket;
use serde_json::{Value, json};
use tonic::transport::Endpoint;
use uuid::Uuid;
use std::{sync::Arc, time::Duration};
use tokio::sync::{RwLock, RwLockReadGuard, mpsc::Sender};

#[derive(Debug, Clone)]
enum Connection {
    Not{ retry_ms: u64, attempt: u8, endpoint: Endpoint },
    Connected(InfoClient<tonic::transport::Channel>),
}

impl Connection {
    fn default_param(endpoint: Endpoint) -> Self {
        Self::Not { retry_ms: 2000, attempt: 3, endpoint }
    }
}

#[derive(Debug)]
pub struct State {
    tree: Arc<RwLock<TreeDir>>,
    create_limit: CreateRateLimit,
    tx_subs: Sender<MsgWs>,
    info_user_connection: RwLock<Connection>,
}

impl State {
    pub async fn new(tree: Arc<RwLock<TreeDir>>, new_subs: Sender<MsgWs>, endpoint: Endpoint) -> Self {
        Self {
            tree,
            create_limit: CreateRateLimit::new(),
            tx_subs: new_subs,
            info_user_connection: RwLock::new(InfoClient::connect(endpoint.clone()).await.ok().map(Connection::Connected).unwrap_or(Connection::default_param(endpoint))),
        }
    }

    pub async fn read(&self) -> RwLockReadGuard<'_, TreeDir> {
        self.tree.read().await
    }

    pub async fn bucket(&self, id: Uuid) -> Result<String, String> {
        
        let mut clone = self.info_user_connection.read().await.clone();

        while let Connection::Not{retry_ms, attempt  , endpoint} = &mut clone && *attempt > 0 {
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
            let req = ProgramInfoRequest{id: id.as_bytes().to_vec()};
            
            match con.program(req).await {
                Ok(ok) => {
                    Ok(ok.into_inner().name)
                },
                Err(err) => Err(err.message().to_string()),
            }
        } else {
            Err("Connection error".to_string())
        }
    }

    pub async fn add_client(&self, subscriber: String, sender: HyperWebsocket) {

        tracing::error!("{subscriber}");
        if let Err(er) = self
            .tx_subs
            .send(MsgWs::NewUser {
                subscriber: Directory::new_unchk_from_path(subscriber).with_prefix(self.read().await.root()),
                sender,
            })
            .await
        {
            tracing::error!("[State] new subscriber error {er}");
        }
    }

    pub fn create_rate_limit(&self) -> &CreateRateLimit {
        &self.create_limit
    }

    pub async fn tree_as_json(&self) -> Value {
        json!(&*self.tree.read().await)
    }
}
