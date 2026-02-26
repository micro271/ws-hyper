use super::{Bucket, Cowed, Key, NewObjNameHandler, Object, RenameObjHandler, types::*};
use std::marker::PhantomData;

#[derive(Debug)]
pub struct NewObjNameHandlerBuilder<'a, O, K, B> {
    object: O,
    key: K,
    bucket: B,
    pub(crate) _ph: PhantomData<&'a ()>,
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
    pub fn key(self, key: Key<'a>) -> NewObjNameHandlerBuilder<'a, O, ObjNameHandlerKey<'a>, B> {
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
        bucket: Bucket<'a>,
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
            key: key.cloned(),
            bucket,
        }
    }
}

impl<'a, K, F, T> RenameObjHandlerBuilder<'a, ObjNameHandlerNoBucket<'a>, K, F, T> {
    pub fn bucket(
        self,
        bucket: Bucket<'a>,
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
    pub fn key(self, key: Key<'a>) -> RenameObjHandlerBuilder<'a, B, ObjNameHandlerKey<'a>, F, T> {
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
            key: key.cloned(),
            from,
            to,
        }
    }
}
