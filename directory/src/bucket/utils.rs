use std::{
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::Arc,
};

use nanoid::nanoid;
use regex::Regex;

use crate::{
    bucket::{
        Bucket,
        key::Key,
        object::{EXTENSION_OBJECT, Object},
    },
    state::local_storage::{LocalStorage, error::LsError},
};

pub struct NormalizeForObjectName;

impl NormalizeForObjectName {
    pub async fn run(path: &Path) -> String {
        let mut to = PathBuf::from(path);
        to.pop();
        let new_name = format!("{}.{EXTENSION_OBJECT}", nanoid!(24));
        to.push(&new_name);

        if let Err(er) = tokio::fs::rename(path, &to).await {
            tracing::error!(
                "From NormalizeFileUtf - Error to rename file from: {path:?} - to: {to:?}"
            );
            tracing::error!("IoError: {er}");
        }

        tracing::warn!("[NormalizeFileUtf] {{ Rename file }} from: {path:?} to: {to:?}");
        new_name
    }
}

#[derive(Debug)]
pub struct FileNameUtf8;

impl FileNameUtf8 {
    pub async fn run(path: &Path) -> Renamed {
        if let Some(str) = path.file_name().and_then(|x| x.to_str()) {
            Renamed::Not(str.to_string())
        } else {
            let mut to = PathBuf::from(path);
            to.pop();

            let ext = path
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or("__unknown");
            let new_name = format!("{}.{ext}", nanoid!(24));
            to.push(&new_name);
            if let Err(er) = tokio::fs::rename(path, &to).await {
                tracing::error!(
                    "From NormalizeFileUtf - Error to rename file from: {path:?} - to: {to:?}"
                );
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
            return Some(Bucket::from(
                child.file_name().and_then(|x| x.to_str()).unwrap(),
            ));
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
            Renamed::Yes(str) | Renamed::Not(str) => Some(str),
            Renamed::Fail(_) => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct ObjNameHandlerNoObject<'a>(PhantomData<&'a ()>);
pub struct ObjNameHandlerObject<'a>(&'a mut Object);

#[derive(Debug, Default)]
pub struct ObjNameHandlerNoBucket<'a>(PhantomData<&'a ()>);
pub struct ObjNameHandlerBucket<'a>(&'a mut Bucket);

#[derive(Debug, Default)]
pub struct ObjNameHandlerNoKey<'a>(PhantomData<&'a ()>);
pub struct ObjNameHandlerKey<'a>(&'a mut Key);

#[derive(Debug, Default)]
pub struct ObjNameHandlerNoTo<'a>(PhantomData<&'a ()>);
pub struct ObjNameHandlerTo<'a>(&'a mut String);

#[derive(Debug, Default)]
pub struct NewObjNameHandlerNoFrom<'a>(PhantomData<&'a ()>);
pub struct NewObjNameHandlerFrom<'a>(&'a mut String);

#[derive(Debug)]
pub struct NewObjNameHandlerBuilder<'a, O, K, B> {
    object: O,
    key: K,
    bucket: B,
    _ph: PhantomData<&'a ()>,
}

pub struct NewObjNameHandler<'a> {
    object: &'a mut Object,
    key: &'a mut Key,
    bucket: &'a mut Bucket,
}

pub struct RenameObjHandler<'a> {
    bucket: &'a mut Bucket,
    key: &'a mut Key,
    from: &'a mut String,
    to: &'a mut String,
}

pub struct RenameObjHandlerBuilder<'a, B, K, F, T> {
    bucket: B,
    key: K,
    from: F,
    to: T,
    _ph: PhantomData<&'a ()>,
}

impl<'a> std::default::Default
    for NewObjNameHandlerBuilder<
        'a,
        ObjNameHandlerNoObject<'a>,
        ObjNameHandlerNoKey<'a>,
        ObjNameHandlerNoBucket<'a>,
    >
{
    fn default() -> Self {
        Self {
            object: ObjNameHandlerNoObject::default(),
            key: ObjNameHandlerNoKey::default(),
            bucket: ObjNameHandlerNoBucket::default(),
            _ph: PhantomData,
        }
    }
}

impl<'a> std::default::Default
    for RenameObjHandlerBuilder<
        'a,
        ObjNameHandlerNoBucket<'a>,
        ObjNameHandlerNoKey<'a>,
        NewObjNameHandlerNoFrom<'a>,
        ObjNameHandlerNoTo<'a>,
    >
{
    fn default() -> Self {
        Self {
            key: ObjNameHandlerNoKey::default(),
            bucket: ObjNameHandlerNoBucket::default(),
            _ph: PhantomData,
            from: NewObjNameHandlerNoFrom::default(),
            to: ObjNameHandlerNoTo::default(),
        }
    }
}

impl<'a, K, B> NewObjNameHandlerBuilder<'a, ObjNameHandlerNoObject<'_>, K, B> {
    pub fn object(
        self,
        object: &'a mut Object,
    ) -> NewObjNameHandlerBuilder<'a, ObjNameHandlerObject<'a>, K, B> {
        NewObjNameHandlerBuilder {
            object: ObjNameHandlerObject(object),
            key: self.key,
            bucket: self.bucket,
            _ph: self._ph,
        }
    }
}
impl<'a, O, B> NewObjNameHandlerBuilder<'a, O, ObjNameHandlerNoKey<'_>, B> {
    pub fn key(
        self,
        key: &'a mut Key,
    ) -> NewObjNameHandlerBuilder<'a, O, ObjNameHandlerKey<'a>, B> {
        NewObjNameHandlerBuilder {
            object: self.object,
            key: ObjNameHandlerKey(key),
            bucket: self.bucket,
            _ph: PhantomData,
        }
    }
}

impl<'a, O, K> NewObjNameHandlerBuilder<'a, O, K, ObjNameHandlerNoBucket<'a>> {
    pub fn bucket(
        self,
        bucket: &'a mut Bucket,
    ) -> NewObjNameHandlerBuilder<'a, O, K, ObjNameHandlerBucket<'a>> {
        NewObjNameHandlerBuilder {
            object: self.object,
            key: self.key,
            bucket: ObjNameHandlerBucket(bucket),
            _ph: PhantomData,
        }
    }
}

impl<'a>
    NewObjNameHandlerBuilder<
        'a,
        ObjNameHandlerObject<'a>,
        ObjNameHandlerKey<'a>,
        ObjNameHandlerBucket<'a>,
    >
{
    pub fn build(self) -> NewObjNameHandler<'a> {
        let ObjNameHandlerObject(object) = self.object;
        let ObjNameHandlerBucket(bucket) = self.bucket;
        let ObjNameHandlerKey(key) = self.key;

        NewObjNameHandler {
            object,
            key,
            bucket,
        }
    }
}

impl<'a, K, F, T> RenameObjHandlerBuilder<'a, ObjNameHandlerNoBucket<'a>, K, F, T> {
    pub fn bucket(
        self,
        bucket: &'a mut Bucket,
    ) -> RenameObjHandlerBuilder<'a, ObjNameHandlerBucket<'a>, K, F, T> {
        RenameObjHandlerBuilder {
            bucket: ObjNameHandlerBucket(bucket),
            key: self.key,
            from: self.from,
            to: self.to,
            _ph: PhantomData,
        }
    }
}

impl<'a, B, F, T> RenameObjHandlerBuilder<'a, B, ObjNameHandlerNoKey<'a>, F, T> {
    pub fn key(
        self,
        key: &'a mut Key,
    ) -> RenameObjHandlerBuilder<'a, B, ObjNameHandlerKey<'a>, F, T> {
        RenameObjHandlerBuilder {
            bucket: self.bucket,
            key: ObjNameHandlerKey(key),
            from: self.from,
            to: self.to,
            _ph: PhantomData,
        }
    }
}

impl<'a, B, K, T> RenameObjHandlerBuilder<'a, B, K, NewObjNameHandlerNoFrom<'a>, T> {
    pub fn from(
        self,
        from: &'a mut String,
    ) -> RenameObjHandlerBuilder<'a, B, K, NewObjNameHandlerFrom<'a>, T> {
        RenameObjHandlerBuilder {
            bucket: self.bucket,
            key: self.key,
            from: NewObjNameHandlerFrom(from),
            to: self.to,
            _ph: PhantomData,
        }
    }
}

impl<'a, B, K, F> RenameObjHandlerBuilder<'a, B, K, F, ObjNameHandlerNoTo<'a>> {
    pub fn to(
        self,
        to: &'a mut String,
    ) -> RenameObjHandlerBuilder<'a, B, K, F, ObjNameHandlerTo<'a>> {
        RenameObjHandlerBuilder {
            bucket: self.bucket,
            key: self.key,
            from: self.from,
            to: ObjNameHandlerTo(to),
            _ph: PhantomData,
        }
    }
}

impl<'a>
    RenameObjHandlerBuilder<
        'a,
        ObjNameHandlerBucket<'a>,
        ObjNameHandlerKey<'a>,
        NewObjNameHandlerFrom<'a>,
        ObjNameHandlerTo<'a>,
    >
{
    pub fn build(self) -> RenameObjHandler<'a> {
        let ObjNameHandlerBucket(bucket) = self.bucket;
        let ObjNameHandlerKey(key) = self.key;
        let NewObjNameHandlerFrom(from) = self.from;
        let ObjNameHandlerTo(to) = self.to;

        RenameObjHandler {
            bucket,
            key,
            from,
            to,
        }
    }
}

impl<'a> NewObjNameHandler<'a> {
    pub async fn run(&mut self, ls: Arc<LocalStorage>) {
        while let Err(LsError::DuplicateKey) =
            ls.new_object(self.bucket, self.key, self.object).await
        {
            tracing::error!(
                "[ RenameObjectHandler ] {{ Duplicate key }} bucket: {}, key: {}, object.name: {} ",
                self.bucket,
                self.key,
                self.object.name
            );
            let regex = Regex::new(r"(^\(@prefix\).*)__-").unwrap();
            if regex.is_match(&self.object.name) {
                self.object.name = regex
                    .replace(
                        &self.object.name,
                        &format!("prefix{}__-{}", nanoid!(6), self.object.name),
                    )
                    .into_owned();
            } else {
                self.object.name.insert_str(
                    0,
                    &format!("(@prefix){}__-{}", nanoid!(6), self.object.name),
                );
            }
            tracing::info!(
                "[ ManagerRunning ] {{ Duplicate key }} object new name {}",
                self.object.name
            );
        }
    }
}

impl<'a> RenameObjHandler<'a> {
    pub async fn run(&mut self, ls: Arc<LocalStorage>) {
        while let Err(LsError::DuplicateKey) =
            ls.set_name(self.bucket, self.key, self.from, self.to).await
        {
            tracing::error!(
                "[ RenameObjectHandler ] {{ Duplicate key }} bucket: {}, key: {}, object.name: {} ",
                self.bucket,
                self.key,
                self.to
            );
            let regex = Regex::new(r"(^\(@prefix\).*)__-").unwrap();
            if regex.is_match(self.to) {
                *self.to = regex
                    .replace(&self.to, &format!("prefix{}__-{}", nanoid!(6), self.to))
                    .into_owned();
            } else {
                self.to
                    .insert_str(0, &format!("(@prefix){}__-{}", nanoid!(6), self.to));
            }
            tracing::info!(
                "[ ManagerRunning ] {{ Duplicate key }} object new name {}",
                self.to
            );
        }
    }
}
