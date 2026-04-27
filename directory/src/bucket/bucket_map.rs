use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    pin::Pin,
};

use futures::FutureExt;

use crate::{
    bucket::{
        Bucket, Cowed,
        fhs_response::FhsResponse,
        key::Key,
        object::{Object, OwnerFile},
        utils::{
            Rename, RenameDecision, list_buckets_and_normalize,
            normalizeds::{NormalizeFileUtf8, NormalizePathUtf8},
        },
    },
    manager::websocket::observer::UserObserver,
    state::local_storage::LocalStorage,
};

#[derive(Default)]
pub struct BucketMap(HashMap<Bucket<'static>, KeyEntry>);

#[derive(Default)]
pub struct KeyEntry {
    objects: Option<Vec<Object>>,
    keys: Option<HashMap<Key<'static>, KeyEntry>>,
    observers: Option<Vec<UserObserver>>,
}

impl KeyEntry {
    pub fn build<'a>(
        path: &'a Path,
        bucket: &'a Bucket<'_>,
        local_storage: &'a LocalStorage,
    ) -> Pin<Box<dyn Future<Output = Self> + Send + 'a>> {
        async move {
            let mut objects = Vec::new();
            let mut keys = HashMap::new();
            let mut read_dir = path.read_dir().unwrap().into_iter();

            while let Some(entry) = read_dir.next().and_then(|x| x.ok().map(|x| x.path())) {
                if entry.is_dir() {
                    let file_name = match NormalizePathUtf8::default().is_new().run(&entry) {
                        Ok(RenameDecision::Not(name)) => name,
                        Ok(RenameDecision::Fail(er)) => {
                            tracing::error!("[ KeyEntry ] Build error: {er:?}");
                            continue;
                        }
                        Ok(RenameDecision::Yes(Rename {
                            mut parent,
                            from,
                            to,
                        })) => {
                            let from = parent.join(from);
                            parent.push(&to);
                            if let Err(er) = tokio::fs::rename(from, parent).await {
                                tracing::error!("[ KeyEntry ] Build; Rename error {er} ");
                                continue;
                            }
                            to
                        }
                        Err(er) => {
                            tracing::error!("[ KeyEntry ] Build error: {er:?}");
                            continue;
                        }
                        _ => {
                            unreachable!()
                        }
                    };

                    let key_entry = KeyEntry::build(&entry, bucket, local_storage).await;
                    let key = Key::new(file_name);
                    keys.insert(key, key_entry);
                } else {
                    objects.push(entry);
                }
            }

            Self {
                objects: Some(
                    sync_objects(
                        objects,
                        bucket.borrow(),
                        Key::from_bucket(bucket.borrow(), path).unwrap(),
                        local_storage,
                    )
                    .await,
                ),
                keys: (!keys.is_empty()).then_some(keys),
                observers: Default::default(),
            }
        }
        .boxed()
    }
}

impl BucketMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_buckets<'a>(&'a self) -> impl IntoIterator<Item = Bucket<'a>> {
        self.0.keys().into_iter().map(|x| x.borrow())
    }

    pub fn get_entry<'a>(
        &'a self,
        bucket: &'a Bucket<'_>,
        key: &'a Key<'_>,
    ) -> Option<&'a KeyEntry> {
        if key == &Key::root() {
            self.0.get(&bucket)
        } else {
            let mut entry = self.0.get(&bucket)?;

            let keys = key
                .name()
                .split('/')
                .map(|x| Key::new(x))
                .collect::<Vec<_>>();

            for key in keys {
                entry = entry.keys.as_ref().and_then(|x| x.get(&key))?;
            }

            Some(entry)
        }
    }

    pub async fn build(&mut self, path: &Path, ls: &LocalStorage) {
        if !path.exists() {
            panic!("{path:?} not found");
        } else if !path.is_dir() {
            panic!("{path:?} isn't directory");
        }

        let buckets = list_buckets_and_normalize(path);

        let mut inner = HashMap::new();
        for (bucket, bucket_path) in buckets {
            let entry = KeyEntry::build(&bucket_path, &bucket, ls).await;
            inner.insert(bucket, entry);
        }

        self.0 = inner;

        // sync_object_with_database(ls, self).await;
    }
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
            let obj = Object::new(path, OwnerFile::default()).await;
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

async fn dir_objects_rename(path: &Path) -> Option<PathBuf> {
    let des = if path.is_dir() {
        NormalizePathUtf8::default().run(path)
    } else {
        NormalizeFileUtf8::run(path)
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

impl<'a> From<&'a KeyEntry> for FhsResponse<'a> {
    fn from(value: &'a KeyEntry) -> Self {
        let key = value
            .keys
            .as_ref()
            .map(|x| x.keys().map(|x| x.name()).collect::<Vec<_>>());

        Self::new(key.unwrap_or_default(), value.objects.as_ref())
    }
}
