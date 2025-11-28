use regex::Regex;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::{fs, sync::mpsc::error::SendError};

use crate::{
    bucket::{
        Bucket,
        key::Key,
        object::Object,
        utils::{FileNameUtf8, Renamed, find_bucket},
    },
    manager::Change,
    state::local_storage::LocalStorage,
};

pub type SenderErrorTokio<T> = Result<(), tokio::sync::mpsc::error::SendError<T>>;

pub trait AsyncRecv: Send {
    type Item;

    fn recv(&mut self) -> impl Future<Output = Option<Self::Item>> + Send;
}

pub trait AsyncSender: Send + 'static {
    type Item;

    fn send(
        &mut self,
        item: Self::Item,
    ) -> impl Future<Output = Result<(), SendError<Self::Item>>> + Send;
}

pub trait OneshotSender: Send + 'static {
    type Item;

    fn send(&self, item: Self::Item) -> Result<(), SendError<Self::Item>>;
}

pub trait TakeOwn<T: Send + 'static> {
    fn take(self) -> T;
}

#[derive(Debug)]
pub enum ValidateError {
    RegexError(Box<dyn std::error::Error>),
    PathNotExist(PathBuf),
}

pub async fn validate_name_and_replace(path: PathBuf, to: &str) -> Result<(), ValidateError> {
    let re =
        Regex::new(r"(^\.*)|(\s+)|(^$)").map_err(|x| ValidateError::RegexError(Box::new(x)))?;

    if !path.exists() {
        return Err(ValidateError::PathNotExist(path));
    }

    // TODO: We have to verify if the new file name already exists or not

    if re.is_match(to) {
        tracing::info!("[Validate Task] {{ Auto rename excecuted }} invalid file name: {to:?}");
        let new_to_file_name = re
            .replace_all(to, |caps: &regex::Captures<'_>| {
                if let Some(txt) = caps.get(1) {
                    let mut resp = txt.as_str().replace(".", "[DOT]").to_string();
                    resp.insert_str(0, nanoid::nanoid!(4).as_str());
                    resp
                } else if caps.get(2).is_some() {
                    "_".to_string()
                } else if caps.get(3).is_some() {
                    uuid::Uuid::new_v4().to_string()
                } else {
                    caps.get(0).unwrap().as_str().to_string()
                }
            })
            .to_string();

        let mut path_from = PathBuf::from(&path);
        path_from.push(to);
        let mut path_to = PathBuf::from(&path);
        path_to.push(&new_to_file_name);

        tracing::debug!("[Validate Task] Attempt to rename from: {path_from:?} - to: {path_to:?}");

        if let Err(err) = fs::rename(&path_from, &path_to).await {
            tracing::error!(
                "[Validate Task] Auto rename error from: {path_from:?} to: {path_to:?}, error: {err}"
            );
        }
        tracing::warn!("[Validate Task] Auto rename from: {path_from:?} - to: {path_to:?}");
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Pending<W: Send + 'static>(W);

impl<W: Send + 'static> Pending<W> {
    pub fn new(inner: W) -> Self {
        Self(inner)
    }
}

#[derive(Debug, Clone)]
pub struct Executing;

impl<W: Send + 'static> TakeOwn<W> for Pending<W> {
    fn take(self) -> W {
        self.0
    }
}

pub trait Task {
    fn task(self) -> impl Future<Output = ()> + Send + 'static
    where
        Self: Sized;
}

pub trait Run {
    fn run(self)
    where
        Self: Sized;
    fn executor(self) -> impl Run
    where
        Self: Sized;
}

impl<T: Task> Run for T {
    fn run(self)
    where
        Self: Sized,
    {
        tokio::spawn(self.task());
    }

    fn executor(self) -> impl Run
    where
        Self: Sized,
    {
        self
    }
}

pub trait SplitTask {
    type Output;
    fn split(self) -> (<Self as SplitTask>::Output, impl Run);
}

pub async fn hd_new_bucket_or_key_watcher(path: PathBuf, root: &Path) -> Result<Change, ()> {
    let file_name = match FileNameUtf8::run(&path).await {
        Renamed::Yes(name) => {
            tracing::warn!("[Event Watcher] {{ Rename bucket }} from: {path:?} to: {name:?}");
            return Err(());
        }
        Renamed::Not(str) => str,
        Renamed::Fail(error) => {
            tracing::error!(
                "[Event Watcher] {{ Error to obtain the bucket name }} Path: {path:?} - Error: {error}"
            );
            return Err(());
        }
    };

    if path.parent().is_some_and(|x| x == path) {
        let bucket = Bucket::from(file_name);
        Ok(Change::NewBucket { bucket })
    } else if let Some(bucket) = find_bucket(root, &path)
        && let Some(key) = Key::from_bucket(&bucket, &path)
    {
        tracing::info!("[Event Watcher] Ket key in the bucket {bucket} - key: {key:?}");
        Ok(Change::NewKey { bucket, key })
    } else {
        tracing::error!("Bucket not found {path:?}");
        Err(())
    }
}

pub async fn hd_new_object_watcher(
    path: PathBuf,
    root: &Path,
    ignore_suffix: &str,
) -> Result<(String, Change), ()> {
    if path
        .file_name()
        .and_then(|x| x.to_str())
        .is_some_and(|x| x.ends_with(ignore_suffix))
    {
        tracing::info!("[Event Watcher] {{ Ignore object }} {path:?} ");
        return Err(());
    }

    let Some(parent) = path.parent().filter(|x| x != &root) else {
        tracing::error!("[Event Watcher] Objects aren't allowed in the root path");
        return Err(());
    };

    let bucket = find_bucket(root, &path).unwrap();
    let key = Key::from_bucket(&bucket, parent).unwrap();
    let object = Object::new(&path).await;

    tracing::trace!("[Event Watcher] bucket: {bucket} - key: {key} - object: {object:?}");
    let file_name = object.file_name.clone();

    Ok((
        file_name,
        Change::NewObject {
            bucket,
            key,
            object,
        },
    ))
}

pub async fn hd_rename_watcher(
    root: &Path,
    mut from: PathBuf,
    mut to: PathBuf,
    rename_tracking: &mut HashMap<String, PathBuf>,
    rename_skip: &mut Vec<String>,
    ls: &LocalStorage,
) -> Result<Change, ()> {
    let file_name = match FileNameUtf8::run(&to).await {
        Renamed::Yes(name) => {
            tracing::debug!(
                "[Event Watcher] {{ Rename tracking }} new name: {name:?} - from: {from:?}"
            );
            rename_tracking.insert(name, from);
            return Err(());
        }
        Renamed::Not(name) => {
            if let Some(from_) = rename_tracking.remove(&name) {
                from = from_;
            } else if rename_skip.pop_if(|x| *x == name).is_some() {
                return Err(());
            }
            name
        }
        Renamed::Fail(error) => {
            tracing::error!("{error}");
            return Err(());
        }
    };

    if to.is_dir() {
        if from.parent().is_some_and(|x| x == root) {
            if let Some(from) = Bucket::new(&from)
                && let Some(to) = Bucket::new(&to)
            {
                return Ok(Change::NameBucket { from, to });
            }
            return Err(());
        } else {
            let Some(bucket) = find_bucket(root, &from) else {
                return Err(());
            };

            if let Some(_from) = Key::from_bucket(&bucket, &from)
                && let Some(to) = Key::from_bucket(&bucket, &to)
            {
                Ok(Change::NameKey {
                    bucket,
                    from: _from,
                    to,
                })
            } else {
                tracing::error!(
                    "[Event Watcher] {{ Fail to obtain the key from: {from:?} - to: {to:?} }}"
                );
                return Err(());
            }
        }
    } else {
        let Some(bucket) = find_bucket(root, &from) else {
            tracing::error!("[Event Watcher] {{ find_bucket }} Bucket {from:?} not found");
            return Err(());
        };

        let key = Key::from_bucket(&bucket, from.parent().unwrap()).unwrap();
        to.pop();
        to.push(&file_name);

        let Some(object) = ls
            .get_object_filename(
                &bucket,
                &key,
                from.file_name().and_then(|x| x.to_str()).unwrap(),
            )
            .await
        else {
            tracing::error!("[Event Watcher] {{ LocalStorage }} object {file_name:?} not found");
            todo!()
        };

        if let Err(er) = tokio::fs::rename(to, from).await {
            tracing::error!("[Event Watcher] {{ Restore Name Error }} {er}");
            return Err(());
        }

        rename_skip.push(file_name.clone());

        Ok(Change::NameObject {
            bucket,
            key,
            from: object.name,
            to: file_name,
        })
    }
}
