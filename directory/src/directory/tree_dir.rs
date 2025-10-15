use super::{Directory, error::TreeDirErr, file::File};
use crate::manager::utils::FromDirEntyAsync;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
};
use tokio::fs;

type TreeDirType = BTreeMap<Directory, Vec<File>>;

#[derive(Debug, Deserialize, Serialize)]
pub struct TreeDir {
    #[serde(flatten)]
    inner: TreeDirType,

    #[serde(skip_serializing)]
    real_path: String,

    #[serde(skip_serializing)]
    root: String,
}

impl TreeDir {
    pub fn real_path(&self) -> &str {
        &self.real_path
    }

    pub fn root(&self) -> &str {
        self.root.as_ref()
    }

    pub async fn new_async(path: &str) -> Result<Self, TreeDirErr> {
        let path = Self::validate(path).await?;

        let path_buf = PathBuf::from(&path);

        if !path_buf.is_dir() {
            return Err(TreeDirErr::IsNotADirectory(path_buf));
        }

        let mut read_dir = fs::read_dir(&path_buf).await?;

        let mut vec = vec![];
        let mut queue = VecDeque::new();
        let mut resp = BTreeMap::new();
        tracing::info!("Directory: {path}");

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if entry.file_type().await.is_ok_and(|x| x.is_dir()) {
                queue.push_front(Directory::from_entry(&entry).await);
            }
            vec.push(File::from_entry(entry).await);
        }

        let directory = Directory("root".to_string());
        resp.insert(directory, vec);

        while let Some(directory) = queue.pop_front() {
            let path = directory.path();
            let mut read_dir = fs::read_dir(path).await.unwrap();
            let mut vec = vec![];
            while let Ok(Some(entry)) = read_dir.next_entry().await {
                if entry
                    .file_type()
                    .await
                    .map(|x| x.is_dir())
                    .unwrap_or_default()
                {
                    queue.push_back(Directory::from_entry(&entry).await);
                }
                vec.push(File::from_entry(entry).await);
            }
            resp.insert(directory, vec);
        }

        Ok(TreeDir {
            inner: resp,
            real_path: path,
            root: "root".to_string(),
        })
    }

    pub fn get_tree(&self) -> &TreeDirType {
        &self.inner
    }

    async fn validate(path: &str) -> Result<String, TreeDirErr> {
        if path == "/" {
            return Err(TreeDirErr::RootNotAllowed);
        }
        
        let mut path = if path.starts_with("../") || path == "./" {
            fs::canonicalize(path).await?.to_str().unwrap().to_string()
        }  else {
            let re = Regex::new(r"(\./)?(([a-zA-Z0-9]+/?)+)$").unwrap();
            let path = re.replace_all(path, "$2").to_string();
            path
        };

        if !path.ends_with("/") {
            path.push('/');
        };

        if !PathBuf::from(&path).is_dir() {
            return Err(TreeDirErr::IsNotADirectory(PathBuf::from(path)));
        }

        let path = format!("{}/",fs::canonicalize(path).await?.to_str().map(ToString::to_string).unwrap().to_string());
        
        Ok(path)
    }
}

impl std::ops::Deref for TreeDir {
    type Target = TreeDirType;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for TreeDir {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
