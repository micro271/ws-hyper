pub mod bucket_map;
pub mod error;
pub mod key;
pub mod object;
pub mod utils;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::bucket::utils::NormalizeFileUtf8;

#[derive(Debug, Serialize, Deserialize, Clone, Hash)]
pub struct Bucket(String);

impl Bucket {
    pub async fn new_or_rename<T>(path: T) -> Self 
    where 
        T: AsRef<Path>
    {
        let file_name = NormalizeFileUtf8::run(path.as_ref()).await.unwrap();
        Self(file_name)
    }

    pub fn inner(self) -> String {
        self.0
    }

}

impl<T> From<T> for Bucket 
where 
    T: Into<String>
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl std::fmt::Display for Bucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Bucket {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<Path> for Bucket {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
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
        <Self as AsRef<str>>::as_ref(self).partial_cmp(other.as_ref())
    }
}

impl std::cmp::Ord for Bucket {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        <Self as AsRef<str>>::as_ref(self).cmp(other.as_ref())
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
