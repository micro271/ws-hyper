use super::{Bucket, error::BucketMapErr, object::Object};
use crate::{
    bucket::{key::Key, object::ObjectName},
    manager::Change,
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
    pub fn new_bucket(&mut self, bucket: Bucket) -> &mut BTreeMap<Key, Vec<Object>> {
        self.inner.entry(bucket).or_default()
    }

    pub fn new_key(&mut self, bucket: Bucket, key: Key) -> &mut Vec<Object> {
        self.new_bucket(bucket).entry(key).or_default()
    }

    pub fn new_object(&mut self, bucket: Bucket, key: Key, object: Object) {
        self.new_bucket(bucket).entry(key).or_default().push(object);
    }

    pub fn set_name_object(&mut self, bucket: Bucket, key: Key, from: ObjectName<'_>, to: Object) {
        if let Some(val) = self.new_key(bucket, key)
            .iter_mut()
            .find(|x| x.name() == from) {
                *val = to;
            }
    }

    pub fn set_key(&mut self, bucket: Bucket, from: Key, to: Key) {
        let bk = self.get_mut(&bucket).unwrap();
        let objs = bk.remove(&from).unwrap();
        bk.insert(to, objs);
    }

    pub fn set_name_bucket(&mut self, from: Bucket, to: Bucket) {
        let tmp = self.inner.remove(&from).unwrap();
        self.inner.insert(to, tmp);
    }

    pub fn remove_object(&mut self, bucket: Bucket, key: Key, object: Object) -> Object {
        self.get_mut(&bucket)
            .unwrap()
            .get_mut(&key)
            .unwrap()
            .pop_if(|x| x.name() == object.name())
            .unwrap()
    }

    pub fn remove_key(&mut self, bucket: Bucket, key: Key) {
        self.get_mut(&bucket).unwrap().remove(&key);
    }

    pub async fn change(&mut self, change: Change) {
        match change {
            Change::NewObject {
                bucket,
                key,
                object,
            } => {
                self.new_object(bucket, key, object);
            }
            Change::NewKey { bucket, key } => {
                self.new_key(bucket, key);
            }
            Change::NewBucket { bucket } => {
                self.new_bucket(bucket);
            }
            Change::NameObject {
                bucket,
                key,
                from,
                to,
            } => {
                self.set_name_object(bucket, key, from, to);
            }
            Change::NameBucket { from, to } => self.set_name_bucket(from, to),
            Change::NameKey { bucket, from, to } => self.set_key(bucket, from, to),
            Change::DeleteObject {
                bucket,
                key,
                object,
            } => {
                self.remove_object(bucket, key, object);
            }
            Change::DeleteKey { bucket, key } => {
                self.remove_key(bucket, key);
            }
        }
    }

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

        tracing::trace!("Existing buckets {buckets:#?}");

        let bk_keys = buckets.keys().cloned().collect::<Vec<_>>();
        for bks in bk_keys {
            root_path.push(bks.as_ref());
            let mut list_dirs = root_path
                .read_dir()?
                .filter(|x| x.is_ok())
                .map(|x| x.unwrap().path())
                .collect::<VecDeque<_>>();
            while let Some(dir) = list_dirs.pop_front() {
                if dir.is_dir() {
                    let tmp = dir
                        .read_dir()?
                        .filter(|x| x.is_ok())
                        .map(|x| x.unwrap().path())
                        .collect::<Vec<_>>();
                    tracing::error!("is dir: {bks:?} {tmp:?}");
                    list_dirs.extend(tmp);
                }
                let key = Key::new(
                    dir.parent()
                        .unwrap()
                        .strip_prefix(&root_path)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
                tracing::error!("is object {bks:?} {key:?}");
                let objs = buckets.get_mut(&bks).unwrap().entry(key).or_default();
                if !dir.is_dir() {
                    objs.push(Object::from(dir));
                }
            }

            root_path.pop();
        }

        tracing::info!("Bucket Tree {buckets:#?}");

        Ok(BucketMap {
            inner: buckets,
            path: path,
        })
    }

    fn validate(path: &mut String) -> Result<(), BucketMapErr> {
        let _path = std::fs::canonicalize(&path)?;

        if _path.metadata().unwrap().permissions().readonly() {
            return Err(BucketMapErr::ReadOnly(_path));
        } else if !_path.is_dir() {
            return Err(BucketMapErr::IsNotABucket(_path));
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
