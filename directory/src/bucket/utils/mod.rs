pub mod normalizeds;
pub mod rename_handlers;

use nanoid::nanoid;
use std::path::{Path, PathBuf};

use crate::{
    bucket::{
        Bucket, DEFAULT_LENGTH_NANOID,
        key::Key,
        object::{EXTENSION_OBJECT, Object},
    },
    state::local_storage::{LocalStorage, error::LsError},
};

pub struct NormalizeForObjectName;

impl NormalizeForObjectName {
    pub async fn run(path: &Path) -> String {
        let mut to = PathBuf::from(path);
        to.pop();
        let new_name = format!("data_{}.{EXTENSION_OBJECT}", nanoid!(DEFAULT_LENGTH_NANOID));
        to.push(&new_name);

        if let Err(er) = tokio::fs::rename(path, &to).await {
            tracing::error!(
                "From NormalizeFileUtf - Error to rename file from: {path:?} - to: {to:?}"
            );
            tracing::error!("IoError: {er}");
        }

        tracing::warn!("[NormalizeFileUtf] {{ Rename file }} from: {path:?} to: {to:?}");
        new_name
    }
}

#[derive(Debug)]
pub enum Renamed {
    Yes(String),
    NeedRestore,
    Not(String),
    Fail(Box<dyn std::error::Error + Send + 'static>),
}

impl Renamed {
    pub fn ok(self) -> Option<String> {
        match self {
            Renamed::Yes(str) | Renamed::Not(str) => Some(str),
            Renamed::Fail(_) | Self::NeedRestore => None,
        }
    }
}
