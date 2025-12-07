use std::collections::HashMap;

use serde::Deserialize;

use crate::state::Types;

#[derive(Debug, Deserialize)]
pub struct BucketUpdate {
    pub icon: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
}

impl From<BucketUpdate> for HashMap<&str, Types> {
    fn from(value: BucketUpdate) -> Self {
        [
            ("icon", value.icon),
            ("name", value.name),
            ("description", value.description),
        ]
        .into_iter()
        .filter_map(|(k, v)| v.map(|x| (k, (!x.is_empty()).then_some(x).into())))
        .collect::<HashMap<&_, Types>>()
    }
}
