mod proto {
    tonic::include_proto!("info");
}

use std::sync::Arc;
pub use proto::{
    UserInfoReply, UserInfoRequest,
    AllowedBucketReply, AllowedBucketReq,
    info_server::{Info, InfoServer},
};
use tonic::{Response, Status, async_trait};
use uuid::Uuid;

use crate::{
    models::{Permissions, UsersBuckets}, repository::{PgRepository, QueryOwn, Types}
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