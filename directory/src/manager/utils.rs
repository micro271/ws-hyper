use regex::Regex;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};
use tokio::sync::mpsc::error::SendError;

use crate::{
    bucket::{
        Bucket, Cowed, DEFAULT_LENGTH_NANOID,
        key::{self, Key},
        utils::{
            Renamed,
            normalizeds::{NormalizePathUtf8, RenamedTo},
            rename_handlers::{NewObjNameHandlerBuilder, PathObject, RenameObjHandlerBuilder},
        },
    },
    manager::Change,
    state::local_storage::LocalStorage,
};

pub static REGEX_OBJECT_NAME: LazyLock<Regex> = LazyLock::new(|| {
    let patt = format!("^data_.{{{DEFAULT_LENGTH_NANOID}}}.__object$",);
    Regex::new(&patt).unwrap()
});

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
    mut path: PathBuf,
    root: &Path,
    skip: &mut ToSkip,
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
            skip.push_bucket(bucket.clone());
        }
        Ok(Change::NewBucket { bucket })
    } else if let Some(bucket) = Bucket::find_bucket(root, &path)
        && let Some(key) = Key::from_bucket(&bucket, &path)
    {
        tracing::info!("[Event Watcher] Get key in the bucket {bucket} - key: {key:?}");
        if to_skip {
            skip.push_key(bucket.clone(), key.clone());
        }
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
) -> Result<Change, ()> {
    if path
        .file_name()
        .and_then(|x| x.to_str())
        .is_some_and(|x| x.ends_with(ignore_suffix))
    {
        tracing::info!("[Event Watcher] {{ Ignore object }} {path:?} ");
        return Err(());
    }

    if path.parent().is_some_and(|x| x == root) {
        tracing::error!("[Event Watcher] Objects aren't allowed in the root path");
        return Err(());
    };

    let path_obj = PathObject::new(root, &path).await.unwrap();
    let (bucket, key, object) = path_obj.clone().inner();

    tracing::trace!("[Event Watcher] bucket: {bucket} - key: {key} - object: {object:?}");

    tracing::trace!("[Event Watcher] {{ skipped }} to skip: {path:?}");

    Ok(Change::NewObject {
        bucket,
        key,
        object,
    })
}

pub async fn hd_rename_path(
    root: &Path,
    from: PathBuf,
    to: PathBuf,
    skipped: &mut ToSkip,
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
                        skipped.push_bucket(bucket);
                    } else {
                        let bucket = Bucket::find_bucket(root, &to).ok_or(())?;
                        let key = Key::from_bucket(&bucket, &to).ok_or(())?;

                        skipped.push_key(bucket, key);
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

                if skipped.pop_bucket(&bucket) {
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
                let key = Key::from_bucket(&bucket, &to).ok_or(())?;

                if skipped.pop_key(bucket.borrow(), key.borrow()) {
                    Err(())
                } else {
                    let from = Key::from_bucket(&bucket, &from).ok_or(())?;
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

pub async fn hd_rename_object(root: &Path, from: PathBuf, to: PathBuf) -> Result<Change, ()> {
    let from_ = from.file_name().and_then(|x| x.to_str()).unwrap();

    if REGEX_OBJECT_NAME.is_match(from_) {
        let bucket = Bucket::find_bucket(root, &to);
        let (Some(key), Some(bucket)) = (
            bucket
                .as_ref()
                .and_then(|bucket| Key::from_bucket(bucket, to.parent().unwrap())),
            bucket,
        ) else {
            return Err(());
        };

        let (rx, task) = RenamedTo::new(&to).to(from).split();
        task.run();

        match rx.await {
            Ok(file_name) => {
                if let Some(to) = to.file_name().and_then(|x| x.to_str()) {
                    Ok(Change::NameObject {
                        bucket,
                        key,
                        file_name,
                        to: to.to_string(),
                    })
                } else {
                    Err(())
                }
            }
            Err(er) => {
                tracing::error!("{er}");
                Err(())
            }
        }
    } else {
        tracing::warn!("[ fn_hd_rename_object ] {{ Skipped }} from: {from:?} - to: {to:?}");
        Err(())
    }
}

#[derive(Debug, Default)]
pub struct ToSkip {
    pub buckets: Vec<Bucket<'static>>,
    pub keys: HashMap<Bucket<'static>, Vec<Key<'static>>>,
}

impl ToSkip {
    pub fn push_key(&mut self, bucket: Bucket<'_>, key: Key<'_>) {
        self.keys
            .entry(bucket.owned())
            .or_default()
            .push(key.owned());
    }

    pub fn push_bucket(&mut self, bucket: Bucket<'_>) {
        self.buckets.push(bucket.owned());
    }

    pub fn pop_key(&mut self, bucket: Bucket<'_>, key: Key<'_>) -> bool {
        let mut resp = false;
        if let Some(keys) = self.keys.get_mut(&bucket.cloned()) {
            if let Some(key) = keys.pop_if(|x| key.eq(x)) {
                tracing::info!("[ ToSkip ] {{ Delete Key }} {key:?} ");
                resp = true;
            }
            if keys.is_empty() && self.keys.remove(&bucket.cloned()).is_some() {
                tracing::error!("[ ToSKip ] {{ Delete Bucket in Key }} {bucket:?}");
            }
        }
        resp
    }
    pub fn pop_bucket(&mut self, bucket: &Bucket) -> bool {
        if self.buckets.pop_if(|x| x == bucket).is_some() {
            tracing::info!("[ ToSKip ] {{ Delete bucket from bucket }} {bucket:?}");
            true
        } else {
            false
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
            NewObjNameHandlerBuilder::default()
                .bucket(bucket.borrow())
                .key(key.borrow())
                .object(object)
                .build()
                .run(ls)
                .await;
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
            RenameObjHandlerBuilder::default()
                .bucket(bucket.borrow())
                .key(key.borrow())
                .to(to)
                .from(file_name)
                .build()
                .run(ls)
                .await;
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
        _ => {
            unimplemented!()
        }
    }
}
