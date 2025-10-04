use std::collections::HashMap;

use serde::Deserialize;

use crate::repository::Types;

#[derive(Debug, Deserialize)]
pub struct ProgramaUpdate {
    pub icon: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
}

impl<'a> From<ProgramaUpdate> for HashMap<&'a str, Types> {
    fn from(value: ProgramaUpdate) -> Self {
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
