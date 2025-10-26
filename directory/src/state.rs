use crate::{
    directory::{Directory, tree_dir::TreeDir},
    manager::{new_file_tba::CreateRateLimit, websocker::MsgWs},
};
use hyper_tungstenite::HyperWebsocket;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, mpsc::Sender};

pub struct State {
    tree: Arc<RwLock<TreeDir>>,
    create_limit: CreateRateLimit,
    tx_subs: Sender<MsgWs>,
}

impl State {
    pub fn new(tree: Arc<RwLock<TreeDir>>, new_subs: Sender<MsgWs>) -> Self {
        Self {
            tree,
            create_limit: CreateRateLimit::new(),
            tx_subs: new_subs,
        }
    }

    pub async fn read(&self) -> RwLockReadGuard<'_, TreeDir> {
        self.tree.read().await
    }

    pub async fn add_client(&self, subscriber: String, sender: HyperWebsocket) {
        if let Err(er) = self
            .tx_subs
            .send(MsgWs::NewUser {
                subscriber: Directory::new_unchk_from_path(subscriber),
                sender,
            })
            .await
        {
            tracing::error!("[State] new subscriber error {er}");
        }
    }

    pub async fn tree_as_json(&self) -> Value {
        json!(&*self.tree.read().await)
    }
}
