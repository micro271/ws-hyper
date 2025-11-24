mod proto {
    tonic::include_proto!("info");
}

use std::{collections::HashMap, pin::Pin, sync::Arc};
use futures::{Stream, StreamExt, lock::Mutex};
pub use proto::{
    UserInfoReply, UserInfoRequest,
    AllowedBucketReply, AllowedBucketReq,
    info_server::{Info, InfoServer},
};
use tonic::{Response, Status, Streaming, async_trait};
use uuid::Uuid;
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    grpc_v1::user_control::proto::{BucketSync, bucket_sync::Msg}, models::{Permissions, UsersBuckets}, state::{PgRepository, QueryOwn, Types}
};

type ResultStream = Pin<Box<dyn Stream<Item = Result<BucketSync, Status>> + Send>>;

#[derive(Debug)]
pub struct InfoUserProgram {
    repo: Arc<PgRepository>,
}

impl InfoUserProgram {
    pub fn new(repo: Arc<PgRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl Info for InfoUserProgram {

    type MessagesStream = ResultStream;

    async fn user(
        &self,
        request: tonic::Request<UserInfoRequest>,
    ) -> Result<Response<UserInfoReply>, Status> {
        let id = request.into_inner().id;

        let reply = self
            .repo
            .get(
                QueryOwn::<UserInfoReply>::builder()
                    .wh("user_id", Types::Uuid(Uuid::from_bytes(id.try_into().unwrap()))),
            )
            .await
            .unwrap();

        Ok(Response::new(reply))
    }

    async fn bucket(&self, request: tonic::Request<AllowedBucketReq>) -> Result<Response<AllowedBucketReply>, Status> {        
        let req = request.into_inner();
        let perm_req = proto::Permissions::try_from(req.permissions).map(|x| Permissions::from(x));
        let bool = self.repo.get::<UsersBuckets>(QueryOwn::builder().wh("user_id", Uuid::from_slice(&req.id[..]).unwrap()).wh("bucket", req.name)).await;
        let msg = AllowedBucketReply { allowed: bool.is_ok_and(|us| perm_req.is_ok_and(|req| us.permissions.into_iter().any(|x| x == req)) ) };
        
        Ok(Response::new(msg))
    }
    
    async fn messages(&self, req: tonic::Request<Streaming<BucketSync>>) -> Result<Response<Self::MessagesStream>, Status> {
        
        let mut stream = req.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        let queue_ops = Arc::new(Mutex::new(HashMap::<Uuid, Msg>::new()));
        
        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(BucketSync { operation_id, msg: Some(msg) }) => {
                        match msg {
                            Msg::CreateBucket(_) => todo!(),
                            Msg::DeleteBucket(_) => todo!(),
                            Msg::RenameBucket(rename_bucket) => todo!(),
                            Msg::Error(kind) => todo!(),
                        }
                    },
                    Err(er) => todo!(),
                    _ => todo!(),
                }
            }
        });

        let out = ReceiverStream::new(rx);

        Ok(Response::new(Box::pin(out) as Self::MessagesStream))
    }
}

impl From<proto::Permissions> for Permissions {
    fn from(value: proto::Permissions) -> Self {
        match value {
            proto::Permissions::Put => Self::Put,
            proto::Permissions::Get => Self::Get,
            proto::Permissions::Delete => Self::Delete,
        }
    }
}