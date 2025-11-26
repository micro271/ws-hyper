use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::bucket::Bucket;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct Key(String);

impl Key {
    pub fn new<K: Into<String>>(inner: K) -> Self {
        Self(inner.into())
    }

    pub fn inner(self) -> String {
        self.0
    }

    pub fn name(&self) -> &str {
        self.as_ref()
    }

    pub fn from_bucket<T: AsRef<Path>>(bucket: &Bucket, path: T) -> Option<Self> {
        let path = path.as_ref().to_str()?;
        let name = bucket.name();
        
        Some(path.split(&format!("{name}/")).nth(1).and_then(|x| x.strip_prefix("/")).map_or(Self("".to_string()),Self::new))
    }
}

impl AsRef<str> for Key {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl std::ops::Deref for Key {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}