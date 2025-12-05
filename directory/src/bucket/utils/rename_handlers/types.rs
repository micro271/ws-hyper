use std::path::{Path, PathBuf};

use super::{Bucket, Key, Object, PhantomData};

#[derive(Debug, Clone, PartialEq)]
pub struct PathObject {
    pub bucket: Bucket,
    pub key: Key,
    pub object: Object,
}

impl PathObject {
    pub fn get(&self) -> (&Bucket, &Key, &Object) {
        (&self.bucket, &self.key, &self.object)
    }
    pub async fn new(root: &Path, path: &Path) -> Option<Self> {
        let bucket = Bucket::find_bucket(root, path)?;
        let key = Key::from_bucket(&bucket, path.parent()?)?;
        Some(Self {
            bucket,
            key,
            object: Object::new(path).await,
        })
    }
    pub fn from_terna(bucket: Bucket, key: Key, object: Object) -> Self {
        Self {
            bucket,
            key,
            object,
        }
    }
    pub fn inner(self) -> (Bucket, Key, Object) {
        (self.bucket, self.key, self.object)
    }
}


#[derive(Debug)]
pub struct RenamedToNoTo;

#[derive(Debug)]
pub struct RenamedToWithTo(pub(crate) PathBuf);

impl RenamedToWithTo {
    pub fn file_name(&self) -> &str {
        self.0.file_name().and_then(|x| x.to_str()).unwrap()
    }
    pub fn path(&self) -> &Path {
        self.0.as_path()
    }
}

#[derive(Debug, Default)]
pub struct ObjNameHandlerNoObject<'a>(PhantomData<&'a ()>);
pub struct ObjNameHandlerObject<'a>(pub(super) &'a mut Object);

#[derive(Debug, Default)]
pub struct ObjNameHandlerNoBucket<'a>(PhantomData<&'a ()>);
pub struct ObjNameHandlerBucket<'a>(pub(super) &'a mut Bucket);

#[derive(Debug, Default)]
pub struct ObjNameHandlerNoKey<'a>(PhantomData<&'a ()>);
pub struct ObjNameHandlerKey<'a>(pub(super) &'a mut Key);

#[derive(Debug, Default)]
pub struct ObjNameHandlerNoTo<'a>(PhantomData<&'a ()>);
pub struct ObjNameHandlerTo<'a>(pub(super) &'a mut String);

#[derive(Debug, Default)]
pub struct NewObjNameHandlerNoFrom<'a>(PhantomData<&'a ()>);
pub struct NewObjNameHandlerFrom<'a>(pub(super) &'a mut String);
