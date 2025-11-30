use nanoid::nanoid;
use regex::Regex;
use std::path::{Path, PathBuf};

use crate::bucket::{DEFAULT_LENGTH_NANOID, utils::Renamed};

#[derive(Debug, Default)]
pub struct NormalizePathUtf8 {
    new_path: bool,
}

impl NormalizePathUtf8 {
    pub fn is_new(mut self) -> Self {
        self.new_path = true;
        self
    }

    pub async fn run(self, to: &Path) -> Renamed {
        let renamed = if let Some(str) = to.file_name().and_then(|x| x.to_str()) {
            let allowed_name = Regex::new(r"^[a-zA-Z0-9_@][A-Za-z0-9:@_-]+$").unwrap();

            if allowed_name.is_match(str) {
                tracing::trace!("[ NormalizePathUtf8 ] The directory {to:?} is ok");
                Renamed::Not(str.to_string())
            } else if self.new_path {
                tracing::trace!("[ NormalizePathUtf8 ] The directory {to:?} is not ok");
                let reg = Regex::new(r"([^a-zA-Z0-9-_:@])|(^-)").unwrap();
                let new_name = reg.replace_all(str, "_").into_owned();
                tracing::warn!(
                    "[ NormalizePathUtf8 ] {{ Invalid path in new path }} The directory {to:?} is renamed to {new_name}"
                );
                Renamed::Yes(new_name)
            } else {
                Renamed::NeedRestore
            }
        } else {
            Renamed::Yes(format!("INVALID_NAME-{}", nanoid!(DEFAULT_LENGTH_NANOID)))
        };

        if let Renamed::Yes(name) = &renamed {
            let mut to_new = PathBuf::from(to);
            to_new.pop();

            to_new.push(name);
            if let Err(er) = tokio::fs::rename(to, &to_new).await {
                tracing::error!(
                    "[ NormalizeKeyUtf8 ] - Error to rename file from: {to:?} - to: {to_new:?}"
                );
                tracing::error!("IoError: {er}");
                return Renamed::Fail(Box::new(er));
            }
            tracing::warn!("[ NormalizePathUtf8 ] {{ Rename file }} from: {to:?} to: {to_new:?}");
        }

        renamed
    }
}

#[derive(Debug)]
pub struct NormalizeFileUtf8;

impl NormalizeFileUtf8 {
    pub async fn run(path: &Path) -> Renamed {
        if let Some(str) = path.file_name().and_then(|x| x.to_str()) {
            Renamed::Not(str.to_string())
        } else {
            let mut to = PathBuf::from(path);
            to.pop();

            let ext = path
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or("__unknown");
            let new_name = format!("{}.{ext}", nanoid!(24));
            to.push(&new_name);
            if let Err(er) = tokio::fs::rename(path, &to).await {
                tracing::error!(
                    "From NormalizeFileUtf - Error to rename file from: {path:?} - to: {to:?}"
                );
                tracing::error!("IoError: {er}");
                return Renamed::Fail(Box::new(er));
            }

            tracing::warn!("[NormalizeFileUtf] {{ Rename file }} from: {path:?} to: {to:?}");
            Renamed::Yes(new_name)
        }
    }
}
