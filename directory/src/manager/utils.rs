use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::sync::mpsc::error::SendError;

use crate::{
    bucket::{
        Bucket,
        key::Key,
        object::Object,
        utils::{
            Renamed,
            normalizeds::{NormalizeFileUtf8, NormalizePathUtf8},
        },
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

pub async fn hd_new_bucket_or_key_watcher(
    mut path: PathBuf,
    root: &Path,
    skip: &mut Vec<PathBuf>,
) -> Result<Change, ()> {
    let file_name = match NormalizePathUtf8::default().is_new().run(&path).await {
        Renamed::Yes(name) => {
            tracing::warn!("[Event Watcher] {{ Rename bucket }} from: {path:?} to: {name:?}");
            path.pop();
            path.push(&name);
            skip.push(path.clone());
            name
        }
        Renamed::Not(str) => str,
        Renamed::Fail(error) => {
            tracing::error!(
                "[Event Watcher] {{ Error to obtain the bucket name }} Path: {path:?} - Error: {error}"
            );
            return Err(());
        }
        e => {
            tracing::error!("{e:?}");
            return Err(());
        }
    };

    if path.parent().is_some_and(|x| x == root) {
        let bucket = Bucket::from(file_name);
        Ok(Change::NewBucket { bucket })
    } else if let Some(bucket) = Bucket::find_bucket(root, &path)
        && let Some(key) = Key::from_bucket(&bucket, &path)
    {
        tracing::info!("[Event Watcher] Get key in the bucket {bucket} - key: {key:?}");
        Ok(Change::NewKey { bucket, key })
    } else {
        tracing::error!("Bucket not found {path:?}");
        Err(())
    }
}

pub async fn hd_new_object_watcher(
    mut path: PathBuf,
    root: &Path,
    ignore_suffix: &str,
    skiped: &mut Vec<PathBuf>,
) -> Result<Change, ()> {
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

    let bucket = Bucket::find_bucket(root, &path).unwrap();
    let key = Key::from_bucket(&bucket, parent).unwrap();
    let object = Object::new(&path).await;

    tracing::trace!("[Event Watcher] bucket: {bucket} - key: {key} - object: {object:?}");

    path.pop();
    path.push(&object.file_name);

    tracing::trace!("[Event Watcher] {{ skipped }} to skip: {path:?}");
    skiped.push(path);

    Ok(Change::NewObject {
        bucket,
        key,
        object,
    })
}

pub async fn hd_rename_watcher(
    root: &Path,
    mut from: PathBuf,
    to: PathBuf,
    rename_tracking: &mut HashMap<String, PathBuf>,
    rename_skip: &mut Vec<PathBuf>,
    ls: &LocalStorage,
) -> Result<Change, ()> {
    if let Some(skipped) = rename_skip.pop_if(|x| *x == to) {
        tracing::warn!("[Event Watcer] file_name skiped: {skipped:?}");
        return Err(());
    }

    let mut sync_from = |str: &str| {
        if let Some(from_) = rename_tracking.remove(str) {
            tracing::info!(
                "[ fn_hd_rename_watcher] {{ closure_sync_from }} rename_tracking old from: {from:?} - new from: {from_:?}"
            );
            from = from_;
        }
    };

    let rename_hd = rename_handler(&to).await;

    match rename_hd {
        NameHandlerType::Path(Renamed::NeedRestore) => {
            if let Err(er) = tokio::fs::rename(&to, &from).await {
                tracing::error!(
                    "[ fn_handler_rename_watcher ] Restore name: from {to:?} - to {from:?} - Error: {er}"
                );
            }
            tracing::error!(
                "[ fn_handler_rename_watcher ] Invalid new path {to:?}; it has been restored to {from:?}"
            );
            tracing::trace!("[ fn_handler_rename_watcher ] {{ skiped }} ");
            rename_skip.push(from);
            Err(())
        }
        NameHandlerType::Path(Renamed::Yes(name)) | NameHandlerType::Object(Renamed::Yes(name)) => {
            tracing::debug!(
                "[Event Watcher] {{ Rename tracking }} new name: {name:?} - from: {from:?}"
            );
            rename_tracking.insert(name, from);
            Err(())
        }
        NameHandlerType::Path(Renamed::Not(name)) => {
            sync_from(&name);

            if from.parent().is_some_and(|x| x == root) {
                Ok(Change::NameBucket {
                    from: Bucket::from(from.file_name().and_then(|x| x.to_str()).ok_or(())?),
                    to: Bucket::from(name),
                })
            } else {
                let Some(bucket) = Bucket::find_bucket(root, &to) else {
                    return Err(());
                };

                if let Some(_from) = Key::from_bucket(&bucket, &from)
                    && let Some(to) = Key::from_bucket(&bucket, &to)
                {
                    tracing::trace!(
                        "[ fn_hd_rename_watcher ] {{ Rename Key }} from: {_from:?} - to: {to:?} "
                    );
                    Ok(Change::NameKey {
                        bucket,
                        from: _from,
                        to,
                    })
                } else {
                    tracing::error!(
                        "[Event Watcher] {{ Fail to obtain the key from: {from:?} - to: {to:?} }}"
                    );
                    Err(())
                }
            }
        }
        NameHandlerType::Object(Renamed::Not(name)) => {
            sync_from(&name);

            let bucket = Bucket::find_bucket(root, &to).unwrap();

            let key = Key::from_bucket(&bucket, to.parent().unwrap()).unwrap();
            let Some(from_file_name) = from
                .file_name()
                .and_then(|x| x.to_str().map(ToString::to_string))
            else {
                tracing::error!(
                    "[ fn_hd_rename_watcher ] {{ Rename Object }} file name of from path: {from:?} is not valid"
                );
                return Err(());
            };

            if let Ok(Some(object)) = ls.get_object_filename(&bucket, &key, &from_file_name).await {
                tracing::trace!("[ fn_hd_rename_watcher ] {{ Object found in db }} {object:?}");

                if let Err(er) = tokio::fs::rename(to, &from).await {
                    tracing::error!("[Event Watcher] {{ Restore Name Error }} {er}");
                    return Err(());
                }

                from.pop();
                from.push(from_file_name);
                tracing::trace!("[ fn_hd_rename_watcher ] {{ Skipped }} {from:?}");
                rename_skip.push(from);

                Ok(Change::NameObject {
                    bucket,
                    key,
                    from: object.name,
                    to: name,
                })
            } else {
                tracing::warn!(
                    "[ hd_rename_watcher ] {{ Object not found }} bucket: {bucket} - key: {key} - file: {from_file_name}"
                );
                let new_obj = Object::new(&to).await;
                tracing::info!(
                    "[ hd_rename_watcher ] {{ Create object from rename }} bucket: {bucket} - key: {key} - name: {} - file_name: {}",
                    new_obj.name,
                    new_obj.file_name
                );
                rename_skip.push(to);
                Ok(Change::NewObject {
                    bucket,
                    key,
                    object: new_obj,
                })
            }
        }
        e => {
            tracing::error!("{e:?}");
            Err(())
        }
    }
}

async fn rename_handler(to: &Path) -> NameHandlerType {
    if to.is_dir() {
        NameHandlerType::Path(NormalizePathUtf8::default().run(to).await)
    } else {
        NameHandlerType::Object(NormalizeFileUtf8::run(to).await)
    }
}

#[derive(Debug)]
enum NameHandlerType {
    Path(Renamed),
    Object(Renamed),
}
