use crate::bucket::{Bucket, key::Key};
use serde::Serialize;
use std::collections::HashMap;
use tokio::sync::broadcast::{Receiver, Sender, channel};

pub struct UserTracker<T> {
    inner: HashMap<Bucket<'static>, HashMap<Key<'static>, Sender<T>>>,
}

impl<T> UserTracker<T>
where
    T: Serialize + Clone,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_rx(&mut self, bucket: Bucket<'static>, key: Key<'static>) -> Receiver<T> {
        let bk = self.inner.entry(bucket).or_default();

        if let Some(sender) = bk.get_mut(&key) {
            sender.subscribe()
        } else {
            let (tx, rx) = channel::<T>(124);
            bk.insert(key, tx);
            rx
        }
    }

    pub fn broadcast(&self, bucket: Bucket<'static>, key: Option<Key<'static>>, value: T) {
        match key {
            Some(key) => {
                if let Some(sender) = self.inner.get(&bucket).and_then(|bk| bk.get(&key)) {
                    let _ = sender.send(value);
                }
            }
            None => {
                // NameBucket / DeleteBucket — notificar a todas las keys del bucket
                if let Some(bk) = self.inner.get(&bucket) {
                    for sender in bk.values() {
                        let _ = sender.send(value.clone());
                    }
                }
            }
        }
    }
}

impl<T> std::default::Default for UserTracker<T> {
    fn default() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }
}
