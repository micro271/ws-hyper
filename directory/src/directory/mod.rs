pub mod error;
pub mod file;
pub mod tree_dir;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, Hash)]
pub struct Directory(String);

impl Directory {
    pub fn all_superpaths(self) -> Vec<Directory> {
        let reg = Regex::new(r"(/.+$)").unwrap();
        let main = &self.0[..];
        let mut resp = Vec::new();
        while let Some(path) = reg.find(&main) {
            todo!()
        }
        resp.push(self);
        resp
    }

    pub fn with_prefix(mut self, prefix: &str) -> Self {
        if self.0.starts_with("/") && prefix.ends_with("/") {
            self.0.insert_str(0,&prefix[..prefix.len() - 1]);
        } else {
            self.0.insert_str(0,prefix);
        }
        self
    }

    pub fn new_unchk_from_path<T>(path: T) -> Self
    where
        T: AsRef<Path>,
    {
        Self(path.as_ref().to_str().map(ToString::to_string).unwrap())
    }

    pub fn path(&self) -> PathBuf {
        PathBuf::from(self.as_ref())
    }

    pub fn inner(self) -> String {
        self.0
    }

    pub fn parent(&self) -> Self {
        let tmp = PathBuf::from(self.as_ref());
        Self(
            tmp.parent()
                .unwrap()
                .to_str()
                .map(ToString::to_string)
                .unwrap(),
        )
    }
}

impl<'a, T> From<WithPrefixRoot<'a, T>> for Directory
where
    T: AsRef<Path>,
{
    fn from(value: WithPrefixRoot<'a, T>) -> Self {
        let (entry, realpath, prefix) = value.take();
        let no_final_slash = &realpath[..realpath.len() - 1];

        let name = entry.as_ref().to_str().map(ToString::to_string).unwrap();

        tracing::trace!("{{From<WithPrefixRoot<'a,T>> for Directory}} entry: {:?} real_path: {} no_final_slah_real_path: {}", entry.as_ref(), realpath, no_final_slash);
        let name = name.replace(
            if name == no_final_slash {
                no_final_slash
            } else {
                realpath
            },
            prefix,
        );
        tracing::trace!("{{From<WithPrefixRoot<'a,T>> for Directory}} From Path {:?} to Directory: {name}", entry.as_ref());
        Self(name)
    }
}

impl std::fmt::Display for Directory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Directory {
    fn from(value: String) -> Self {
        Self(value)
    }
}


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
        self.as_ref()
    }
}

pub struct WithPrefixRoot<'a, T> {
    path: T,
    real_path: &'a str,
    root: &'a str,
}

impl<'a, T> WithPrefixRoot<'a, T>
where
    T: AsRef<Path>,
{
    pub fn new(path: T, real_path: &'a str, root: &'a str) -> Self {
        Self {
            path,
            real_path,
            root,
        }
    }
    pub fn take(self) -> (T, &'a str, &'a str) {
        (self.path, self.real_path, self.root)
    }
}
