use serde::Serialize;

use crate::bucket::{Bucket, bucket_map::KeyEntry, object::Object};

#[derive(Serialize)]
#[serde(untagged)]
pub enum Fhs<'a> {
    FhsEntry {
        key: &'a str,
        inner_key: Option<Vec<Fhs<'a>>>,
        objects: Option<&'a Vec<Object>>,
    },
    Name(&'a str),
}

impl<'a> Fhs<'a> {
    pub fn create_fhs(entry: &'a KeyEntry) -> Self {
        Self::FhsEntry {
            key: "",
            inner_key: entry
                .keys
                .as_ref()
                .map(|x| x.values().map(|x| x.into()).collect::<Vec<Fhs<'_>>>()),
            objects: entry.objects.as_ref(),
        }
    }
}

impl<'a> From<&'a KeyEntry> for Fhs<'a> {
    fn from(value: &'a KeyEntry) -> Self {
        Self::FhsEntry {
            key: "",
            inner_key: value
                .keys
                .as_ref()
                .map(|x| x.keys().map(|x| Self::Name(x.name())).collect::<Vec<_>>()),
            objects: value.objects.as_ref(),
        }
    }
}

impl<'a> From<Vec<&'a Bucket<'_>>> for Fhs<'a> {
    fn from(value: Vec<&'a Bucket<'_>>) -> Self {
        Self::FhsEntry {
            key: "/",
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
