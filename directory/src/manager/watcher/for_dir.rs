use std::path::{Path, PathBuf};

use crate::directory::{Directory, WithPrefixRoot, file::File};

#[derive(Debug, Default)]
pub struct ForDir<T> {
    root: T,
    real_path: T,
}

impl ForDir<String> {
    pub fn new(root: String, real_path: String) -> Self {
        tracing::trace!("[ForDir] root: {root:?} real_path: {real_path:?}");

        Self { real_path, root }
    }

    pub fn get(&self) -> ForDir<&str> {
        ForDir {
            root: &self.root,
            real_path: &self.real_path,
        }
    }
}

impl ForDir<&str> {
    pub fn directory<T: AsRef<Path>>(&self, path: T) -> Result<Directory, ForDirErr> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(ForDirErr::FileNotFound(path.to_path_buf()));
        }

        Ok(Directory::from(WithPrefixRoot::new(
            path,
            self.real_path,
            self.root,
        )))
    }

    pub fn dir_and_file<T: AsRef<Path>>(&self, path: T) -> Result<(Directory, File), ForDirErr> {
        let path = path.as_ref();

        let parent = path
            .parent()
            .ok_or(ForDirErr::ParentNotFound(path.to_path_buf()))?;

        Ok((self.directory(parent)?, File::from(path)))
    }
}

#[derive(Debug)]
pub enum ForDirErr {
    ParentNotFound(PathBuf),
    FileNotFound(PathBuf),
}

impl std::fmt::Display for ForDirErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForDirErr::ParentNotFound(path_buf) => write!(f, " Parent {path_buf:?} not found"),
            ForDirErr::FileNotFound(path_buf) => write!(f, "File {path_buf:?} not found"),
        }
    }
}

impl std::error::Error for ForDirErr {}
