use super::{Bucket, error::BucketMapErr, object::Object};
use crate::bucket::key::Key;
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
        let mut root_path = PathBuf::from(&path);

        let mut buckets = std::fs::read_dir(&root_path)?
            .filter_map(|x| {
                x.ok()
                    .filter(|x| x.file_type().map(|x| x.is_dir()).unwrap_or_default())
                    .map(|entry| {
                        (
                            entry
                                .path()
                                .file_name()
                                .map(|x| Bucket::new_unchk(x.to_string_lossy().into_owned()))
                                .unwrap(),
                            BTreeMap::<Key, Vec<Object>>::new(),
                        )
                    })
            })
            .collect::<HashMap<_, _>>();

        let bk_keys = buckets.keys().cloned().collect::<Vec<_>>();
        for bks in bk_keys {
            root_path.push(bks.as_ref());
            let mut list = VecDeque::new();
            let mut objs = Vec::new();
            let mut reader = root_path.read_dir()?;
            while let Some(Ok(dir)) = reader.next() {
                if dir.file_type().is_ok_and(|x| x.is_dir()) {
                    list.push_back(dir.path());
                } else {
                    objs.push(Object::from(dir.path()));
                }
            }

            buckets
                .get_mut(&bks)
                .unwrap()
                .entry(Key::new(""))
                .or_default()
                .extend(objs);

            while let Some(path) = list.pop_front() {
                let key = 
                    path.strip_prefix(&root_path)
                        .map(|x| Key::new(x.to_string_lossy().into_owned()))
                        .unwrap();
                
                let mut inner_keys = path.read_dir()?;
                let mut objs = Vec::new();
                while let Some(Ok(inner)) = inner_keys.next() {
                    if inner.file_type().unwrap().is_dir() {
                        list.push_back(inner.path());
                    }
                    objs.push(Object::from(inner.path()));
                }
                buckets
                    .get_mut(&bks)
                    .unwrap()
                    .entry(key)
                    .or_default()
                    .extend(objs);
            }
        }

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

        if !path.ends_with('/') {
            path.push('/');
        }

        *path = _path.to_string_lossy().into_owned();

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
