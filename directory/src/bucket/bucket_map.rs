use std::{
    borrow::Cow,
    collections::HashMap,
    path::{Path, PathBuf},
    pin::Pin,
};

use futures::{FutureExt, TryStreamExt};
use mongodb::bson::{Document, doc, oid::ObjectId};

use crate::{
    bucket::{
        Bucket, Cowed,
        key::Key,
        object::Object,
        utils::{
            Rename, RenameDecision, list_buckets_and_normalize,
            normalizeds::{NormalizeFileUtf8, NormalizePathUtf8},
        },
    },
    manager::{Change, websocket::observer::UserObserver},
    state::local_storage::{AsObjectDeserialize, COLLECTION, LocalStorage},
};

pub struct AbsoluteKey<'a>(pub Cow<'a, str>);

#[derive(Debug)]
pub struct BucketMap {
    path: PathBuf,
    tree: HashMap<Bucket<'static>, KeyEntry>,
}

#[derive(Default, Debug)]
pub struct KeyEntry {
    pub objects: Option<Vec<Object>>,
    pub keys: Option<HashMap<Key<'static>, KeyEntry>>,
    pub observers: Option<Vec<UserObserver>>,
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
        key: &'a AbsoluteKey<'_>,
        file_name: &'a str,
    ) -> Option<&'a Object> {
        self.get_entry(bucket, key).and_then(|(_, v)| {
            v.objects
                .as_ref()
                .and_then(|x| x.iter().find(|x| x.file_name == file_name))
        })
    }

    pub fn get_buckets<'a>(&'a self) -> impl IntoIterator<Item = &'a Bucket<'a>> {
        self.tree.keys().into_iter()
    }

    pub fn get_entry<'a>(
        &'a self,
        bucket: &'a Bucket<'_>,
        key: &'a AbsoluteKey<'_>,
    ) -> Option<(Key<'a>, &'a KeyEntry)> {
        if key.0 == "." {
            self.tree.get(bucket).map(|x| (Key::new("/"), x))
        } else {
            let mut entry = self.tree.get(&bucket)?;
            let keys = key.0.split('/').map(|x| Key::new(x)).collect::<Vec<_>>();

            let last = keys.last().map(|x| x.cloned());

            for key in keys {
                entry = entry.keys.as_ref().and_then(|x| x.get(&key))?;
            }

            Some((last.unwrap(), entry))
        }
    }

    pub async fn change(&mut self, change: Change) {
        todo!()
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
                let key = Key::new(file_name);
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
            observers: Default::default(),
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
