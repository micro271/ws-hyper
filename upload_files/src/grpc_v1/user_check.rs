mod proto {
    tonic::include_proto!("check_user");
}
use crate::models::user::Role;
pub use proto::{user_info_client::UserInfoClient, UserInfoReply, UserInfoRequest};

impl TryFrom<i32> for Role {
    type Error = <proto::Role as TryFrom<i32>>::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        let value = proto::Role::try_from(value)?;

        Ok(match value {
            proto::Role::SuperUs => Role::SuperUs,
            proto::Role::Administrator => Role::Administrator,
            proto::Role::Operador => Role::Operador,
            proto::Role::Productor => Role::Productor,
        })
    }
}