use crate::bucket::{Cowed, key::Key, object::Object};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ObjectEntry<'a> {
    key: &'a Key<'a>,
    objects: &'a Vec<Object>,
}

impl<'a> From<(&'a Key<'a>, &'a Vec<Object>)> for ObjectEntry<'a> {
    fn from(value: (&'a Key, &'a Vec<Object>)) -> Self {
        Self {
            key: value.0,
            objects: value.1,
        }
    }
}
