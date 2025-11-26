use std::{borrow::Cow, path::{Path, PathBuf}};

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

    pub fn name(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.0.as_ref())
    }

    pub fn from_bucket<T: AsRef<Path>>(bucket: &Bucket, path: T) -> Option<Self> {
        let path = path.as_ref().to_str()?;
        let name: &str = bucket.as_ref();
        path.split(&format!("{name}/")).nth(1).map(Self::new)
    }
}

impl AsRef<str> for Key {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
