pub mod error;
pub mod file;
pub mod tree_dir;
use crate::manager::utils::FromDirEntyAsync;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::DirEntry;

#[derive(Debug, Serialize, Deserialize)]
pub struct Directory(String);

impl Directory {
    pub fn path(&self) -> PathBuf {
        PathBuf::from(self.as_ref())
    }
}

impl Directory {
    pub fn inner(self) -> String {
        self.0
    }
}

impl FromDirEntyAsync<DirEntry> for Directory {
    fn from_entry(value: DirEntry) -> impl Future<Output = Self> {
        async move { Self(value.path().to_str().unwrap().to_string()) }
    }
}

impl FromDirEntyAsync<&DirEntry> for Directory {
    fn from_entry(value: &DirEntry) -> impl Future<Output = Self> {
        async move {
            Self(value.path().to_str().unwrap().to_string()) 
        }
    }
}

impl<'a> FromDirEntyAsync<WithPrefixRoot<'a>> for Directory {
    fn from_entry(value: WithPrefixRoot<'a>) -> impl Future<Output = Self> {
        async move {
            let (entry, realpath, prefix) = value.take();
            
            let name = entry.path().canonicalize().ok().and_then(|x| x.to_str().map(ToString::to_string)).unwrap();
            let name = name.replace(realpath, prefix);
            Self(name)
        }
    }
}

impl From<String> for Directory {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Debug)]
pub struct FromEntryToDirErr;

impl std::fmt::Display for FromEntryToDirErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "This is not a directory")
    }
}

impl std::error::Error for FromEntryToDirErr {}

impl AsRef<str> for Directory {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<T> std::cmp::PartialEq<T> for Directory
where
    T: AsRef<str>,
{
    fn eq(&self, other: &T) -> bool {
        self.0.eq(other.as_ref())
    }
}

impl std::cmp::Eq for Directory {}

impl<T> std::cmp::PartialOrd<T> for Directory
where
    T: AsRef<str>,
{
    fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
        self.as_ref().partial_cmp(other.as_ref())
    }
}

impl std::cmp::Ord for Directory {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl std::ops::Deref for Directory {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

pub struct WithPrefixRoot<'a> {
    entry: &'a DirEntry,
    real_path: &'a str,
    root: &'a str,
}

impl<'a> WithPrefixRoot<'a> {
    pub fn new(entry: &'a DirEntry, real_path: &'a str, root: &'a str) -> Self {
        Self {
            entry,
            real_path,
            root,
        }
    }
    pub fn take(self) -> (&'a DirEntry, &'a str, &'a str) {
        (self.entry, self.real_path, self. root)
    }
}