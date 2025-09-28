mod proto {
    tonic::include_proto!("check_user");
}

use std::{io::Read, sync::Arc};

pub use proto::{
    RoleReply, RoleRequest,
    user_control_server::{UserControl, UserControlServer},
};
use tonic::{Request, Response, Status, async_trait, transport::Server};
use uuid::Uuid;

use crate::{repository::{PgRepository, QueryResult, Types}, Repository};

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
        
        let QueryResult::SelectOne(user ) = self.repo.get_user("id", Types::Uuid(Uuid::from_bytes(id.try_into().unwrap()))).await.unwrap() else {
            panic!()
        };

        let reply = RoleReply {
            username: user.username,
            role: user.role.to_string(),
            resources: user.resources.unwrap(),
        };

        Ok(Response::new(reply))
    }
}