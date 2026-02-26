pub mod bucket_map;
pub mod error;
pub mod key;
pub mod object;
pub mod utils;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, ffi::OsStr, path::Path};

pub const DEFAULT_LENGTH_NANOID: usize = 21;

#[derive(Debug, Serialize, Deserialize, Clone, Hash)]
pub struct Bucket<'a>(Cow<'a, str>);

impl<'a> Bucket<'a> {
    pub fn new<T: AsRef<Path>>(path: &T) -> Option<Bucket<'_>> {
        path.as_ref()
            .file_name()
            .and_then(|x| x.to_str().map(ToString::to_string))
            .map(|x| Bucket(Cow::Owned(x)))
    }

    pub fn new_random(ext: Option<&OsStr>) -> Self {
        let ext = ext.and_then(|x| x.to_str()).unwrap_or("__unknown");
        Self::new_unchecked(format!("{}.{ext}", nanoid!(DEFAULT_LENGTH_NANOID)))
    }

    pub fn new_unchecked<T: Into<Cow<'a, str>>>(name: T) -> Self {
        Self(name.into())
    }

    pub fn into_inner(self) -> String {
        self.0.into_owned()
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    pub fn name_mut(&mut self) -> &mut String {
        self.0.to_mut()
    }

    pub fn find_bucket(root: &Path, path: &Path) -> Option<Bucket<'static>> {
        let mut child = path;
        while let Some(parent) = child.parent() {
            if parent == root {
                return Some(
                    Bucket::new_unchecked(child.file_name().and_then(|x| x.to_str()).unwrap())
                        .owned(),
                );
            }
            child = parent;
        }
        None
    }
}

impl<'a> std::fmt::Display for Bucket<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a> AsRef<str> for Bucket<'a> {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'a, T> std::cmp::PartialEq<T> for Bucket<'a>
where
    T: AsRef<str>,
{
    fn eq(&self, other: &T) -> bool {
        self.0.eq(other.as_ref())
    }
}

impl<'a> std::cmp::Eq for Bucket<'a> {}

impl<'a, T> std::cmp::PartialOrd<T> for Bucket<'a>
where
    T: AsRef<str>,
{
    fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
        <Self as AsRef<str>>::as_ref(self).partial_cmp(other.as_ref())
    }
}

impl<'a> std::cmp::Ord for Bucket<'a> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        <Self as AsRef<str>>::as_ref(self).cmp(other.as_ref())
    }
}

impl<'a> From<Bucket<'a>> for mongodb::bson::Bson {
    fn from(value: Bucket<'a>) -> Self {
        mongodb::bson::to_bson(value.name()).unwrap()
    }
}

impl<'a> Cowed for Bucket<'a> {
    type Borrow<'b>
        = Bucket<'b>
    where
        Self: 'b;

    type Owned = Bucket<'static>;

    fn borrow(&self) -> Self::Borrow<'_> {
        Bucket(Cow::Borrowed(&self.0))
    }

    fn owned(self) -> Self::Owned
    where
        Self: Sized,
    {
        Bucket(Cow::Owned(self.0.into_owned()))
    }

    fn cloned(&self) -> Self::Owned {
        Bucket(Cow::Owned(self.0.to_string()))
    }
}

pub trait Cowed {
    type Borrow<'a>
    where
        Self: 'a;
    type Owned: 'static;

    fn borrow(&self) -> Self::Borrow<'_>;

    fn owned(self) -> Self::Owned
    where
        Self: Sized;

    fn cloned(&self) -> Self::Owned;
}
