use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    pin::Pin,
};

use futures::{FutureExt, TryStreamExt};
use mongodb::bson::{Document, doc, oid::ObjectId};

use crate::{
    bucket::{
        Bucket, Cowed,
        key::{self, Key, Segment},
        object::Object,
        utils::{
            Rename, RenameDecision, list_buckets_and_normalize,
            normalizeds::{NormalizeFileUtf8, NormalizePathUtf8},
        },
    },
    manager::Change,
    state::local_storage::{AsObjectDeserialize, COLLECTION, LocalStorage},
};

#[derive(Debug)]
pub struct BucketMap {
    path: PathBuf,
    pub tree: HashMap<Bucket<'static>, KeyEntry>,
}

pub struct KeyEntry {
    pub objects: Option<Vec<Object>>,
    pub keys: Option<HashMap<Segment<'static>, KeyEntry>>,
    pub observers: tokio::sync::broadcast::Sender<Change>,
}

impl BucketMap {
    pub fn new<T: Into<PathBuf>>(path: T) -> Self {
        let path = path.into();

        if !path.exists() {
            panic!("{path:?} not found");
        } else if !path.is_dir() {
            panic!("{path:?} isn't directory");
        }

        Self {
            path,
            tree: Default::default(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn get_object<'a>(
        &'a self,
        bucket: &'a Bucket<'_>,
        key: &'a Key<'_>,
        file_name: &'a str,
    ) -> Option<&'a Object> {
        self.get_entry(bucket, key).and_then(|v| {
            v.objects
                .as_ref()
                .and_then(|x| x.iter().find(|x| x.file_name == file_name))
        })
    }

    pub fn get_buckets<'a>(&'a self) -> impl IntoIterator<Item = &'a Bucket<'a>> {
        self.tree.keys()
    }

    pub fn get_entry<'a>(
        &'a self,
        bucket: &'a Bucket<'_>,
        key: &'a Key<'_>,
    ) -> Option<&'a KeyEntry> {
        if key.is_root() {
            self.tree.get(bucket)
        } else {
            key.into_iter()
                .try_fold(self.tree.get(&bucket)?, |entry, x| {
                    entry.keys.as_ref()?.get(&x)
                })
        }
    }

    pub fn get_mut_entry<'a>(
        &'a mut self,
        bucket: &'a Bucket<'static>,
        key: &'a Key<'_>,
    ) -> Option<&'a mut KeyEntry> {
        if key.is_root() {
            self.tree.get_mut(bucket)
        } else {
            let mut entry = self.tree.get_mut(&bucket)?;
            let keys = key.into_iter().map(|x| x.owned()).collect::<Vec<_>>();

            for key in keys {
                entry = entry.keys.as_mut().and_then(|x| x.get_mut(&key))?;
            }

            Some(entry)
        }
    }

    pub async fn change(&mut self, change: Change) {
        match change {
            Change::NewObject {
                bucket,
                key,
                object,
            } => {
                let Some(entry) = self.get_mut_entry(&bucket, &key) else {
                    return;
                };

                entry.objects.get_or_insert_default().push(object);
            }
            Change::NewKey { bucket, key } => {
                let key = key.inner();
                if let Some((key, new_key)) = key
                    .rsplit_once("/")
                    .map(|(k, nk)| (Key::new(k), Segment::new(nk)))
                {
                    if let Some(entry) = self.get_mut_entry(&bucket, &key) {
                        entry
                            .keys
                            .as_mut()
                            .unwrap()
                            .entry(new_key.owned())
                            .or_default();
                    } else {
                        tracing::debug!(
                            "[ BucketMap ] New key, parent key {} not found in bucket {}",
                            key,
                            bucket
                        );
                    }
                } else {
                    self.tree.get_mut(&bucket).map(|x| {
                        x.keys
                            .as_mut()
                            .unwrap()
                            .entry(Segment::new(key))
                            .or_default()
                    });
                }
            }
            Change::NewBucket { bucket } => {
                self.tree.entry(bucket).or_default();
            }
            Change::NameObject {
                bucket,
                key,
                from,
                to,
            } => {
                if let Some(entry) = self.get_mut_entry(&bucket, &key) {
                    if let Some(object) = entry
                        .objects
                        .as_mut()
                        .and_then(|x| x.iter_mut().find(|x| x.file_name == from))
                    {
                        tracing::debug!(
                            "[ BucketMap ] Rename object, from {} to {}, in {}/{}",
                            from,
                            to,
                            bucket,
                            key
                        );
                        object.file_name = to;
                    } else {
                        tracing::debug!(
                            "[ BucketMap ] Rename object, object {} not found, in {}/{}",
                            from,
                            bucket,
                            key
                        );
                    }
                }
            }
            Change::NameBucket { from, to } => {
                if self.tree.get(&to).is_some() {
                    tracing::error!("[ BucketMap ] RenameBucket; bucket {} already exists", to);
                }

                if let Some(entry) = self.tree.remove(&from) {
                    self.tree.insert(to, entry);
                } else {
                    tracing::error!("[ BucketMap ] RenameBucket; bucket {} not found", from);
                }
            }
            Change::NameKey { bucket, from, to } => {
                let from_key = from.inner();
                todo!()
            }
            Change::DeleteObject {
                bucket,
                key,
                file_name,
            } => todo!(),
            Change::DeleteKey { bucket, key } => todo!(),
            Change::DeleteBucket { bucket } => todo!(),
        }
    }

    pub async fn build(&mut self, ls: &LocalStorage) {
        let buckets = list_buckets_and_normalize(&self.path);
        let mut object_ids = Vec::new();
        let mut inner = HashMap::new();
        tracing::info!("[ BucketMap ] Build");
        for (bucket, bucket_path) in buckets {
            let entry = build_key_entry(&bucket_path, &bucket, &mut object_ids, ls).await;
            inner.insert(bucket, entry);
        }
        tracing::debug!("[ BucketMap ] Build: {:#?}", inner);
        self.tree = inner;

        sync_object_with_database(ls, object_ids).await;
    }
}

async fn sync_objects(
    vec: Vec<PathBuf>,
    bucket: Bucket<'_>,
    key: Key<'_>,
    local_storage: &LocalStorage,
    objects_ids: &mut Vec<ObjectId>,
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
            objects_ids.push(object._id.unwrap());
            resp.push(object);
        } else {
            let obj = Object::new(path, Default::default()).await;

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

async fn file_name_normalize(path: PathBuf) -> Option<(PathBuf, String)> {
    let des = if path.is_dir() {
        NormalizePathUtf8::default().is_new().run(&path)
    } else {
        NormalizeFileUtf8::run(&path)
    }
    .ok()?;

    match des {
        RenameDecision::Yes(Rename {
            mut parent,
            from,
            to,
        }) => {
            let from = parent.join(from);
            parent.push(&to);
            if let Err(er) = tokio::fs::rename(from, &parent).await {
                tracing::error!("{er}");
                None
            } else {
                Some((parent, to))
            }
        }
        RenameDecision::Not(file_name) => Some((path, file_name)),
        RenameDecision::Fail(error) => {
            tracing::error!("{error:?}");
            None
        }
        _ => unreachable!(),
    }
}

fn build_key_entry<'a>(
    path: &'a Path,
    bucket: &'a Bucket<'_>,
    objects_ids: &'a mut Vec<ObjectId>,
    local_storage: &'a LocalStorage,
) -> Pin<Box<dyn Future<Output = KeyEntry> + Send + 'a>> {
    async move {
        let mut objects = Vec::new();
        let mut keys = HashMap::new();
        let mut read_dir = path.read_dir().unwrap().into_iter();

        while let Some(entry) = read_dir.next().and_then(|x| x.ok().map(|x| x.path())) {
            let Some((entry, file_name)) = file_name_normalize(entry).await else {
                continue;
            };
            if entry.is_dir() {
                let key_entry = build_key_entry(&entry, bucket, objects_ids, local_storage).await;
                let key = Segment::new(file_name);
                keys.insert(key, key_entry);
            } else {
                objects.push(entry);
            }
        }

        let fut = sync_objects(
            objects,
            bucket.borrow(),
            Key::from_bucket(bucket.borrow(), path).unwrap(),
            local_storage,
            objects_ids,
        )
        .await;

        KeyEntry {
            objects: Some(fut),
            keys: (!keys.is_empty()).then_some(keys),
            ..Default::default()
        }
    }
    .boxed()
}

pub async fn sync_object_with_database(ls: &LocalStorage, objects_ids: Vec<ObjectId>) {
    let pool = ls.pool.default_database().unwrap();

    let objects = pool
        .collection::<AsObjectDeserialize>(COLLECTION)
        .find(doc! {"_id":{"$nin": objects_ids}})
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .expect("[ fn sync_object_with_database ] Failed to get Objects");

    tracing::warn!(
        "[ fn sync_object_with_database ] {} Object dont found in filesystem: {:#?}",
        objects.len(),
        objects
    );

    let objects = objects
        .into_iter()
        .filter_map(|x| x.object._id)
        .collect::<Vec<_>>();

    let delete_result = pool
        .collection::<Document>(COLLECTION)
        .delete_many(doc! {"_id": {"$in": objects}})
        .await
        .expect("[ fn sync_object_with_database ] Failed to delete Objects");
    tracing::warn!(
        "[ fn sync_object_with_database ] {} Objects deleted",
        delete_result.deleted_count
    );
}

impl std::fmt::Debug for KeyEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyEntry")
            .field("objects", &self.objects)
            .field("keys", &self.keys)
            .field("observers", &"...")
            .finish()
    }
}

impl std::default::Default for KeyEntry {
    fn default() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(32);
        Self {
            objects: None,
            keys: None,
            observers: tx,
        }
    }
}
