pub mod skipper;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::mpsc::error::SendError;

use crate::{
    bucket::{
        Bucket, Cowed,
        key::Key,
        object::{Object, OwnerFile},
        utils::{
            Rename, RenameDecision,
            normalizeds::{NormalizeFileUtf8, NormalizePathUtf8},
        },
    },
    manager::{Change, utils::skipper::Skipper},
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

pub async fn hd_new_bucket_or_key_watcher(
    path: PathBuf,
    root: &Path,
    skip: Skipper,
) -> Result<Change, ()> {
    match NormalizePathUtf8::default().is_new().run(&path).await {
        Ok(RenameDecision::Not(str)) => {
            tracing::debug!("[ fn hd_new_bucket_or_key_watcher ] File mane ok {str}");
            let Some(parent) = path.parent() else {
                return Err(());
            };
            tracing::debug!("[ fn hd_new_bucket_or_key_watcher ] Parent file {parent:?}");
            if parent == root {
                let bucket = Bucket::new_unchecked(str);
                Ok(Change::NewBucket { bucket })
            } else {
                let bucket = Bucket::find_bucket(root, &path).unwrap();
                let Some(key) = Key::from_bucket(bucket.borrow(), &path) else {
                    tracing::error!(
                        "[ fn hd_new_bucket_or_key_watcher ] key not found from the bucket {bucket:?} - path: {path:?}"
                    );
                    return Err(());
                };
                Ok(Change::NewKey { bucket, key })
            }
        }
        Ok(RenameDecision::Yes(Rename { parent, from, to })) => {
            tracing::warn!(
                "[ fn hd_new_bucket_or_key_watcher ] We need rename from {from} to {to} - path: {parent:?} "
            );
            let from = parent.join(from);
            let to_ = parent.join(&to);
            tracing::trace!(
                "[ fn hd_new_bucket_or_key_watcher ] Path from {from:?} - Path to {to_:?}"
            );
            if let Err(er) = tokio::fs::rename(from, to_).await {
                tracing::error!("[ fn hd_new_bucket_or_key_watcher ] Rename error: {er}");
                return Err(());
            }
            let bucket = Bucket::new_unchecked(&to).owned();
            if root == parent {
                skip.bucket_tracker().to_skip(bucket.cloned()).await;
                tracing::debug!("[ fn hd_new_bucket_or_key_watcher ] new skip: {skip:?}");
                Ok(Change::NewBucket { bucket })
            } else {
                let Some(key) = Key::from_bucket(bucket.borrow(), &parent.join(&to)) else {
                    tracing::error!(
                        "[ fn hd_new_bucket_or_key_watcher ] key not found from the bucket {bucket:?} - path: {path:?}"
                    );
                    return Err(());
                };
                skip.key_tracker()
                    .to_skip(bucket.cloned(), key.cloned())
                    .await;
                tracing::debug!("[ fn hd_new_bucket_or_key_watcher ] new skip: {skip:?}");
                Ok(Change::NewKey { bucket, key })
            }
        }
        Err(_) => Err(()),
        _ => {
            unreachable!("This arm should never be to reached")
        }
    }
}

pub async fn hd_new_object_watcher(
    path: PathBuf,
    root: &Path,
    skip: Skipper,
) -> Result<Change, ()> {
    if path.parent().is_some_and(|x| x == root) {
        tracing::error!("[Event Watcher] Objects aren't allowed in the root path");
        return Err(());
    };

    match NormalizeFileUtf8::run(&path).await {
        Ok(RenameDecision::Yes(Rename { parent, from, to })) => {
            let from = parent.join(from);
            let to_ = parent.join(&to);

            if let Err(er) = tokio::fs::rename(from, &to_).await {
                tracing::error!("file Rename error: {er}");
                return Err(());
            }

            let bucket = Bucket::find_bucket(root, &path).unwrap();
            let key = Key::from_bucket(bucket.borrow(), &parent).unwrap();
            let object = Object::new(&to_, OwnerFile::User("Test".to_string())).await;
            skip.object_tracker()
                .to_skip(bucket.cloned(), key.cloned(), to)
                .await;

            Ok(Change::NewObject {
                bucket,
                key,
                object,
            })
        }
        Ok(RenameDecision::Not(_)) => {
            let bucket = Bucket::find_bucket(root, &path).unwrap();
            let key = Key::from_bucket(bucket.borrow(), path.parent().unwrap()).unwrap();
            let object = Object::new(&path, OwnerFile::User("test".to_string())).await;
            tracing::trace!("[Event Watcher] bucket: {bucket} - key: {key} - object: {object:?}");

            Ok(Change::NewObject {
                bucket,
                key,
                object,
            })
        }
        Err(er) => {
            tracing::error!("[ fn hd_new_object_watcher ] NormalizeFIleUtf8 Error {er:?}");
            Err(())
        }
        _ => unreachable!("This arm shound never be reached"),
    }
}

pub async fn hd_rename_path<'a>(
    root: &Path,
    original_from: PathBuf,
    original_to: PathBuf,
    skipped: Skipper,
) -> Result<Change, ()> {
    match NormalizePathUtf8::default().run(&original_to).await {
        Ok(RenameDecision::Not(name)) => {
            if original_to.parent().is_some_and(|x| x == root) {
                let to = Bucket::new_unchecked(name);

                if skipped.bucket_tracker().skipped(&to).await {
                    tracing::trace!("[ fn hd_rename_parh ] skipped {to:?}");
                    return Err(());
                }

                let from = Bucket::new_unchecked(
                    original_from.file_name().and_then(|x| x.to_str()).unwrap(),
                )
                .owned();
                Ok(Change::NameBucket { from, to })
            } else {
                let bucket = Bucket::find_bucket(root, &original_to).unwrap();
                let key = Key::from_bucket(bucket.borrow(), &original_to).unwrap();

                if skipped.key_tracker().skipped(&bucket, &key).await {
                    tracing::trace!("[ fn hd_rename_part ] skipped {bucket:?} {key:?}");
                    return Err(());
                }

                let key_from = Key::from_bucket(bucket.borrow(), &original_from).unwrap();
                Ok(Change::NameKey {
                    bucket,
                    from: key_from,
                    to: key,
                })
            }
        }
        Ok(RenameDecision::Yes(Rename { parent, from, to })) => {
            let from_ = parent.join(&from);
            let to_ = parent.join(&to);

            tracing::trace!("[ fn hd_rename_part ] rename from: {from_:?} to: {to:?}");
            if let Err(er) = tokio::fs::rename(&from_, &to_).await {
                tracing::error!("{er}");
            }

            if parent == root {
                let bucket = Bucket::new_unchecked(to);
                skipped.bucket_tracker().to_skip(bucket.cloned()).await;
                let original_name = Bucket::new_unchecked(
                    original_from.file_name().and_then(|x| x.to_str()).unwrap(),
                )
                .owned();
                Ok(Change::NameBucket {
                    from: original_name,
                    to: bucket,
                })
            } else {
                let bucket = Bucket::find_bucket(root, &original_to).unwrap();
                let original_key = Key::from_bucket(bucket.borrow(), &original_from).unwrap();
                let key = Key::from_bucket(bucket.borrow(), &original_to).unwrap();
                skipped
                    .key_tracker()
                    .to_skip(bucket.cloned(), key.cloned())
                    .await;

                Ok(Change::NameKey {
                    bucket,
                    from: original_key,
                    to: key,
                })
            }
        }
        Ok(RenameDecision::NeedRestore) => {
            tracing::trace!(
                "[ fn hd_rename_path] Restore name from: {original_to:?} to: {original_from:?}"
            );
            if let Err(er) = tokio::fs::rename(&original_to, &original_from).await {
                tracing::error!("{er}");
            }
            Err(())
        }
        Err(_) => todo!(),
        _ => Err(()),
    }
}

pub async fn hd_rename_object<'a>(
    root: &Path,
    original_from: PathBuf,
    original_to: PathBuf,
    to_skip: Skipper,
) -> Result<Change, ()> {
    let bucket = Bucket::find_bucket(root, &original_to).unwrap();
    let key = Key::from_bucket(bucket.borrow(), original_to.parent().unwrap()).unwrap();

    match NormalizeFileUtf8::run(&original_to).await {
        Ok(RenameDecision::Not(name)) => {
            if to_skip.object_tracker().skipped(&bucket, &key, &name).await {
                tracing::trace!(
                    "[ fn hd_rename_object ] Rename object skipped {bucket:?} {key} {name}"
                );
                return Err(());
            }
            let old_name = original_from
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap()
                .to_string();

            Ok(Change::NameObject {
                bucket,
                key,
                from: old_name,
                to: name,
            })
        }
        Ok(RenameDecision::Yes(Rename { parent, from, to })) => {
            let from_ = parent.join(&from);
            let to_ = parent.join(&to);
            if let Err(er) = tokio::fs::rename(&from_, &to_).await {
                tracing::error!("{er}");
                return Err(());
            }
            tracing::debug!("[ fn hd_rename_object ] skip {bucket:?} {key:?} {to:?}");
            to_skip
                .object_tracker()
                .to_skip(bucket.clone(), key.clone(), to.clone())
                .await;
            Ok(Change::NameObject {
                bucket,
                key,
                from,
                to,
            })
        }
        Err(er) => {
            tracing::error!("{er:?}");
            Err(())
        }
        Ok(des) => unreachable!(
            "[ fn rename_object ] this arm never would reached {:?}",
            des
        ),
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
            from,
        } => {
            if let Err(er) = ls.set_name(bucket.borrow(), key.borrow(), from, to).await {
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
