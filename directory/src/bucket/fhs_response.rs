use serde::Serialize;

use crate::bucket::object::Object;

#[derive(Debug, Serialize)]
pub struct FhsResponse<'a> {
    key: &'a str,
    inner_key: Option<Vec<&'a str>>,
    objects: Option<&'a Vec<Object>>,
}

impl<'a> FhsResponse<'a> {
    pub fn new(key: &'a str, inner_key: Vec<&'a str>, objects: &'a Vec<Object>) -> Self {
        Self {
            key,
            inner_key: (!inner_key.is_empty()).then_some(inner_key),
            objects: (!objects.is_empty()).then_some(objects),
        }
    }
}
