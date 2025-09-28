mod proto {
    tonic::include_proto!("check_user");
}

use std::sync::Arc;

pub use proto::{
    RoleReply, RoleRequest,
    user_control_server::{UserControl, UserControlServer},
};
use tonic::{Response, Status, async_trait};
use uuid::Uuid;

use crate::{models::user::User, repository::{PgRepository, QueryOwn, Types}};

#[derive(Debug)]
pub struct CheckUser {
    repo: Arc<PgRepository>,
}

impl CheckUser {
    pub fn new(repo: Arc<PgRepository>) -> Self {
        Self { repo: repo }
    }
}

#[async_trait]
impl UserControl for CheckUser {
    async fn get_role(
        &self,
        request: tonic::Request<RoleRequest>,
    ) -> Result<Response<RoleReply>, Status> {
        let id = request.into_inner().id;

        let user = self
            .repo
            .get(QueryOwn::<User>::builder().wh("id",Types::Uuid(Uuid::from_bytes(id.try_into().unwrap()))))
            .await
            .unwrap();

        let reply = RoleReply {
            username: user.username,
            role: user.role.to_string(),
            resources: user.resources.unwrap(),
        };

        Ok(Response::new(reply))
    }
}
