use super::{Bucket, Key, Object, PhantomData};

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
