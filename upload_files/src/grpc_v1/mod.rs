pub mod user_check;

use tonic::Status;
use user_check::{UserInfoClient, UserInfoReply, UserInfoRequest};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct GrpcClient {
    check_user: UserInfoClient<tonic::transport::Channel>,
}

impl GrpcClient {
    pub async fn new(endpoint_check_user: String) -> Self {
        Self {
            check_user: UserInfoClient::connect(endpoint_check_user).await.unwrap(),
        }
    }

    pub async fn user_info(&self, id: Uuid) -> Result<UserInfoReply, GrpcErr> {
        Ok(self
            .check_user
            .clone()
            .get_role(UserInfoRequest { id: id.into() })
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
