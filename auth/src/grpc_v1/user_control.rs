mod proto {
    tonic::include_proto!("check_user");
}

use std::sync::Arc;

pub use proto::{
    UserInfoRequest, UserInfoReply,
    user_info_server::{UserInfo, UserInfoServer},
};
use tonic::{Response, Status, async_trait};
use uuid::Uuid;

use crate::{
    models::user::User, repository::{PgRepository, QueryOwn, Types}
};

#[derive(Debug)]
pub struct CheckUser {
    repo: Arc<PgRepository>,
}

impl CheckUser {
    pub fn new(repo: Arc<PgRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl UserInfo for CheckUser {
    async fn get_role(
        &self,
        request: tonic::Request<UserInfoRequest>,
    ) -> Result<Response<UserInfoReply>, Status> {
        let id = request.into_inner().id;

        let user = self
            .repo
            .get(
                QueryOwn::<User>::builder()
                    .wh("id", Types::Uuid(Uuid::from_bytes(id.try_into().unwrap()))),
            )
            .await
            .unwrap();

        let reply = UserInfoReply {
            username: user.username,
            role: user.role.into(),
            resources: user.resources.unwrap(),
        };

        Ok(Response::new(reply))
    }
}
