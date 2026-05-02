use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    pin::Pin,
};

use futures::{FutureExt, StreamExt, TryStreamExt};
use hyper_tungstenite::HyperWebsocket;
use mongodb::bson::{Document, doc, oid::ObjectId};

use crate::{
    actor::Actor,
    bucket::{
        self, Bucket, Cowed,
        fhs::Fhs,
        key::{Key, Segment},
        object::Object,
        utils::{
            Rename, RenameDecision, list_buckets_and_normalize,
            normalizeds::{NormalizeFileUtf8, NormalizePathUtf8},
        },
    },
    manager::{
        Change,
        websocket::{WebSocketHandler, broker::WSBroker},
    },
    state::local_storage::{AsObjectDeserialize, COLLECTION, LocalStorage},
};

pub struct BucketMap {
    path: PathBuf,
    pub tree: BTreeMap<Bucket<'static>, KeyEntry>,
    broker: <WSBroker as Actor>::ActorRef,
}

pub struct KeyEntry {
    pub objects: Option<Vec<Object>>,
    pub keys: Option<BTreeMap<Segment<'static>, KeyEntry>>,
    pub broker: <WSBroker as Actor>::ActorRef,
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
            broker: WSBroker::default().start(),
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

    pub async fn subscriber(
        &mut self,
        bucket: Option<Bucket<'_>>,
        key: Option<Key<'_>>,
        ws: HyperWebsocket,
    ) {
        match ws.await.map(|x| x.split()) {
            Ok((tx, mut rx)) => {
                let broker = if let Some(bucket) = bucket {
                    let key = key.unwrap_or(Key::root());
                    match self.get_entry(&bucket, &key) {
                        Some(entry) => entry.broker.clone(),
                        None => {
                            tracing::error!(
                                "[ BucketMap ] Subscriber error, entry not found: bucket: {bucket} - key: {key}"
                            );
                            return;
                        }
                    }
                } else {
                    self.broker.clone()
                };

                let actor_ref = WebSocketHandler { user: tx, broker }.start();

                tokio::spawn(async move {
                    loop {
                        match rx.next().await {
                            Some(Ok(msg)) => {
                                tracing::info!(
                                    "[ WebSocketPeer from Subscriber BucketMap ]: {msg}"
                                );
                            }
                            Some(Err(er)) => {
                                tracing::error!(
                                    "[ WebSocketPeer from Subscriber BucketMap ] error {er}"
                                );
                            }
                            None => {
                                actor_ref.shutdown().await;
                                break;
                            }
                        }
                    }
                });
            }
            Err(er) => {
                tracing::error!("[ BucketMap ] subscriber error: {er}");
            }
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
                if let Some((parent, from_seg)) = from
                    .name()
                    .rsplit_once('/')
                    .map(|(key, seg)| (Key::new(key).owned(), Segment::new(seg).owned()))
                {
                    let Some(entry) = self.get_mut_entry(&bucket, &parent) else {
                        tracing::error!("[ Bucket Map ] Key {from} not found");
                        return;
                    };

                    let keys = entry.keys.as_mut().unwrap();

                    if keys.get(&to).is_some() {
                        tracing::error!(
                            "[ BucketMap ] Key {parent}/{to} already exists, i cannot rename the key {from}"
                        );
                        return;
                    }

                    let old = keys.remove(&from_seg);
                    keys.insert(to, old.unwrap_or_default());
                } else {
                    let Some(entry) = self.tree.get_mut(&bucket) else {
                        tracing::error!("[ BucketMap ] Bucket not found {bucket}");
                        return;
                    };

                    let keys = entry.keys.as_mut().unwrap();
                    let from = from.into();
                    if keys.get(&to).is_some() {
                        tracing::error!("[ BucketMap ] {to} already exists");
                        return;
                    };

                    let old = keys.remove(&from);

                    keys.insert(to, old.unwrap_or_default());
                }
            }
            Change::DeleteObject {
                bucket,
                key,
                file_name,
            } => {
                if let Some(entry) = self.get_mut_entry(&bucket, &key) {
                    let Some(objs) = entry.objects.as_mut() else {
                        tracing::error!("[ BucketMap ] Delete Object: I haven't objects in {key}");
                        return;
                    };

                    if let Some(idx) = objs.iter().position(|x| x.file_name == file_name) {
                        objs.swap_remove(idx);
                        tracing::debug!("[ BucketMap ] object {file_name} deleted from key {key}");
                    } else {
                        tracing::error!("[ BucketMap ] object {file_name} not found in key {key}");
                    }
                } else {
                    tracing::error!("[ BucketMap ] The bucket {bucket} with key {key} not found");
                }
            }
            Change::DeleteKey { bucket, key } => {
                if let Some((parent, to_delete)) = key
                    .name()
                    .rsplit_once('/')
                    .map(|(p, s)| (Key::new(p.to_string()), Segment::new(s.to_string())))
                {
                    let Some(entry) = self.get_mut_entry(&bucket, &parent) else {
                        return;
                    };

                    let keys = entry.keys.as_mut().unwrap();
                    if let Some(entry) = keys.remove(&to_delete) {
                        tracing::info!(
                            "[ BucketMap ] from bucket {bucket} delete: {:#?}",
                            Fhs::create_branch(Some((&bucket).into()), &entry)
                        );
                    } else {
                        tracing::error!("[ BucketMap ] Key {key} not found in bucket {bucket}");
                    }
                } else {
                    let Some(entry) = self.tree.get_mut(&bucket) else {
                        return;
                    };
                    let seg = key.into();
                    if entry.keys.as_mut().unwrap().remove(&seg).is_some() {
                        tracing::info!("[ BucketMap ] {seg} deleted from {bucket}");
                    } else {
                        tracing::error!("[ BucketMap ] {seg} not found in bucket {bucket}");
                    }
                }
            }
            Change::DeleteBucket { bucket } => {
                if let Some(bk) = self.tree.remove(&bucket) {
                    tracing::info!(
                        "[ BucketMap ] deleted: {:#?}",
                        Fhs::create_branch(Some(bucket.into()), &bk)
                    );
                } else {
                    tracing::error!("[ BucketMap ] bucket {bucket} not found");
                }
            }
        }
    }

    pub async fn build(&mut self, ls: &LocalStorage) {
        let buckets = list_buckets_and_normalize(&self.path);
        let mut object_ids = Vec::new();
        let mut inner = BTreeMap::new();
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
            resp.push(object.object);
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
        let mut keys = BTreeMap::new();
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

        let objs = sync_objects(
            objects,
            bucket.borrow(),
            Key::from_bucket(bucket.borrow(), path).unwrap(),
            local_storage,
            objects_ids,
        )
        .await;

        KeyEntry {
            objects: (!objs.is_empty()).then_some(objs),
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
        .filter_map(|x| x._id)
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
        let broker = WSBroker::default().start();
        Self {
            objects: None,
            keys: None,
            broker,
        }
    }
}

impl std::fmt::Debug for BucketMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BucketMap")
            .field("broker", &"..")
            .field("path", &self.path)
            .field("tree", &self.tree)
            .finish()
    }
}
