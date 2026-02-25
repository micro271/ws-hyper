mod proto {
    tonic::include_proto!("directory_handler");
}

pub use proto::directory_server::DirectoryServer;
use proto::{FileNameReply, FileNameReq, directory_server::Directory};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;
use tonic::async_trait;

use crate::{
    bucket::{
        Bucket,
        bucket_map::BucketMap,
        key::Key,
        utils::rename_handlers::{
            NewObjNameHandlerBuilder, RenameObjHandler, RenameObjHandlerBuilder,
        },
    },
    state::local_storage::LocalStorage,
};

pub struct BucketGrpcSrv {
    map: Arc<RwLock<BucketMap>>,
    path: PathBuf,
}

impl BucketGrpcSrv {
    pub fn new(map: Arc<RwLock<BucketMap>>, root_path: impl Into<PathBuf>) -> Self {
        Self {
            map,
            path: root_path.into(),
        }
    }
}

#[async_trait]
impl Directory for BucketGrpcSrv {
    async fn file_name(
        &self,
        request: tonic::Request<FileNameReq>,
    ) -> Result<tonic::Response<FileNameReply>, tonic::Status> {
        let FileNameReq { bucket, key, name } = request.into_inner();
        let bucket = Bucket::new_unchecked(bucket);
        let key = Key::new(key);

        match self
            .map
            .read()
            .await
            .get_object_name(bucket.clone(), key.borrow(), &name)
            .map(|x| x.file_name.clone())
        {
            Some(file_name) => Ok(tonic::Response::new(FileNameReply { file_name })),
            None => Err(tonic::Status::not_found(format!(
                "{}/{}/{} not found",
                bucket, key, name
            ))),
        }
    }

    async fn create_object(
        &self,
        request: tonic::Request<FileNameReq>,
    ) -> Result<tonic::Response<FileNameReply>, tonic::Status> {
        let FileNameReq { bucket, key, name } = request.into_inner();

        let mut path = self.path.clone();
        path.push(&bucket);
        path.push(&key);
        path.push(&name);

        if path.exists() {
            Err(tonic::Status::already_exists(format!(
                "{} already exists",
                name
            )))
        } else {
            let file_name = loop {
                match self.map.read().await.get_object_name(
                    Bucket::new_unchecked(&bucket),
                    Key::new(&key),
                    &name,
                ) {
                    Some(_) => {}
                    None => {}
                }
            };
        }
    }
}
