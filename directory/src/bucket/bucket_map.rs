use super::{Bucket, error::BucketMapErr, object::Object};
use crate::{
    bucket::{
        Cowed,
        fhs_response::FhsResponse,
        key::Key,
        utils::{
            Rename, RenameDecision, list_buckets_and_normalize,
            normalizeds::{NormalizeFileUtf8, NormalizePathUtf8},
        },
    },
    manager::Change,
    state::local_storage::{LocalStorage, utils::sync_object_with_database},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::{Path, PathBuf},
};
pub type ObjectTree<'a, T> = BTreeMap<Key<'a>, Vec<T>>;
pub type BucketMapType<'a, T> = HashMap<Bucket<'a>, ObjectTree<'a, T>>;

#[derive(Debug, Deserialize, Serialize)]
pub struct BucketMap<'a> {
    #[serde(flatten)]
    inner: BucketMapType<'a, Object>,

    #[serde(skip_serializing)]
    path: PathBuf,
}

impl<'a> BucketMap<'a> {
    pub fn new(path: PathBuf) -> Result<Self, BucketMapErr> {
        let path = path.canonicalize()?;
        tracing::info!("[ BucketMap ] Root path: {path:?}");
        if !path.exists() {
            Err(BucketMapErr::RootPathNotFound(path))
        } else if !path.is_dir() {
            Err(BucketMapErr::RootPathIsNotDirectory(path))
        } else {
            Ok(BucketMap {
                inner: Default::default(),
                path,
            })
        }
    }

    pub fn get_response<'b>(
        &'b self,
        bucket: Option<&'b Bucket<'_>>,
        key: Option<&'b Key<'_>>,
    ) -> Option<FhsResponse<'b>> {
        let (Some(bucket), Some(key)) = (bucket, key) else {
            let buckets = self.get_buckets().map(|x| x.name()).collect::<Vec<_>>();
            return Some(FhsResponse::new(buckets, None));
        };

        let tree = self.inner.get(&bucket)?;
        let objects = tree.get(&key);
        let key_ = key.name();
        let mut keys = key
            .is_root()
            .then(|| {
                tree.keys()
                    .map(|x| x.name())
                    .filter(|x| x.ne(&"."))
                    .filter_map(|x| x.split("/").next())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                tree.range(key..)
                    .take_while(|(k, _)| k.name().starts_with(key_))
                    .filter_map(|(k, _)| {
                        k.name()
                            .strip_prefix(key_)
                            .and_then(|x| x.strip_prefix("/"))
                            .and_then(|x| x.split("/").next())
                    })
                    .collect::<Vec<_>>()
            });
        keys.dedup();
        Some(FhsResponse::new(keys, objects))
    }

    pub fn get_bucket<'b>(&'b self, bucket: Bucket<'b>) -> Option<&'b ObjectTree<'b, Object>> {
        self.inner.get(&bucket)
    }

    pub fn get_buckets(&self) -> impl Iterator<Item = &Bucket<'_>> {
        self.inner.keys()
    }

    pub fn get_key<'b>(&'b self, bucket: Bucket<'b>, key: Key<'b>) -> Option<&'b Vec<Object>> {
        self.inner.get(&bucket).and_then(|x| x.get(&key))
    }

    pub fn get_keys<'b>(&'b self, bucket: Bucket<'b>) -> Option<Vec<Key<'b>>> {
        self.inner
            .get(&bucket)
            .map(|x| x.keys().map(|x| x.borrow()).collect::<Vec<_>>())
    }

    pub fn get_object_by_file_name<'b>(
        &'b self,
        bucket: Bucket<'b>,
        key: Key<'b>,
        name: &str,
    ) -> Option<&'b Object> {
        self.inner
            .get(&bucket)
            .and_then(|x| x.get(&key))
            .and_then(|x| x.iter().find(|x| x.file_name == name))
    }

    pub fn insert_bucket(&mut self, bucket: Bucket<'a>) {
        self.inner.entry(bucket).or_default();
    }

    pub fn insert_key(&mut self, bucket: Bucket<'a>, key: Key<'a>) {
        self.inner
            .entry(bucket)
            .or_default()
            .entry(key)
            .or_default();
    }

    pub fn get_objects<'b>(
        &'b mut self,
        bucket: &'b Bucket<'_>,
        key: &'b Key<'_>,
    ) -> Option<&'b Vec<Object>> {
        self.inner.get(&bucket).and_then(|x| x.get(&key))
    }

    pub fn insert_object(&mut self, bucket: Bucket<'a>, key: Key<'a>, object: Object) {
        self.inner
            .entry(bucket)
            .or_default()
            .entry(key)
            .or_default()
            .push(object);
    }

    pub fn set_name_object(
        &mut self,
        bucket: &Bucket<'a>,
        key: &Key<'a>,
        from: &str,
        to: impl Into<String>,
    ) {
        if let Some(val) = self.inner.get_mut(bucket).and_then(|x| {
            x.get_mut(key)
                .and_then(|x| x.iter_mut().find(|x| x.file_name == from))
        }) {
            val.file_name = to.into();
        }
    }

    pub fn set_key(&mut self, bucket: Bucket<'a>, from: Key<'a>, to: Key<'a>) {
        let bk = self.inner.get_mut(&bucket).unwrap();
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

    pub fn set_name_bucket(&mut self, from: Bucket<'a>, to: Bucket<'a>) {
        let tmp = self.inner.remove(&from).unwrap();
        self.inner.insert(to, tmp);
    }

    pub fn remove_object(
        &mut self,
        bucket: Bucket<'a>,
        key: Key<'a>,
        file_name: &str,
    ) -> Option<Object> {
        self.inner
            .get_mut(&bucket)
            .unwrap()
            .get_mut(&key)
            .unwrap()
            .pop_if(|x| x.file_name == file_name)
    }

    pub fn remove_key(&mut self, bucket: Bucket<'a>, key: Key<'a>) {
        self.inner.get_mut(&bucket).unwrap().remove(&key);
    }

    pub async fn change(&mut self, change: Change) {
        match change {
            Change::NewObject {
                bucket,
                key,
                object,
            } => {
                self.insert_object(bucket, key, object);
            }
            Change::NewKey { bucket, key } => {
                self.insert_key(bucket, key);
            }
            Change::NewBucket { bucket } => {
                self.insert_bucket(bucket);
            }
            Change::NameObject {
                bucket,
                key,
                from,
                to,
            } => {
                self.set_name_object(&bucket, &key, &from, to);
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
        let mut buckets = list_buckets_and_normalize(&self.path)
            .await
            .into_iter()
            .map(|x| (x, BTreeMap::new()))
            .collect::<HashMap<Bucket, _>>();

        tracing::trace!("[ BucketMap ] {{ build }} bucket found: {buckets:?}");
        for (bks, map) in &mut buckets {
            tracing::trace!("[ BucketMap ] {{ build }} create branch for bucket: {bks}");

            let mut list_dirs = VecDeque::from([self.path.join(bks.name())]);

            tracing::trace!("[ BucketMap ] {{ Directories }} {list_dirs:?}");
            while let Some(dir) = list_dirs.pop_front() {
                let (dirs, objs) = dir_objects(&dir).await;
                list_dirs.extend(dirs);
                let key = Key::from_bucket(bks.borrow(), &dir).unwrap();
                let objects = sync_objects(objs, bks.borrow(), key.borrow(), local_storage).await;
                tracing::trace!(
                    "[ BucketMap build ] bucket {bks} - key {key:?} - {objects:?} - path: {dir:?}"
                );
                map.insert(key, objects);
            }
        }
        self.inner = buckets;
        sync_object_with_database(local_storage, self).await;
        Ok(())
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

async fn dir_objects(entry: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut dirs = Vec::new();
    let mut objects = Vec::new();
    tracing::trace!("[ fn dir_objects ] entry {entry:?}");
    let mut reader = entry.read_dir().unwrap();

    while let Some(Ok(path)) = reader.next() {
        if let Some(path) = dir_objects_rename(&path.path()).await {
            if path.is_dir() {
                dirs.push(path);
            } else {
                objects.push(path);
            }
        }
    }
    tracing::trace!(
        "[ fn dir_objects ] {{ directories and objects found }} {dirs:?} - {objects:?}"
    );
    (dirs, objects)
}

async fn dir_objects_rename(path: &Path) -> Option<PathBuf> {
    let des = if path.is_dir() {
        NormalizePathUtf8::default().run(path).await
    } else {
        NormalizeFileUtf8::run(path).await
    }
    .ok()?;

    match des {
        RenameDecision::Yes(Rename {
            mut parent,
            from,
            to,
        }) => {
            let from = parent.join(from);
            parent.push(to);
            if let Err(er) = tokio::fs::rename(from, &parent).await {
                tracing::error!("{er}");
                None
            } else {
                Some(parent)
            }
        }
        RenameDecision::Not(_) => Some(path.to_path_buf()),
        RenameDecision::Fail(error) => {
            tracing::error!("{error:?}");
            None
        }
        _ => unreachable!(),
    }
}
