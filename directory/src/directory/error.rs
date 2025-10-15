use std::path::PathBuf;

#[derive(Debug)]
pub enum TreeDirErr {
    IsNotADirectory(PathBuf),
    RootNotAllowed,
    ReadDir(Box<dyn std::error::Error>),
}

impl std::fmt::Display for TreeDirErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IsNotADirectory(dir) => write!(f, "{dir:?} isn't a directory"),
            Self::RootNotAllowed => write!(f, "You cannot use the root \"/\" directory"),
            Self::ReadDir(e) => write!(f, "ReadDir Error: {e}"),
        }
    }
}

impl std::error::Error for TreeDirErr {}

impl From<std::io::Error> for TreeDirErr {
    fn from(value: std::io::Error) -> Self {
        Self::ReadDir(Box::new(value))
    }
}
