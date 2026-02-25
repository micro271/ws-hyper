use super::{Bucket, error::BucketMapErr, object::Object};
use crate::{
    bucket::key::Key,
    manager::Change,
    state::local_storage::{LocalStorage, utils::sync_object_to_database},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::{Path, PathBuf},
};
pub type ObjectTree = BTreeMap<Key<'static>, Vec<Object>>;
pub type BucketMapType = HashMap<Bucket<'static>, ObjectTree>;

#[derive(Debug, Deserialize, Serialize)]
pub struct BucketMap {
    #[serde(flatten)]
    inner: BucketMapType,

    #[serde(skip_serializing)]
    path: PathBuf,
}

impl BucketMap {
    pub fn get_bucket<'a>(&'a self, bucket: Bucket<'a>) -> Option<&'a ObjectTree> {
        self.inner.get(&bucket)
    }

    pub fn get_buckets(&self) -> Vec<&Bucket> {
        self.inner.keys().collect::<Vec<_>>()
    }

    pub fn get_key<'a>(&'a self, bucket: &'a Bucket, key: Key<'a>) -> Option<&'a Vec<Object>> {
        self.inner.get(bucket).and_then(|x| x.get(&key))
    }

    pub fn get_keys<'a>(&'a self, bucket: &'a Bucket) -> Option<Vec<&'a Key<'a>>> {
        self.inner.get(bucket).map(|x| x.keys().collect::<Vec<_>>())
    }

    pub fn get_object_name<'a>(
        &'a self,
        bucket: Bucket<'a>,
        key: Key<'a>,
        name: &str,
    ) -> Option<&'a Object> {
        self.inner
            .get(&bucket)
            .and_then(|x| x.get(&key))
            .and_then(|x| x.iter().find(|x| x.name == name))
    }

    pub fn new_bucket(&mut self, bucket: Bucket<'static>) {
        self.inner
            .entry(bucket)
            .or_default()
            .entry(Key::from("."))
            .or_default();
    }

    pub fn new_key(&mut self, bucket: Bucket<'static>, key: Key<'static>) {
        self.inner
            .entry(bucket)
            .or_default()
            .entry(key)
            .or_default();
    }

    pub fn get_objs_or_insert_default(
        &mut self,
        bucket: Bucket<'static>,
        key: Key<'static>,
    ) -> &mut Vec<Object> {
        self.inner
            .entry(bucket)
            .or_default()
            .entry(key)
            .or_default()
    }

    pub fn new_object(&mut self, bucket: Bucket<'static>, key: Key<'static>, object: Object) {
        self.inner
            .entry(bucket)
            .or_default()
            .entry(key)
            .or_default()
            .push(object);
    }

    pub fn set_name_object(
        &mut self,
        bucket: Bucket<'static>,
        key: Key<'static>,
        file_name: String,
        to: String,
    ) {
        if let Some(val) = self
            .get_objs_or_insert_default(bucket, key)
            .iter_mut()
            .find(|x| x.file_name == file_name)
        {
            val.name = to;
        }
    }

    pub fn set_key(&mut self, bucket: Bucket<'_>, from: Key<'_>, to: Key<'_>) {
        let bk = self.inner.get_mut(&bucket.owned()).unwrap();
        let keys = bk
            .range(&from..)
            .map(|(k, _)| k.cloned())
            .collect::<Vec<_>>();

        for key in &keys {
            let objs = bk.remove(key).unwrap();
            let new_key = key.name().replace(from.name(), to.name()).into();
            bk.insert(new_key, objs);
        }
    }

    pub fn set_name_bucket(&mut self, from: Bucket<'_>, to: Bucket<'_>) {
        let tmp = self.inner.remove(&from.owned()).unwrap();
        self.inner.insert(to.owned(), tmp);
    }

    pub fn remove_object(
        &mut self,
        bucket: Bucket<'_>,
        key: Key<'_>,
        file_name: &str,
    ) -> Option<Object> {
        self.inner
            .get_mut(&bucket.owned())
            .unwrap()
            .get_mut(&key.owned())
            .unwrap()
            .pop_if(|x| x.file_name == file_name)
    }

    pub fn remove_key(&mut self, bucket: Bucket<'_>, key: Key<'_>) {
        self.inner
            .get_mut(&bucket.owned())
            .unwrap()
            .remove(&key.owned());
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
                file_name,
                to,
            } => {
                self.set_name_object(bucket, key, file_name, to);
            }
            Change::NameBucket { from, to } => self.set_name_bucket(from, to),
            Change::NameKey { bucket, from, to } => self.set_key(bucket, from, to),
            Change::DeleteObject {
                bucket,
                key,
                file_name,
            } => {
                self.remove_object(bucket, key, &file_name);
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
                                .map(|x| Bucket::new_unchecked(x.to_str().unwrap()).owned())
                                .unwrap(),
                            BTreeMap::<Key, Vec<Object>>::new(),
                        )
                    })
            })
            .collect::<HashMap<_, _>>();

        let mut path = self.path.clone();
        for (bks, map) in &mut buckets {
            path.push(bks.as_ref());
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
                let objects = sync_objects(objs, bks.borrow(), key.borrow(), local_storage).await;
                tracing::trace!("bucket {bks} - key {key:?} - {objects:?} - path: {path:?}");
                map.insert(key, objects);
            }

            path.pop();
        }
        self.inner = buckets;
        sync_object_to_database(local_storage, self).await;
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
    bucket: Bucket<'_>,
    key: Key<'_>,
    local_storage: &LocalStorage,
) -> Vec<Object> {
    let mut resp = Vec::new();
    for path in vec {
        if let Some(name) = path.file_name().and_then(|x| x.to_str())
            && let Ok(Some(object)) = local_storage
                .get_object_filename(bucket.borrow(), key.borrow(), name)
                .await
        {
            tracing::info!(
                "[ fn_sync_object ] {{ Object found on db (Method::name) }} object: {object:?}"
            );
            resp.push(object);

            continue;
        } else {
            let obj = Object::new(path).await;
            if let Err(er) = local_storage
                .new_object(bucket.borrow(), key.borrow(), &obj)
                .await
            {
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
