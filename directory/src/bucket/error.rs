use std::path::PathBuf;

#[derive(Debug)]
pub enum BucketMapErr {
    IsNotABucket(PathBuf),
    RootNotAllowed,
    ReadDir(Box<dyn std::error::Error>),
    ReadOnly(PathBuf),
    PermissionDenied(PathBuf),
    RootPathIsNotDirectory(PathBuf),
    RootPathNotFound(PathBuf),
}

impl std::fmt::Display for BucketMapErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IsNotABucket(dir) => write!(f, "{dir:?} isn't a directory"),
            Self::RootNotAllowed => write!(f, "You cannot use the root \"/\" directory"),
            Self::ReadDir(e) => write!(f, "ReadDir Error: {e}"),
            Self::ReadOnly(dir) => write!(f, "The directory {dir:?} is read only"),
            Self::PermissionDenied(dir) => write!(f, "Permission denied: {dir:?}"),
            Self::RootPathIsNotDirectory(path) => write!(f, "Path {path:?} isn't an directory"),
            Self::RootPathNotFound(path) => write!(f, "Path {path:?} not found"),
        }
    }
}

impl std::error::Error for BucketMapErr {}

impl From<std::io::Error> for BucketMapErr {
    fn from(value: std::io::Error) -> Self {
        Self::ReadDir(Box::new(value))
    }
}
