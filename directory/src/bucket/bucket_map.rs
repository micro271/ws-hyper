use super::{Bucket, error::BucketMapErr, key::Object};
use crate::{bucket::WithPrefixRoot, manager::{utils::FromDirEntyAsync as _, watcher::for_dir::ForDir}};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
};
use tokio::fs;

type BucketMapType = BTreeMap<Bucket, Vec<Object>>;

#[derive(Debug, Deserialize, Serialize)]
pub struct BucketMap {
    #[serde(flatten)]
    inner: BucketMapType,

    #[serde(skip_serializing)]
    real_path: String,

    #[serde(skip_serializing)]
    root: String,
}

impl BucketMap {
    pub fn real_path(&self) -> &str {
        &self.real_path
    }

    pub fn root(&self) -> &str {
        self.root.as_ref()
    }

    pub async fn new_async(path: &str, mut prefix_root: String) -> Result<Self, BucketMapErr> {
        let path = Self::validate(path).await?;
        let path_buf = PathBuf::from(&path);

        if !prefix_root.ends_with("/") {
            prefix_root.push('/');
        }

        if !path_buf.is_dir() {
            return Err(BucketMapErr::IsNotABucket(path_buf));
        }

        let mut read_dir = fs::read_dir(&path_buf).await?;

        let mut vec = vec![];
        let mut queue = VecDeque::new();
        let mut resp = BTreeMap::new();
        tracing::info!("Bucket: {path:?}");
        tracing::info!("Root path: {prefix_root:?}");

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if entry.file_type().await.is_ok_and(|x| x.is_dir()) {
                queue.push_front(entry.path());
            }
            vec.push(Object::from_entry(entry).await);
            println!("{vec:?}, {queue:?}");
        }

        let directory = Bucket(prefix_root.to_string());
        resp.insert(directory, vec);

        while let Some(dir) = queue.pop_front() {
            let mut read_dir = fs::read_dir(&dir).await.unwrap();
            let mut vec = vec![];
            while let Ok(Some(entry)) = read_dir.next_entry().await {
                if entry
                    .file_type()
                    .await
                    .map(|x| x.is_dir())
                    .unwrap_or_default()
                {
                    queue.push_back(entry.path());
                }
                vec.push(Object::from_entry(entry).await);
            }
            resp.insert(
                Bucket::from(WithPrefixRoot::new(dir, &path, &prefix_root)),
                vec,
            );
        }
        tracing::error!("{path:?}, {prefix_root:?}");
        Ok(BucketMap {
            inner: resp,
            real_path: path,
            root: prefix_root,
        })
    }

    pub fn get_tree(&self) -> &BucketMapType {
        &self.inner
    }

    async fn validate(path: &str) -> Result<String, BucketMapErr> {
        if path == "/" {
            return Err(BucketMapErr::RootNotAllowed);
        }

        let path = fs::canonicalize(path).await?;

        if path.metadata().unwrap().permissions().readonly() {
            return Err(BucketMapErr::ReadOnly(path));
        } else if !path.is_dir() {
            return Err(BucketMapErr::IsNotABucket(path));
        }

        let mut path = path.to_str().unwrap().to_string();

        if !path.ends_with("/") {
            path.push('/');
        };

        Ok(path)
    }
}

impl std::ops::Deref for BucketMap {
    type Target = BucketMapType;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for BucketMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl From<&BucketMap> for ForDir<String> {
    fn from(value: &BucketMap) -> Self {
        Self::new(value.root().to_string(), value.real_path().to_string())
    }
}