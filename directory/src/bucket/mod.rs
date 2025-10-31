pub mod bucket_map;
pub mod error;
pub mod key;
pub mod object;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, Hash)]
pub struct Bucket(String);

impl Bucket {

    pub fn new_unchk_from_path<T>(path: T) -> Self
    where
        T: AsRef<Path>,
    {
        Self(path.as_ref().to_str().map(ToString::to_string).unwrap())
    }

    pub fn new_unchk<P: Into<String>>(path: P) -> Self {
        Self(path.into())
    }

    pub fn path(&self) -> PathBuf {
        PathBuf::from(self.as_ref())
    }

    pub fn inner(self) -> String {
        self.0
    }
}

impl std::fmt::Display for Bucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Bucket {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for Bucket {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<T> std::cmp::PartialEq<T> for Bucket
where
    T: AsRef<str>,
{
    fn eq(&self, other: &T) -> bool {
        self.0.eq(other.as_ref())
    }
}

impl std::cmp::Eq for Bucket {}

impl<T> std::cmp::PartialOrd<T> for Bucket
where
    T: AsRef<str>,
{
    fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
        self.as_ref().partial_cmp(other.as_ref())
    }
}

impl std::cmp::Ord for Bucket {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl std::ops::Deref for Bucket {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl std::ops::DerefMut for Bucket {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}