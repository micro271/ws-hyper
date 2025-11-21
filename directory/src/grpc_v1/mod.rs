mod proto {
    tonic::include_proto!("info");
}

use futures::StreamExt;
pub use proto::{
    AllowedBucketReply, AllowedBucketReq, BucketSync, Permissions, info_client::InfoClient,
};
use std::sync::Arc;
use tokio::sync::{
    RwLock,
    mpsc::{Receiver, Sender, UnboundedSender, channel},
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{
    Request,
    transport::{Channel, Endpoint},
};
use uuid::Uuid;

use crate::{bucket::Bucket, grpc_v1::proto::bucket_sync::Msg, manager::Change};

#[derive(Debug, Clone)]
enum StreamState {
    Pending,
    Executed(Sender<BucketSync>),
    Closed,
}

#[derive(Debug, Clone)]
enum Connection {
    Connected(InfoClient<Channel>),
    Not(Endpoint),
}

#[derive(Debug, Clone)]
pub struct InfoUserGrpc {
    inner: Connection,
    notify_changes: UnboundedSender<Change>,
    state: Arc<RwLock<StreamState>>,
}

impl InfoUserGrpc {
    pub async fn new(endpoint: Endpoint, tx_notify_changes: UnboundedSender<Change>) -> Self {
        Self {
            inner: InfoClient::connect(endpoint.clone())
                .await
                .map(Connection::Connected)
                .unwrap_or(Connection::Not(endpoint)),
            notify_changes: tx_notify_changes,
            state: Arc::new(RwLock::new(StreamState::Pending)),
        }
    }
    async fn get_connect(&mut self) -> Result<InfoClient<Channel>, ()> {
        match &self.inner {
            Connection::Connected(info_client) => Ok(info_client.clone()),
            Connection::Not(endpoint) => {
                if let Ok(ch) = InfoClient::connect(endpoint.clone()).await {
                    self.inner = Connection::Connected(ch.clone());
                    Ok(ch)
                } else {
                    Err(())
                }
            }
        }
    }
    pub async fn bucket(
        &mut self,
        id: Uuid,
        name: String,
        permissions: Permissions,
    ) -> Result<bool, ()> {
        Ok(self
            .get_connect()
            .await?
            .bucket(AllowedBucketReq {
                id: id.into(),
                name,
                permissions: i32::from(permissions),
            })
            .await
            .unwrap()
            .into_inner()
            .allowed)
    }

    pub async fn notify_peer(&self, msg: BucketSync) -> Result<(), ()> {
        if let StreamState::Executed(tx) = &*self.state.read().await
            && let Err(er) = tx.send(msg).await
        {
            tracing::error!("{er}");
            Err(())
        } else {
            Err(())
        }
    }

    pub async fn run_stream(&self) {
        match &mut *self.state.write().await {
            StreamState::Executed(_) => {}
            state => {
                let state = self.state.clone();
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {}

                        }
                    }
                });

                let exc = self.clone();
                let (tx, rx) = channel(256);
                //*state = StreamState::Executed(tx.clone());

                tokio::spawn(exc.stream_handler(tx, rx));
                todo!()
            }
        }
    }

    async fn stream_handler(mut self, tx: Sender<BucketSync>, rx: Receiver<BucketSync>) {
        tracing::info!("[Stream Handler Stated]");
        /*
        let stream = ReceiverStream::new(rx);
        let rx = self.inner
            .message_for_consistency(Request::new(stream))
            .await
            .unwrap();
        let mut inner = rx.into_inner();
        let notify = self.notify_changes;

        loop {
            let Some(msg) = inner.next().await else {
                *self.state.write().await = StreamState::Closed;
                break;
            };

            let (op_id, msg) = match msg {
                Ok(msg) => {
                    (msg.operation_id, msg.msg.unwrap())
                },
                Err(er) =>{
                    tracing::error!("{er}");
                    continue;
                }
            };

            match msg {
                Msg::CreateBucket(bucket) => {
                    let bk = Bucket::new_unchk(bucket.clone());
                    if let Err(er) = notify.send(Change::NewBucket { bucket: bk }) {
                        tracing::error!("{er}");
                    }
                }
                Msg::DeleteBucket(bucket) => {
                    todo!()
                }
                Msg::RenameBucket(rename_bucket) => todo!(),
                Msg::Error(error) => todo!(),
            }
        }
        */
        todo!()
    }
}
