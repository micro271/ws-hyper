pub mod skipper;

use regex::Regex;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};
use tokio::sync::mpsc::error::SendError;

use crate::{
    bucket::{
        Bucket, Cowed,
        bucket_map::BucketMapType,
        key::Key,
        object::Object,
        utils::{
            Renamed,
            normalizeds::{NormalizeFileUtf8, NormalizePathUtf8},
        },
    },
    manager::{Change, utils::skipper::Skipper},
    state::local_storage::LocalStorage,
};

pub static OBJECT_NAME_REPEATED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^( .{{4}} ) ").unwrap());

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
        Self: Sized,
    {
        self
    }
}

impl<T: Task> Run for T {
    fn run(self)
    where
        Self: Sized,
    {
        tokio::spawn(self.task());
    }
}

pub trait SplitTask {
    type Output;
    fn split(self) -> (<Self as SplitTask>::Output, impl Run);
}

pub async fn hd_new_bucket_or_key_watcher<'a>(
    mut path: PathBuf,
    root: &Path,
    skip: &mut Skipper<'a>,
) -> Result<Change, ()> {
    let mut to_skip = false;
    let file_name = match NormalizePathUtf8::default().is_new().run(&path).await {
        Renamed::Yes(name) => {
            tracing::warn!("[Event Watcher] {{ Rename bucket }} from: {path:?} to: {name:?}");
            let (recv, task) = name.split();
            task.run();
            match recv.await {
                Ok(name) => {
                    path.pop();
                    path.push(&name);
                    to_skip = true;
                    name
                }
                Err(er) => {
                    tracing::error!("{er}");
                    return Err(());
                }
            }
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
        let bucket = Bucket::new_unchecked(file_name);
        if to_skip {
            skip.to_skip(skipper::Skip::Bucket {
                bucket: bucket.cloned(),
            });
        }
        Ok(Change::NewBucket { bucket })
    } else if let Some(bucket) = Bucket::find_bucket(root, &path)
        && let Some(key) = Key::from_bucket(bucket.borrow(), &path)
    {
        tracing::info!("[Event Watcher] Get key in the bucket {bucket} - key: {key:?}");
        if to_skip {
            skip.to_skip(skipper::Skip::Key {
                bucket: bucket.cloned(),
                key: key.cloned(),
            });
        }
        Ok(Change::NewKey { bucket, key })
    } else {
        tracing::error!("Bucket not found {path:?}");
        Err(())
    }
}

pub async fn hd_new_object_watcher(path: PathBuf, root: &Path) -> Result<Change, ()> {
    if path.parent().is_some_and(|x| x == root) {
        tracing::error!("[Event Watcher] Objects aren't allowed in the root path");
        return Err(());
    };

    let bucket = Bucket::find_bucket(root, &path).unwrap();
    let key = Key::from_bucket(bucket.borrow(), path.parent().unwrap()).unwrap();
    let object = Object::new(&path).await;
    tracing::trace!("[Event Watcher] bucket: {bucket} - key: {key} - object: {object:?}");

    tracing::trace!("[Event Watcher] {{ skipped }} to skip: {path:?}");

    Ok(Change::NewObject {
        bucket,
        key,
        object,
    })
}

pub async fn hd_rename_path<'a>(
    root: &Path,
    from: PathBuf,
    to: PathBuf,
    skipped: &mut Skipper<'a>,
) -> Result<Change, ()> {
    match NormalizePathUtf8::default().run(&to).await {
        Renamed::NeedRestore(renamed_to) => {
            let restore = renamed_to.to(&from);
            let (recv, task) = restore.split();

            task.run();

            match recv.await {
                Ok(from_name) => {
                    if let Some(parent) = to.parent()
                        && parent == root
                    {
                        let bucket = Bucket::new_unchecked(from_name);
                        skipped.to_skip(skipper::Skip::Bucket { bucket });
                    } else {
                        let bucket = Bucket::find_bucket(root, &to).ok_or(())?;
                        let key = Key::from_bucket(bucket.borrow(), &to).ok_or(())?;

                        skipped.to_skip(skipper::Skip::Key { bucket, key });
                    }
                    tracing::error!(
                        "[ fn_hd_rename_path ] {{ Restore Name }} from: {to:?} to: {from:?} "
                    );
                }
                Err(er) => tracing::error!("{er}"),
            }
            Err(())
        }
        Renamed::Not(name) => {
            if let Some(parent) = from.parent()
                && parent == root
            {
                let bucket = Bucket::new_unchecked(name);

                if skipped.skipped(skipper::Skip::Bucket {
                    bucket: bucket.cloned(),
                }) {
                    tracing::trace!("[ fn hd_rename_path ] Rename bucket, skipped name: {bucket}");
                    Err(())
                } else {
                    Ok(Change::NameBucket {
                        from: Bucket::new_unchecked(
                            from.file_name().and_then(|x| x.to_str()).ok_or(())?,
                        )
                        .owned(),
                        to: bucket,
                    })
                }
            } else {
                let bucket = Bucket::find_bucket(root, &to).ok_or(())?;
                let key = Key::from_bucket(bucket.borrow(), &to).ok_or(())?;

                if skipped.skipped(skipper::Skip::Key {
                    bucket: bucket.cloned(),
                    key: key.cloned(),
                }) {
                    Err(())
                } else {
                    let from = Key::from_bucket(bucket.borrow(), &from).ok_or(())?;
                    Ok(Change::NameKey {
                        bucket,
                        from,
                        to: key.owned(),
                    })
                }
            }
        }
        Renamed::Fail(error) => {
            tracing::error!(" [ fn_rename_path ] {{ NormalizePathUtf8 }} Error: {error}");
            Err(())
        }
        _ => todo!(),
    }
}

pub async fn hd_rename_object<'a>(
    root: &Path,
    from: PathBuf,
    to: PathBuf,
    to_skip: &mut Skipper<'a>,
) -> Result<Change, ()> {
    let bucket = Bucket::find_bucket(root, &to).unwrap();
    let key = Key::from_bucket(bucket.borrow(), &to).unwrap();
    let old_name = from.file_name().and_then(|x| x.to_str()).unwrap();

    match NormalizeFileUtf8::run(&to).await {
        Renamed::Yes(renamed) => {
            let file_name = renamed.file_name().to_string();
            let (rx, task) = renamed.split();
            task.run();

            if let Err(er) = rx.await {
                tracing::error!("[ fn hd_rename_object ] err: {er}, file: {to:?}");
                Err(())
            } else {
                to_skip.to_skip(skipper::Skip::Object {
                    bucket: bucket.cloned(),
                    key: key.cloned(),
                    file_name: file_name.clone(),
                });
                Ok(Change::NameObject {
                    bucket: bucket,
                    key: key,
                    file_name: old_name.to_string(),
                    to: file_name,
                })
            }
        }
        Renamed::Not(file) => {
            if to_skip.skipped(skipper::Skip::Object {
                bucket: bucket.cloned(),
                key: key.cloned(),
                file_name: (&file).into(),
            }) {
                return Err(());
            }

            Ok(Change::NameObject {
                bucket: bucket,
                key: key,
                file_name: old_name.to_string(),
                to: file,
            })
        }
        Renamed::Fail(er) => {
            tracing::error!("[ fn hd_rename_object ] Error: {er}");
            Err(())
        }
        _ => {
            unimplemented!()
        }
    }
}

pub async fn change_local_storage(ch: &mut Change, ls: Arc<LocalStorage>) {
    match ch {
        Change::NewObject {
            object,
            key,
            bucket,
        } => {
            if let Err(er) = ls.new_object(bucket.borrow(), key.borrow(), object).await {
                tracing::error!("[ fn change_local_storage ] error: {er}");
            }
        }
        Change::DeleteObject {
            file_name,
            bucket,
            key,
        } => {
            ls.delete_object(bucket.borrow(), key.borrow(), file_name)
                .await;
        }
        Change::NameObject {
            key,
            to,
            bucket,
            file_name,
        } => {
            if let Err(er) = ls
                .set_name(bucket.borrow(), key.borrow(), file_name, to)
                .await
            {
                tracing::error!("[ fn change_local_storage ] error: {er} ")
            }
        }
        Change::DeleteBucket { bucket } => {
            if let Err(er) = ls.delete_bucket(bucket.borrow()).await {
                tracing::debug!("{er}")
            }
        }
        Change::NameBucket { from, to } => {
            if let Err(er) = ls.set_name_bucket(from.borrow(), to.borrow()).await {
                tracing::debug!("{er}")
            }
        }
        Change::NameKey { bucket, from, to } => {
            if let Err(er) = ls
                .set_name_key(bucket.borrow(), from.borrow(), to.borrow())
                .await
            {
                tracing::debug!("{er}")
            }
        }
        Change::DeleteKey { bucket, key } => {
            if let Err(er) = ls.delete_key(bucket.borrow(), key.borrow()).await {
                tracing::debug!("{er}")
            }
        }
        e => tracing::warn!("[fn change_local_storage] Unimplemented arm; change: {e:?}"),
    }
}
