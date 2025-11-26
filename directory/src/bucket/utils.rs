use std::path::{Path, PathBuf};

use nanoid::nanoid;

use crate::bucket::Bucket;


#[derive(Debug)]
pub struct NormalizeFileUtf8;

impl NormalizeFileUtf8 {
    pub async fn run<'a>(path: &'a Path) -> Option<String> {
        if let Some(str) = path.file_name().and_then(|x| x.to_str()) {
            Some(str.to_string())
        } else {
            let mut to = PathBuf::from(path);
            to.pop();
            
            let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("__unknown");
            let new_name = format!("{}.{ext}", nanoid!(24));
            to.push(&new_name);
            if let Err(er) = tokio::fs::rename(path, &to).await {
                tracing::error!("From NormalizeFileUtf - Error to rename file from: {path:?} - to: {to:?}");
                tracing::error!("IoError: {er}");
                return None;
            }
            Some(new_name)
        }
    }
}


pub fn find_bucket(root: &Path, path: &Path) -> Option<Bucket> {
    let child = path;
    while let Some(parent) = child.parent() {
        if parent == root {
            return Some(Bucket::from(child.file_name().and_then(|x| x.to_str()).unwrap()));
        }
    }
    None
}