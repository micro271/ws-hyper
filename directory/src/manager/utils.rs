use regex::Regex;
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::directory::{Directory, WithPrefixRoot};

pub trait FromDirEntyAsync<T>
where
    Self: Sized + Sync + Send,
{
    fn from_entry(value: T) -> impl Future<Output = Self>;
}

pub struct ValidateError;

pub async fn validate_name_and_replace(path: PathBuf, to: &str) -> Result<(), ValidateError> {
    let re = Regex::new(r"(^\.[^.])|(^\.\.)|(\s+)|(^$)").map_err(|_| ValidateError)?;

    if !path.exists() {
        return Err(ValidateError);
    }

    // TODO: We have to verify if the new file name already exists or not

    if re.is_match(to) {
        tracing::info!("[Validate Task] {{ Auto rename excecuted }} invalid file name: {to:?}");
        let new_to_file_name = re
            .replace_all(to, |caps: &regex::Captures<'_>| {
                if caps.get(1).is_some() {
                    "[DOT]".to_string()
                } else if caps.get(2).is_some() {
                    "[DOT][DOT]".to_string()
                } else if caps.get(3).is_some() {
                    "_".to_string()
                } else if caps.get(4).is_some() {
                    uuid::Uuid::new_v4().to_string()
                } else {
                    caps.get(0).unwrap().as_str().to_string()
                }
            })
            .to_string();

        let mut path_from = PathBuf::from(&path);
        path_from.push(to);
        let mut path_to = PathBuf::from(&path);
        path_to.push(&new_to_file_name);

        tracing::debug!("[Validate Task] Attempt to rename from: {path_from:?} - to: {path_to:?}");

        if let Err(err) = fs::rename(&path_from, &path_to).await {
            tracing::error!(
                "[Validate Task] Auto rename error from: {path_from:?} to: {path_to:?}, error: {err}"
            );
        }
        tracing::warn!("[Validate Task] Auto rename from: {path_from:?} - to: {path_to:?}");
    }
    Ok(())
}

#[derive(Debug, Default)]
pub struct ForDir<T> {
    root: T,
    real_path: T,
}

impl ForDir<String> {
    pub fn new(root: String, real_path: String) -> Self {
        Self {
            real_path,
            root,
        }
    }

    pub fn get(&self) -> ForDir<&str> {
        ForDir { root: &self.root, real_path: &self.real_path }
    }
}

impl ForDir<&str> {
    pub fn directory<T: AsRef<Path>>(&self, path: T) -> Directory {
        Directory::from(WithPrefixRoot::new(path, self.real_path, self.root))
    }
}