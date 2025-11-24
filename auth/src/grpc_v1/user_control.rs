mod proto {
    tonic::include_proto!("info");
}

pub use proto::{
    AllowedBucketReply, BucketUser as BucketUserProto, UserByIdReq, UserReply,
    info_server::{Info, InfoServer},
};
use std::sync::Arc;
use tonic::{Response, Status, async_trait};
use uuid::Uuid;

use crate::{
    grpc_v1::user_control::proto::{AllowedBucketReq, BucketReply, BucketReq},
    models::{BucketUser, Permissions},
    state::{PgRepository, QueryOwn, Types},
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

        let reply = self
            .repo
            .get(
                QueryOwn::<UserReply>::builder()
                    .wh(
                        "user_id",
                        Types::Uuid(Uuid::from_bytes(id.try_into().unwrap())),
                    )
                    .group_by("user_id"),
            )
            .await
            .unwrap();

        Ok(Response::new(reply))
    }

    async fn bucket_is_allowed(
        &self,
        request: tonic::Request<AllowedBucketReq>,
    ) -> Result<Response<AllowedBucketReply>, Status> {
        let AllowedBucketReq {
            id,
            name,
            permissions,
        } = request.into_inner();
        let id = Uuid::from_slice(&id[..]).unwrap();

        let permission = Permissions::try_from(permissions).unwrap();

        let resp = self
            .repo
            .get(
                QueryOwn::<AllowedBucketReply>::builder()
                    .wh("name", name)
                    .wh("user_id", id)
                    .wh_vec_any("permissions", vec![permission]),
            )
            .await
            .unwrap_or(AllowedBucketReply { allowed: false });

        Ok(Response::new(resp))
    }

    async fn get_bucket(
        &self,
        request: tonic::Request<BucketReq>,
    ) -> Result<Response<BucketReply>, Status> {
        let req = request.into_inner();
        let qr = req.name.map_or(QueryOwn::<BucketUser>::builder(), |q| {
            QueryOwn::<BucketUser>::builder().wh("name", q)
        });
        let resp = self.repo.gets(qr).await.unwrap_or_default();
        Ok(Response::new(resp.into()))
    }
}

impl From<proto::Permissions> for Permissions {
    fn from(value: proto::Permissions) -> Self {
        match value {
            proto::Permissions::Put => Self::Put,
            proto::Permissions::Get => Self::Get,
            proto::Permissions::Delete => Self::Delete,
            proto::Permissions::Read => Self::Read,
        }
    }
}

impl From<Vec<BucketUser>> for BucketReply {
    fn from(value: Vec<BucketUser>) -> Self {
        BucketReply {
            buckets: value.into_iter().map(|x| x.bucket).collect(),
        }
    }
}
