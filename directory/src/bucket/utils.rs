use std::path::{Path, PathBuf};

use nanoid::nanoid;

use crate::bucket::{Bucket, object::EXTENSION_OBJECT};

pub struct NormalizeForObjectName;

impl NormalizeForObjectName {
    pub async fn run<'a>(path: &'a Path) -> String {
        let mut to = PathBuf::from(path);
        to.pop();
        let new_name = format!("{}.{EXTENSION_OBJECT}", nanoid!(24));
        to.push(&new_name);

        if let Err(er) = tokio::fs::rename(path, &to).await {
            tracing::error!("From NormalizeFileUtf - Error to rename file from: {path:?} - to: {to:?}");
            tracing::error!("IoError: {er}");
        }

        tracing::warn!("[NormalizeFileUtf] {{ Rename file }} from: {path:?} to: {to:?}");
        new_name
    }    
}


#[derive(Debug)]
pub struct FileNameUtf8;

impl FileNameUtf8 {
    pub async fn run<'a>(path: &'a Path) -> Renamed {
        if let Some(str) = path.file_name().and_then(|x| x.to_str()) {
            Renamed::Not(str.to_string())
        } else {
            let mut to = PathBuf::from(path);
            to.pop();
            
            let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("__unknown");
            let new_name = format!("{}.{ext}", nanoid!(24));
            to.push(&new_name);
            if let Err(er) = tokio::fs::rename(path, &to).await {
                tracing::error!("From NormalizeFileUtf - Error to rename file from: {path:?} - to: {to:?}");
                tracing::error!("IoError: {er}");
                return Renamed::Fail(Box::new(er));
            }

            tracing::warn!("[NormalizeFileUtf] {{ Rename file }} from: {path:?} to: {to:?}");
            Renamed::Yes(new_name)
        }
    }
}


pub fn find_bucket(root: &Path, path: &Path) -> Option<Bucket> {
    let mut child = path;
    while let Some(parent) = child.parent() {
        if parent == root {
            return Some(Bucket::from(child.file_name().and_then(|x| x.to_str()).unwrap()));
        } 
        child = parent;
    }
    None
}

pub enum Renamed {
    Yes(String),
    Not(String),
    Fail(Box<dyn std::error::Error>),
}

impl Renamed {
    pub fn ok(self) -> Option<String> {
        match self {
            Renamed::Yes(str) | Renamed::Not(str)=> Some(str),
            Renamed::Fail(_) => None,
        }
    }
}