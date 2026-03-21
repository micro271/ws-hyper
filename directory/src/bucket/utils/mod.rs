pub mod normalizeds;
pub mod rename_handlers;

use std::path::PathBuf;

use crate::{
    bucket::{Bucket, key::Key, object::Object},
    state::local_storage::{LocalStorage, error::LsError},
};

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
