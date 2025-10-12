use super::{Directory, file::File};
use crate::manager::utils::FromDirEntyAsync;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::DirEntry,
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
}

impl TreeDir {
    pub fn real_path(&self) -> &str {
        &self.real_path
    }

    pub async fn new_async(path: PathBuf) -> std::io::Result<Self> {
        let mut read_dir = fs::read_dir(&path).await.unwrap();
        let mut vec = vec![];
        let mut queue = VecDeque::new();
        let mut resp = BTreeMap::new();
        let mut father = None;
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if father.is_none() {
                father = entry
                    .path()
                    .parent()
                    .and_then(|x| x.to_str().map(ToString::to_string));
            }
            if entry.file_type().await.is_ok_and(|x| x.is_dir()) {
                queue.push_front(Directory::from_entry(&entry).await);
            }
            vec.push(File::from_entry(entry).await);
        }

        let directory = Directory(father.clone().unwrap());
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
        let realpath = if ["./", "../", "/"].iter().any(|x| path.starts_with(x)) {
            fs::canonicalize(path)
                .await
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        } else {
            fs::canonicalize(&path).await.unwrap().to_str().unwrap().to_string().strip_suffix(&format!("/{}", path.to_str().unwrap())).unwrap().to_string()
        };
        Ok(TreeDir {
            inner: resp,
            real_path: realpath,
        })
    }
}

impl FromIterator<DirEntry> for TreeDir {
    fn from_iter<T: IntoIterator<Item = DirEntry>>(iter: T) -> Self {
        let mut resp = BTreeMap::new();
        let mut vec = vec![];
        fn visit(entry: DirEntry, resp: &mut TreeDirType) {
            let mut vec = vec![];
            let path = entry.path().clone();
            let directory = Directory::try_from(entry).unwrap();
            let mut dir = std::fs::read_dir(path).unwrap();
            while let Some(Ok(entry)) = dir.next() {
                if entry.file_type().map(|x| x.is_dir()).unwrap_or_default() {
                    visit(entry, resp);
                } else {
                    vec.push(File::try_from(entry).unwrap());
                }
            }
            resp.insert(directory, vec);
        }
        let mut father = None;
        for i in iter {
            if father.is_none() {
                father = i
                    .path()
                    .parent()
                    .and_then(|x| x.to_str())
                    .map(ToString::to_string);
            }
            if i.file_type().map(|x| x.is_dir()).unwrap_or_default() {
                visit(i, &mut resp);
            } else {
                vec.push(File::try_from(i).unwrap());
            }
        }
        let directory = Directory(father.unwrap());
        resp.insert(directory, vec);

        Self {
            inner: resp,
            real_path: String::new(),
        }
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
