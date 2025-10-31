use super::{Bucket, error::BucketMapErr, object::Object};
use crate::{
    bucket::key::Key,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::PathBuf,
};

type BucketMapType = HashMap<Bucket, BTreeMap<Key, Vec<Object>>>;

#[derive(Debug, Deserialize, Serialize)]
pub struct BucketMap {
    #[serde(flatten)]
    inner: BucketMapType,

    #[serde(skip_serializing)]
    path: String,
}

impl BucketMap {
    pub fn path(&self) -> &str {
        self.path.as_ref()
    }

    pub fn new(mut path: String) -> Result<Self, BucketMapErr> {
        Self::validate(&mut path)?;
        let path_buf = PathBuf::from(&path);

        if !path_buf.is_dir() {
            return Err(BucketMapErr::IsNotABucket(path_buf));
        }
        if !path.ends_with('/') {
            path.push('/');
        }

        let mut buckets = std::fs::read_dir(&path_buf)?
            .filter_map(|x| {
                x.ok()
                    .filter(|x| x.file_type().map(|x| x.is_dir()).unwrap_or_default())
                    .and_then(|x| {
                        Some(
                            (x.path()
                                .strip_prefix(&path)
                                .ok()
                                .and_then(|x| Some(Bucket::new_unchk(x.to_str()?.to_string())))?,
                                BTreeMap::<Key, Vec<Object>>::new()
                            )
                        )
                    })
            })
            .collect::<HashMap<_,_>>();

        Ok(BucketMap {
            inner: buckets,
            path: path,
        })
    }

    pub fn get_tree(&self) -> &BucketMapType {
        &self.inner
    }

    fn validate(path: &mut String) -> Result<(), BucketMapErr> {
        let _path = std::fs::canonicalize(&path)?;

        if _path.metadata().unwrap().permissions().readonly() {
            return Err(BucketMapErr::ReadOnly(_path));
        } else if !_path.is_dir() {
            return Err(BucketMapErr::IsNotABucket(_path));
        }

        *path = _path.to_str().unwrap().to_string();

        Ok(())
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