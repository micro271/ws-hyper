mod proto {
    tonic::include_proto!("info");
}

use futures::StreamExt;
pub use proto::{
    AllowedBucketReply, AllowedBucketReq, BucketSync, Permissions, info_client::InfoClient,
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{
    RwLock,
    mpsc::{self, Receiver, Sender},
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, transport::Channel};
use uuid::Uuid;

use crate::{
    bucket::{Bucket, bucket_map::BucketMap},
    grpc_v1::proto::bucket_sync::Msg,
};

pub struct InfoUserProgram {
    inner: InfoClient<Channel>,
    tx_stream: mpsc::Sender<Msg>,
    state: Arc<RwLock<BucketMap>>,
}

impl InfoUserProgram {
    pub async fn new(endpoint: String, state: Arc<RwLock<BucketMap>>) -> Self {
        let (tx, rx) = mpsc::channel(128);

        let resp = Self {
            inner: InfoClient::connect(endpoint).await.unwrap(),
            tx_stream: tx.clone(),
            state: state.clone(),
        };

        //tokio::spawn(Self::stream_handler(resp.inner.clone(), tx, rx, state));

        resp
    }

    pub async fn bucket(
        &self,
        id: Uuid,
        name: String,
        permissions: Permissions,
    ) -> AllowedBucketReply {
        self.inner
            .clone()
            .bucket(AllowedBucketReq {
                id: id.into(),
                name,
                permissions: i32::try_from(permissions).unwrap(),
            })
            .await
            .unwrap()
            .into_inner()
    }

    async fn stream_handler(
        mut client: InfoClient<Channel>,
        tx: Sender<BucketSync>,
        rx: Receiver<BucketSync>,
        state: Arc<RwLock<BucketMap>>,
    ) {
        let path = state.read().await.path().to_string();

        let stream = ReceiverStream::new(rx);
        let rx = client
            .message_for_consistency(Request::new(stream))
            .await
            .unwrap();

        let mut inner = rx.into_inner();

        while let Some(Ok(msg)) = inner.next().await {
            let path = &path[..];
            match msg.msg.unwrap() {
                Msg::CreateBucket(bucket) => {
                    let bk = Bucket::new_unchk(format!("{}", bucket));
                    tracing::trace!("[Stream Handler] {{Create bucket}} bucket: {bk}");
                    if !state.read().await.contains_key(&bk) {
                        tracing::info!("Bucker {bucket} is not present in the root");
                        let mut path = PathBuf::from(path);
                        path.push(bucket);
                        match tokio::fs::create_dir(path)
                        .await
                        {
                            Ok(()) => {
                                tracing::info!(
                                    "[Stream Handler] We've created the bucker {bk} for auth services notifycation"
                                );
                            }
                            Err(err) => tracing::error!("{err}"),
                        }
                    }
                }
                Msg::DeleteBucket(bucket) => {
                    if state
                        .read()
                        .await
                        .get(&Bucket::from(bucket))
                        .filter(|x| x.len() > 0)
                        .is_some()
                    {
                        if let Err(err) = tx
                            .send(BucketSync {
                                operation_id: todo!(),
                                msg: todo!(),
                            })
                            .await
                        {}
                    }
                }
                Msg::RenameBucket(rename_bucket) => todo!(),
                Msg::Error(error) => todo!(),
            }
        }
    }
}
