mod proto {
    tonic::include_proto!("info");
}

use std::sync::Arc;
pub use proto::{
    UserInfoReply, UserInfoRequest,
    ProgramInfoRequest, ProgramInfoReply,
    info_server::{Info, InfoServer},
};
use tonic::{Response, Status, async_trait};
use uuid::Uuid;

use crate::{
    models::{programas::Programa, user::User},
    repository::{PgRepository, QueryOwn, Types},
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

    async fn program(&self, request: tonic::Request<ProgramInfoRequest>) -> Result<Response<ProgramInfoReply>, Status> {        
        let id = Uuid::from_bytes(request.into_inner().id.try_into().unwrap());
        let msg = ProgramInfoReply {
            name: self.repo.get::<Programa>(QueryOwn::builder().wh("id", id)).await.unwrap().name,
        };
        
        Ok(Response::new(msg))
    }
    
}
