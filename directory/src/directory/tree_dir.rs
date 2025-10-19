use super::{Directory, error::TreeDirErr, file::File};
use crate::{directory::WithPrefixRoot, manager::utils::FromDirEntyAsync as _};
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

    pub async fn new_async(path: &str, mut prefix_root: String) -> Result<Self, TreeDirErr> {
        let path = Self::validate(path).await?;
        let path_buf = PathBuf::from(&path);

        if !prefix_root.ends_with("/") {
            prefix_root.push('/');
        }

        if !path_buf.is_dir() {
            return Err(TreeDirErr::IsNotADirectory(path_buf));
        }

        let mut read_dir = fs::read_dir(&path_buf).await?;

        let mut vec = vec![];
        let mut queue = VecDeque::new();
        let mut resp = BTreeMap::new();
        tracing::info!("Directory: {path:?}");
        tracing::info!("Root path: {prefix_root:?}");

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if entry.file_type().await.is_ok_and(|x| x.is_dir()) {
                queue.push_front(entry.path());
            }
            vec.push(File::from_entry(entry).await);
            println!("{vec:?}, {queue:?}");
        }

        let directory = Directory(prefix_root.to_string());
        resp.insert(directory, vec);

        while let Some(dir) = queue.pop_front() {
            let mut read_dir = fs::read_dir(&dir).await.unwrap();
            let mut vec = vec![];
            while let Ok(Some(entry)) = read_dir.next_entry().await {
                if entry
                    .file_type()
                    .await
                    .map(|x| x.is_dir())
                    .unwrap_or_default()
                {
                    queue.push_back(entry.path());
                }
                vec.push(File::from_entry(entry).await);
            }
            resp.insert(
                Directory::from(WithPrefixRoot::new(dir, &path, &prefix_root)),
                vec,
            );
        }

        Ok(TreeDir {
            inner: resp,
            real_path: path,
            root: prefix_root,
        })
    }

    pub fn get_tree(&self) -> &TreeDirType {
        &self.inner
    }

    async fn validate(path: &str) -> Result<String, TreeDirErr> {
        if path == "/" {
            return Err(TreeDirErr::RootNotAllowed);
        }

        let path = fs::canonicalize(path).await?;

        if path.metadata().unwrap().permissions().readonly() {
            return Err(TreeDirErr::ReadOnly(path));
        } else if !path.is_dir() {
            return Err(TreeDirErr::IsNotADirectory(path));
        }

        let mut path = path.to_str().unwrap().to_string();

        if !path.ends_with("/") {
            path.push('/');
        };

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
