use nanoid::nanoid;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::bucket::utils::{RenameDecision, RenameError};
use crate::bucket::{DEFAULT_LENGTH_NANOID, utils::Rename};

static NAME_ALLOWED: LazyLock<Regex> =
    LazyLock::new(|| regex::Regex::new(r"[^a-zA-Z0-9_.]").unwrap());

#[derive(Debug, Default)]
pub struct NormalizePathUtf8 {
    new_path: bool,
}

impl NormalizePathUtf8 {
    pub fn is_new(mut self) -> Self {
        self.new_path = true;
        self
    }

    pub async fn run<T: Into<PathBuf>>(self, to: T) -> Result<RenameDecision, RenameError> {
        let to = to.into();
        let Some(file_name) = to.file_name().map(|x| x.to_string_lossy()) else {
            return Err(RenameError::InvalidPath(to));
        };

        let new_name = NAME_ALLOWED
            .replace_all(file_name.trim_start_matches("-"), "_")
            .into_owned();

        if !new_name.is_empty() {
            if file_name.as_ref() != new_name {
                tracing::trace!(
                    "[ NormalizePathUtf8 ] We going to rename the directory from {file_name} to {new_name}"
                );
                let Some(to_) = to.parent() else {
                    return Err(RenameError::InvalidParent(to));
                };
                return Ok(RenameDecision::Yes(Rename {
                    parent: to_.into(),
                    from: file_name.into_owned(),
                    to: new_name,
                }));
            } else {
                return Ok(RenameDecision::Not(new_name));
            }
        }

        if self.new_path {
            let Some(to_) = to.parent() else {
                return Err(RenameError::InvalidParent(to));
            };
            Ok(RenameDecision::Yes(Rename::new(
                to_.into(),
                file_name.into_owned(),
                format!("INVALID_NAME-{}", nanoid!(DEFAULT_LENGTH_NANOID)),
            )))
        } else {
            Ok(RenameDecision::NeedRestore)
        }
    }
}

#[derive(Debug)]
pub struct NormalizeFileUtf8;

impl NormalizeFileUtf8 {
    pub async fn run(path: &Path) -> Result<RenameDecision, RenameError> {
        let Some(name) = path.file_name().map(|x| x.to_string_lossy()) else {
            return Err(RenameError::InvalidPath(path.into()));
        };

        let new_name = name.trim_start_matches('-').replace("\u{FFFD}", "_");
        let new_name = (!new_name.is_empty())
            .then_some(new_name)
            .unwrap_or_else(|| format!("{}.unknown", nanoid!(24)));

        if name != new_name {
            let Some(path) = path.parent() else {
                return Err(RenameError::InvalidParent(path.into()));
            };
            Ok(RenameDecision::Yes(Rename::new(
                path.into(),
                name.into_owned(),
                new_name,
            )))
        } else {
            Ok(RenameDecision::Not(new_name))
        }
    }
}
