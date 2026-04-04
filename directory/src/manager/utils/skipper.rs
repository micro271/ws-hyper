use std::collections::{BTreeMap, HashMap, HashSet};

use crate::bucket::Cowed;

use super::{Bucket, Key};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Default, Clone)]
pub struct Skipper(Arc<InnerSkipper>);

impl Skipper {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn bucket_tracker(&self) -> &BucketTracker {
        &self.0.bucket_tracker
    }
    pub fn key_tracker(&self) -> &KeyTracker {
        &self.0.key_tracker
    }
    pub fn object_tracker(&self) -> &ObjectTracker {
        &self.0.object_tracker
    }
}

#[derive(Debug, Default)]
pub struct InnerSkipper {
    bucket_tracker: BucketTracker,
    key_tracker: KeyTracker,
    object_tracker: ObjectTracker,
}

#[derive(Debug, Default)]
pub struct BucketTracker(Mutex<HashSet<Bucket<'static>>>);

impl BucketTracker {
    pub async fn to_skip<'a>(&self, bucket: Bucket<'a>) -> bool {
        self.0.lock().await.insert(bucket.owned())
    }

    pub async fn skipped<'a>(&self, bucket: &Bucket<'static>) -> bool {
        self.0.lock().await.remove(bucket)
    }
}

#[derive(Debug, Default)]
pub struct KeyTracker(Mutex<HashMap<Bucket<'static>, HashSet<Key<'static>>>>);

impl KeyTracker {
    pub async fn to_skip(&self, bucket: Bucket<'_>, key: Key<'_>) -> bool {
        self.0
            .lock()
            .await
            .entry(bucket.owned())
            .or_default()
            .insert(key.owned())
    }

    pub async fn skipped(&self, bucket: &Bucket<'static>, key: &Key<'static>) -> bool {
        self.0
            .lock()
            .await
            .get_mut(bucket)
            .map(|x| x.remove(key))
            .unwrap_or_default()
    }
}

#[derive(Debug, Default)]
pub struct ObjectTracker(Mutex<HashMap<Bucket<'static>, BTreeMap<Key<'static>, HashSet<String>>>>);

impl ObjectTracker {
    pub async fn to_skip(
        &self,
        bucket: Bucket<'_>,
        key: Key<'_>,
        file_name: impl Into<String>,
    ) -> bool {
        self.0
            .lock()
            .await
            .entry(bucket.owned())
            .or_default()
            .entry(key.owned())
            .or_default()
            .insert(file_name.into())
    }

    pub async fn skipped(
        &self,
        bucket: &Bucket<'static>,
        key: &Key<'static>,
        file_name: &str,
    ) -> bool {
        self.0
            .lock()
            .await
            .get_mut(bucket)
            .and_then(|x| x.get_mut(key).map(|x| x.remove(file_name)))
            .unwrap_or_default()
    }
}
