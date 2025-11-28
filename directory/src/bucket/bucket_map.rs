use super::{Bucket, error::BucketMapErr, object::Object};
use crate::{bucket::key::Key, manager::Change, state::local_storage::LocalStorage};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::{Path, PathBuf},
};

pub type BucketMapType = HashMap<Bucket, BTreeMap<Key, Vec<Object>>>;

#[derive(Debug, Deserialize, Serialize)]
pub struct BucketMap {
    #[serde(flatten)]
    inner: BucketMapType,

    #[serde(skip_serializing)]
    path: PathBuf,
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

    pub fn set_name_object(&mut self, bucket: Bucket, key: Key, from: String, to: String) {
        if let Some(val) = self
            .new_key(bucket, key)
            .iter_mut()
            .find(|x| x.name == *from)
        {
            val.name = to;
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
            .pop_if(|x| x.name == object.name)
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
            Change::DeleteBucket { .. } => todo!(),
        }
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub async fn build(&mut self, local_storage: &LocalStorage) -> Result<(), BucketMapErr> {
        let mut buckets = std::fs::read_dir(&self.path)?
            .filter_map(|x| {
                x.ok()
                    .filter(|x| x.file_type().map(|x| x.is_dir()).unwrap_or_default())
                    .map(|entry| {
                        (
                            entry
                                .path()
                                .file_name()
                                .map(|x| Bucket::from(x.to_str().unwrap()))
                                .unwrap(),
                            BTreeMap::<Key, Vec<Object>>::new(),
                        )
                    })
            })
            .collect::<HashMap<_, _>>();

        let mut path = self.path.clone();
        for (bks, map) in &mut buckets {
            path.push(bks);
            tracing::trace!("[BucketMap] {{ build }} bucket found: {bks}");

            let mut list_dirs = VecDeque::new();
            list_dirs.push_back(path.clone());

            list_dirs.extend(
                path.read_dir()?
                    .filter_map(|x| x.ok().filter(|y| y.path().is_dir()).map(|x| x.path()))
                    .collect::<Vec<_>>(),
            );

            tracing::trace!("[BucketMap] {{ Directories }} {list_dirs:?}");
            while let Some(dir) = list_dirs.pop_front() {
                let (dirs, objs) = dir_objects(&dir);
                list_dirs.extend(dirs);
                let key = Key::from_bucket(bks, &dir).unwrap();
                let objects = sync_objects(objs, bks, &key, local_storage).await;
                tracing::trace!("bucket {bks} - key {key:?} - {objects:?} - path: {path:?}");
                map.insert(key, objects);
            }

            path.pop();
        }
        self.inner = buckets;

        Ok(())
    }

    pub fn new(path: PathBuf) -> Result<Self, BucketMapErr> {
        if !path.exists() {
            return Err(BucketMapErr::RootPathNotFound(path));
        }

        if !path.is_dir() {
            return Err(BucketMapErr::RootPathNotFound(path));
        }

        Ok(BucketMap {
            inner: Default::default(),
            path: path.canonicalize()?,
        })
    }
}

async fn sync_objects(
    vec: Vec<PathBuf>,
    bucket: &str,
    key: &str,
    local_storage: &LocalStorage,
) -> Vec<Object> {
    let mut resp = Vec::new();
    for path in vec {
        if let Some(name) = path.file_name().and_then(|x| x.to_str())
            && let Some(object) = local_storage.get_object_filename(bucket, key, name).await
        {
            tracing::info!(
                "[ fn_sync_object ] {{ Object found on db (Method::name) }} object: {object:?}"
            );
            resp.push(object);

            continue;
        } else {
            let obj = Object::new(path).await;
            if let Err(er) = local_storage.new_object(bucket, key, &obj).await {
                tracing::error!("{er}");
            }
            resp.push(obj);
        }
    }
    resp
}

fn dir_objects(entry: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut dirs = Vec::new();
    let mut objects = Vec::new();
    tracing::trace!("[ fn_dir_objects ] entry {entry:?}");
    let mut reader = entry.read_dir().unwrap();

    while let Some(Ok(path)) = reader.next() {
        let path = path.path();
        if path.is_dir() {
            dirs.push(path);
        } else {
            objects.push(path);
        }
    }
    tracing::trace!(
        "[ fn_dir_objects ] {{ directories and objects found }} {dirs:?} - {objects:?}"
    );
    (dirs, objects)
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
