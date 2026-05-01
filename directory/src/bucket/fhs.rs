use serde::Serialize;

use crate::bucket::{
    Bucket, Cowed,
    bucket_map::{BucketMap, KeyEntry},
    key::Segment,
    object::Object,
};

#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum Fhs<'a> {
    Branch {
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<Segment<'a>>,
        inner_key: Option<Vec<Fhs<'a>>>,
        objects: Option<&'a Vec<Object>>,
    },
    Leaf(Segment<'a>),
}

impl<'a> Fhs<'a> {
    pub fn create_branch(key: Option<Segment<'a>>, entry: &'a KeyEntry) -> Self {
        Self::Branch {
            key: key,
            inner_key: entry.keys.as_ref().map(|x| {
                x.iter()
                    .map(|(k, v)| Fhs::create_branch(Some(k.borrow()), v))
                    .collect::<Vec<Fhs<'_>>>()
            }),
            objects: entry.objects.as_ref(),
        }
    }
}

impl<'a> From<&'a KeyEntry> for Fhs<'a> {
    fn from(value: &'a KeyEntry) -> Self {
        Self::Branch {
            key: None,
            inner_key: value
                .keys
                .as_ref()
                .map(|x| x.keys().map(|x| Self::Leaf(x.borrow())).collect::<Vec<_>>()),
            objects: value.objects.as_ref(),
        }
    }
}

impl<'a> From<Vec<&'a Bucket<'_>>> for Fhs<'a> {
    fn from(value: Vec<&'a Bucket<'_>>) -> Self {
        Self::Branch {
            key: None,
            inner_key: Some(
                value
                    .into_iter()
                    .map(|x| Self::Leaf(x.into()))
                    .collect::<Vec<_>>(),
            ),
            objects: None,
        }
    }
}

impl<'a> From<&'a BucketMap> for Fhs<'a> {
    fn from(value: &'a BucketMap) -> Self {
        Self::Branch {
            key: Some(Segment::new("/")),
            inner_key: Some(
                value
                    .tree
                    .iter()
                    .map(|(k, v)| Fhs::create_branch(Some(k.into()), v))
                    .collect::<Vec<_>>(),
            ),
            objects: None,
        }
    }
}
