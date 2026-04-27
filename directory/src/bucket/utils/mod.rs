pub mod normalizeds;

use std::path::{Path, PathBuf};

use crate::bucket::{Bucket, utils::normalizeds::NormalizePathUtf8};

#[derive(Debug)]
pub struct Rename {
    pub parent: PathBuf,
    pub from: String,
    pub to: String,
}

impl Rename {
    pub fn new(path: PathBuf, from: String, to: String) -> Self {
        Self {
            parent: path,
            from,
            to,
        }
    }
}

#[derive(Debug)]
pub enum RenameDecision {
    NeedRestore,
    Yes(Rename),
    Not(String),
    Fail(Box<dyn std::error::Error + Send + 'static>),
}

pub trait Changed<Rhs = Self> {
    fn change(&self, other: &Rhs) -> bool;
}

impl<T, K: PartialEq<T>> Changed<T> for K {
    fn change(&self, other: &T) -> bool {
        self.ne(other)
    }
}

#[derive(Debug)]
pub enum RenameError {
    InvalidPath(PathBuf),
    InvalidParent(PathBuf),
}

pub fn list_buckets_and_normalize(root: &Path) -> Vec<(Bucket<'static>, PathBuf)> {
    let mut resp = Vec::new();
    for bucket in root.read_dir().unwrap().flatten() {
        let path = bucket.path();
        match NormalizePathUtf8::default().is_new().run(&path) {
            Ok(RenameDecision::Not(bk)) => resp.push((Bucket::new_unchecked(bk), path)),
            Ok(RenameDecision::Yes(Rename {
                mut parent,
                from,
                to,
            })) => {
                let from = parent.join(from);
                parent.push(&to);
                if let Err(er) = std::fs::rename(from, parent) {
                    tracing::error!(
                        "[ BucketMap build ] fn list_buckets_and_normalize error: {er:?}"
                    )
                }
                resp.push((Bucket::new_unchecked(to), path));
            }
            Err(er) => {
                tracing::error!("[ BucketMap build ] fn list_buckets_and_normalize error: {er:?}");
            }
            _ => todo!(),
        }
    }

    resp
}
