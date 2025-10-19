use crate::{directory::tree_dir::TreeDir, manager::new_file_tba::CreateRateLimit};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct State {
    tree: Arc<RwLock<TreeDir>>,
    create_limit: CreateRateLimit,
}

impl State {
    pub fn new(tree: Arc<RwLock<TreeDir>>) -> Self {
        Self {
            tree,
            create_limit: CreateRateLimit::new(),
        }
    }
    pub async fn add_client(&self, id: Uuid) {
        self.create_limit.add_client(id).await;
    }

    pub async fn tree_as_json(&self) -> Value {
        json!(&*self.tree.read().await)
    }
}
