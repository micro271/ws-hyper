use std::collections::HashMap;

use hyper_tungstenite::tungstenite;

use crate::{
    bucket::{Bucket, Cowed, key::Key, object::Object},
    manager::websocket::observer::Observer,
};

#[derive(Default)]
pub struct BucketMap(HashMap<Bucket<'static>, KeyEntry>);

#[derive(Default)]
pub struct KeyEntry {
    objects: Option<Vec<Object>>,
    keys: Option<HashMap<Key<'static>, KeyEntry>>,
    observers: Option<Vec<Box<dyn Observer<Event = tungstenite::Message>>>>,
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
}
