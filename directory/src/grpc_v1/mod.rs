mod proto {
    tonic::include_proto!("info");
}

use std::sync::Arc;
use futures::StreamExt;
use tokio::sync::{mpsc::{self, Receiver, Sender}, RwLock};
pub use proto::{AllowedBucketReq, AllowedBucketReply, info_client::InfoClient, Permissions, BucketSync};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, transport::Channel};
use uuid::Uuid;

use crate::{bucket::{Bucket, bucket_map::BucketMap}, grpc_v1::proto::bucket_sync::Msg, manager::watcher::for_dir::{self, ForDir}};

pub struct InfoUserProgram {
    inner: InfoClient<Channel>,
    tx_stream: mpsc::Sender<Msg>,
    state: Arc<RwLock<BucketMap>>,
}

impl InfoUserProgram {
    pub async fn new(endpoint: String, state: Arc<RwLock<BucketMap>>) -> Self {
        let (tx,rx) = mpsc::channel(128);

        let resp = Self {
            inner: InfoClient::connect(endpoint).await.unwrap(),
            tx_stream: tx.clone(),
            state: state.clone(),
        };

        //tokio::spawn(Self::stream_handler(resp.inner.clone(), tx, rx, state));

    
        resp
    }
    pub async fn bucket(&self, id: Uuid, name: String, permissions: Permissions) -> AllowedBucketReply {
        self.inner
            .clone()
            .bucket(AllowedBucketReq { id: id.into(), name, permissions:  i32::try_from(permissions).unwrap()})
            .await
            .unwrap()
            .into_inner()
    }

    async fn stream_handler(mut client: InfoClient<Channel>,tx: Sender<BucketSync>, mut rx: Receiver<BucketSync>, state: Arc<RwLock<BucketMap>>) {
        let for_dir = ForDir::from(&*state.read().await);
        let stream = ReceiverStream::new(rx);
        let rx = client.message_for_consistency(Request::new(stream)).await.unwrap();

        let mut inner = rx.into_inner();

        while let Some(Ok(msg)) = inner.next().await {
            match msg.msg.unwrap() {
                Msg::CreateBucket(bucket) => {
                    let for_dir = for_dir.get();
                    let dir = Bucket::new_unchk(format!("{}{}",for_dir.root(), bucket));
                    tracing::trace!("[Stream Handler] {{Create bucket}} bucket: {dir}");
                    if !state.read().await.contains_key(&dir) {
                        tracing::info!("Bucker {bucket} is not present in the root");
                        match tokio::fs::create_dir(format!("{}", dir.as_ref().replace(for_dir.root(), for_dir.real_path()))).await {
                            Ok(()) => {
                                tracing::info!("[Stream Handler] We've created the bucker {bucket} for auth services notifycation");
                            },
                            Err(err) => tracing::error!("{err}"),
                        }
                    }
                },
                Msg::DeleteBucket(bucket) => {
                    let for_dir = for_dir.get();
                    if state.read().await.get(&for_dir.directory(&bucket).unwrap()).filter(|x| x.len() > 0).is_some() {
                        if let Err(err) = tx.send(BucketSync{ operation_id: todo!(), msg: todo!() }).await {

                        }
                    }
                },
                Msg::RenameBucket(rename_bucket) => todo!(),
                Msg::Error(error) => todo!(),
            }
        }
    }
}