mod proto {
    tonic::include_proto!("check_user");
}

pub use proto::{user_control_client::UserControlClient, RoleReply, RoleRequest};

