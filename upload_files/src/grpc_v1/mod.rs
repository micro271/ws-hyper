pub mod user_check;

use crate::{grpc_v1::user_check::UserControlClient, models::logs::Logs};
use tonic::Status;
use user_check::{RoleReply, RoleRequest};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct GrpcClient {
    check_user: UserControlClient<tonic::transport::Channel>,
}

impl GrpcClient {
    pub async fn new(endpoint_check_user: String) -> Self {
        Self {
            check_user: UserControlClient::connect(endpoint_check_user)
                .await
                .unwrap(),
        }
    }

    pub async fn user_info(&self, id: Uuid) -> Result<RoleReply, GrpcErr> {
        Ok(self
            .check_user
            .clone()
            .get_role(RoleRequest { id: id.into() })
            .await?
            .into_inner())
    }
}

#[derive(Debug)]
pub struct GrpcErr(String);

impl From<Status> for GrpcErr {
    fn from(value: Status) -> Self {
        GrpcErr(value.message().to_string())
    }
}

impl std::error::Error for GrpcErr {}

impl std::fmt::Display for GrpcErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
