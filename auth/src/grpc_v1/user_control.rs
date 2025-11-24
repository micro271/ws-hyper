mod proto {
    tonic::include_proto!("info");
}

use std::sync::Arc;
pub use proto::{
    UserByIdReq, UserByNameReq,UserReply,
    info_server::{Info, InfoServer},
};
use tonic::{Response, Status, async_trait};
use uuid::Uuid;

use crate::{grpc_v1::user_control::proto::{AllowedBucketReply, AllowedBucketReq, BucketReply, BucketReq}, models::Permissions, state::{PgRepository, QueryOwn, Types}
};

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

    async fn user_by_id(
        &self,
        request: tonic::Request<UserByIdReq>,
    ) -> Result<Response<UserReply>, Status> {
        let id = request.into_inner().id;
        todo!();
        /*
        let reply = self
            .repo
            .get(
                QueryOwn::<UserInfoReply>::builder()
                    .wh("user_id", Types::Uuid(Uuid::from_bytes(id.try_into().unwrap()))),
            )
            .await
            .unwrap();

        Ok(Response::new(reply))
         */
    }

    async fn user_by_name(
        &self,
        request: tonic::Request<UserByNameReq>,
    ) -> Result<Response<UserReply>, Status> {
        let username = request.into_inner().username;
        todo!();
    }

    async fn bucket_is_allowed(&self, request: tonic::Request<AllowedBucketReq>) -> Result<Response<AllowedBucketReply>, Status> {        
        let req = request.into_inner();
        todo!()
    }

    async fn get_bucket(&self, request: tonic::Request<BucketReq>) -> Result<Response<BucketReply>, Status> {
        todo!()
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