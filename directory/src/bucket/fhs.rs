use serde::Serialize;

use crate::bucket::{Bucket, Cowed, bucket_map::KeyEntry, key::Key, object::Object};

#[derive(Serialize)]
#[serde(untagged)]
pub enum Fhs<'a> {
    FhsEntry {
        key: Key<'a>,
        inner_key: Option<Vec<Fhs<'a>>>,
        objects: Option<&'a Vec<Object>>,
    },
    Name(&'a str),
}

impl<'a> Fhs<'a> {
    pub fn create_tree(root_key: Key<'a>, entry: &'a KeyEntry) -> Self {
        Self::FhsEntry {
            key: root_key,
            inner_key: entry.keys.as_ref().map(|x| {
                x.iter()
                    .map(|(k, v)| (k.borrow(), v).into())
                    .collect::<Vec<Fhs<'_>>>()
            }),
            objects: entry.objects.as_ref(),
        }
    }
}

impl<'a> From<(Key<'a>, &'a KeyEntry)> for Fhs<'a> {
    fn from(value: (Key<'a>, &'a KeyEntry)) -> Self {
        Self::FhsEntry {
            key: value.0,
            inner_key: value
                .1
                .keys
                .as_ref()
                .map(|x| x.keys().map(|x| Self::Name(x.name())).collect::<Vec<_>>()),
            objects: value.1.objects.as_ref(),
        }
    }
}

impl<'a> From<Vec<&'a Bucket<'_>>> for Fhs<'a> {
    fn from(value: Vec<&'a Bucket<'_>>) -> Self {
        Self::FhsEntry {
            key: Key::new("/"),
            inner_key: Some(
                value
                    .into_iter()
                    .map(|x| Self::Name(x.name()))
                    .collect::<Vec<_>>(),
            ),
            objects: None,
        }
    }
}
