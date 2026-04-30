use std::{borrow::Cow, path::Path};

use serde::{Deserialize, Serialize};

use crate::bucket::{Bucket, Cowed};

#[derive(Debug, Deserialize, Serialize, Hash, PartialEq, Eq, Clone)]
pub struct Segment<'a>(Cow<'a, str>);

impl<'a> Segment<'a> {
    pub fn new(segment: impl Into<Cow<'a, str>>) -> Self {
        Self(segment.into())
    }
}

pub struct KeyIter<'a> {
    inner: std::str::Split<'a, char>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key<'a>(Cow<'a, str>);

impl<'a> Key<'a> {
    pub fn root() -> Self {
        Key(".".into())
    }

    pub fn is_root(&self) -> bool {
        self.0 == "."
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    pub fn new<K: Into<Cow<'a, str>>>(inner: K) -> Self {
        Self(inner.into())
    }

    pub fn inner(self) -> String {
        self.0.into_owned()
    }

    pub fn from_bucket(bucket: Bucket<'_>, path: &Path) -> Option<Self> {
        let path = path.to_str()?;
        let name = bucket.name();
        tracing::trace!("[ Key::fn_from_bucket ] path: {path} - name: {name}");
        path.split_once(name)
            .map(|(_, x)| x.strip_prefix("/").unwrap_or(x).to_string())
            .map(|x| Self::new(if x.is_empty() { ".".to_string() } else { x }))
    }
}

impl<'a> std::clone::Clone for Key<'a> {
    fn clone(&self) -> Self {
        self.cloned()
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

impl<'a> Cowed for Key<'a> {
    type Borrow<'b>
        = Key<'b>
    where
        Self: 'b;

    type Owned = Key<'static>;

    fn borrow(&self) -> Self::Borrow<'_> {
        Key(Cow::Borrowed(&self.0))
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

impl<'a> Cowed for Segment<'a> {
    type Borrow<'b>
        = Segment<'b>
    where
        Self: 'b;

    type Owned = Segment<'static>;

    fn borrow(&self) -> Self::Borrow<'_> {
        Segment(Cow::Borrowed(&self.0))
    }

    fn owned(self) -> Self::Owned
    where
        Self: Sized,
    {
        Segment(Cow::Owned(self.0.into_owned()))
    }

    fn cloned(&self) -> Self::Owned {
        Segment(Cow::Owned(self.0.to_string()))
    }
}

impl<'a> IntoIterator for &'a Key<'a> {
    type Item = <KeyIter<'a> as Iterator>::Item;

    type IntoIter = KeyIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        KeyIter {
            inner: self.0.split('/'),
        }
    }
}

impl<'a> std::iter::Iterator for KeyIter<'a> {
    type Item = Segment<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|x| Segment(Cow::Borrowed(x)))
    }
}

impl<'a> From<&'a Bucket<'a>> for Segment<'a> {
    fn from(value: &'a Bucket<'a>) -> Self {
        Self::new(value.name())
    }
}

impl<'a> From<&'a Key<'a>> for Segment<'a> {
    fn from(value: &'a Key<'a>) -> Self {
        Self::new(value.name())
    }
}

impl<'a> std::fmt::Display for Segment<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a> AsRef<str> for Segment<'a> {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'a> From<Segment<'a>> for mongodb::bson::Bson {
    fn from(value: Segment<'a>) -> Self {
        mongodb::bson::to_bson(value.as_ref()).unwrap()
    }
}
