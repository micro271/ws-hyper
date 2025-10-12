mod proto {
    tonic::include_proto!("info");
}

pub use proto::{ProgramInfoReply, ProgramInfoRequest, info_client::InfoClient};
use uuid::Uuid;

pub struct InfoUserProgram {
    inner: InfoClient<tonic::transport::Channel>,
}

impl InfoUserProgram {
    pub async fn new(endpoint: String) -> Self {
        Self {
            inner: InfoClient::connect(endpoint).await.unwrap(),
        }
    }
    pub async fn program_name(&self, id: Uuid) -> String {
        self.inner
            .clone()
            .program(ProgramInfoRequest { id: id.into() })
            .await
            .unwrap()
            .into_inner()
            .name
    }
}
