use std::collections::{HashMap, HashSet};

use super::{Bucket, BucketMapType, Cowed, Key};

#[derive(Debug, Default)]
pub struct Skipper<'a> {
    bucket_tracker: Option<HashSet<Bucket<'a>>>,
    key_tracker: Option<HashMap<Bucket<'a>, HashSet<Key<'a>>>>,
    object_tracker: Option<BucketMapType<'a, String>>,
}

pub enum Skip<'a> {
    Bucket {
        bucket: Bucket<'a>,
    },
    Key {
        bucket: Bucket<'a>,
        key: Key<'a>,
    },
    Object {
        bucket: Bucket<'a>,
        key: Key<'a>,
        file_name: String,
    },
}

impl<'a> Skipper<'a> {
    pub fn to_skip(&mut self, skip: Skip<'a>) {
        match skip {
            Skip::Bucket { bucket } => {
                self.bucket_tracker.get_or_insert_default().insert(bucket);
            }
            Skip::Key { bucket, key } => {
                self.key_tracker
                    .get_or_insert_default()
                    .entry(bucket)
                    .or_default()
                    .insert(key);
            }
            Skip::Object {
                bucket,
                key,
                file_name,
            } => {
                let file_name = file_name.into();
                tracing::debug!(
                    "[ SkippedObj ] New skiped: bucker: {bucket} - key: {key} - file_name: {file_name} "
                );
                self.object_tracker
                    .get_or_insert_default()
                    .entry(bucket)
                    .or_default()
                    .entry(key)
                    .or_default()
                    .push(file_name);
            }
        }
    }

    pub fn skipped(&mut self, skip: Skip<'a>) -> bool {
        match skip {
            Skip::Bucket { bucket } => self
                .bucket_tracker
                .as_mut()
                .is_some_and(|x| x.remove(&bucket)),
            Skip::Key { bucket, key } => self
                .key_tracker
                .as_mut()
                .is_some_and(|x| x.get_mut(&bucket.owned()).is_some_and(|x| x.remove(&key))),
            Skip::Object {
                bucket,
                key,
                file_name,
            } => {
                let Some(tree) = self
                    .object_tracker
                    .as_mut()
                    .and_then(|x| x.get_mut(&bucket))
                else {
                    return false;
                };

                if tree
                    .get_mut(&key)
                    .is_some_and(|x| x.pop_if(|x| file_name.eq(x)).is_some())
                {
                    tracing::debug!(
                        "[ SkippedObj ] object skiped found: bucker: {bucket} - key: {key} - file_name: {file_name} "
                    );
                    true
                } else {
                    false
                }
            }
        }
    }
}
