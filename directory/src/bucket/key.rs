use std::{borrow::Cow, path::Path};

use serde::{Deserialize, Serialize};

use crate::bucket::{Bucket, Cowed};

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct Key<'a>(Cow<'a, str>);

impl<'a> Key<'a> {
    pub fn is_parent(&self, child: &Key) -> bool {
        child.name().strip_prefix(self.name()).is_some()
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    pub fn name_mut(&mut self) -> &mut String {
        self.0.to_mut()
    }

    pub fn set_name(&mut self, name: &str) {
        *self.name_mut() = name.to_string();
    }

    pub fn new<K: Into<Cow<'a, str>>>(inner: K) -> Self {
        Self(inner.into())
    }

    pub fn inner(self) -> String {
        self.0.into_owned()
    }

    pub fn from_bucket(bucket: &Bucket, path: &Path) -> Option<Self> {
        let path = path.to_str()?;
        let name = bucket.name();
        tracing::trace!("[ Key::fn_from_bucket ] path: {path} - name: {name}");
        path.split_once(name)
            .map(|(_, x)| x.strip_prefix("/").unwrap_or(x).to_string())
            .map(|x| Self::new(if x.is_empty() { ".".to_string() } else { x }))
    }
}

impl<'a> AsRef<str> for Key<'a> {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl<'a> std::fmt::Display for Key<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a, T: Into<Cow<'a, str>>> From<T> for Key<'a> {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl<'a> From<Key<'a>> for mongodb::bson::Bson {
    fn from(value: Key<'a>) -> Self {
        mongodb::bson::to_bson(value.name()).unwrap()
    }
}

impl<'a> Cowed<'a> for Key<'a> {
    type Borrow = Key<'a>;

    type Owned = Key<'static>;

    fn borrow(&'a self) -> Self::Borrow {
        Self(Cow::Borrowed(&self.0))
    }

    fn owned(self) -> Self::Owned
    where
        Self: Sized,
    {
        Key(Cow::Owned(self.0.into_owned()))
    }

    fn cloned(&self) -> Self::Owned {
        Key(Cow::Owned(self.0.to_string()))
    }
}
