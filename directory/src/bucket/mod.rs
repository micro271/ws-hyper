pub mod error;
pub mod key;
pub mod bucket_map;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, Hash)]
pub struct Bucket(String);

impl Bucket {
    pub fn all_superpaths(self) -> Vec<Bucket> {
        let reg = Regex::new(r"/([^/]+)").unwrap();
        let path = self.as_ref();
        tracing::trace!("{path:?}");
        let mut resp = Vec::new();

        for mt in reg.find_iter(path) {
            let aux = &path[..mt.start()];
            tracing::trace!("[All superpaths] match: {mt:?}");
            if !aux.contains('/') {
                resp.push(Bucket::new_unchk(format!("{aux}/")));
            } else {
                resp.push(Bucket::new_unchk(aux.to_string()));
            }
        }
        resp.push(self);
        tracing::trace!("[All superpaths] result {resp:?}");
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

    pub fn new_unchk(path: String) -> Self {
        Self(path)
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

impl<'a, T> From<WithPrefixRoot<'a, T>> for Bucket
where
    T: AsRef<Path>,
{
    fn from(value: WithPrefixRoot<'a, T>) -> Self {
        let (entry, realpath, prefix) = value.take();
        let no_final_slash = &realpath[..realpath.len() - 1];

        let name = entry.as_ref().to_str().map(ToString::to_string).unwrap();

        tracing::trace!("{{From<WithPrefixRoot<'a,T>> for Bucket}} entry: {:?} real_path: {} no_final_slah_real_path: {}", entry.as_ref(), realpath, no_final_slash);
        let name = name.replace(
            if name == no_final_slash {
                no_final_slash
            } else {
                realpath
            },
            prefix,
        );
        tracing::trace!("{{From<WithPrefixRoot<'a,T>> for Bucket}} From Path {:?} to Bucket: {name}", entry.as_ref());
        Self(name)
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
