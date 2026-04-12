use serde::Serialize;

use crate::bucket::object::Object;

#[derive(Debug, Serialize)]
pub struct FhsResponse<'a> {
    inner_key: Option<Vec<&'a str>>,
    objects: Option<&'a Vec<Object>>,
}

impl<'a> FhsResponse<'a> {
    pub fn new(inner_key: Vec<&'a str>, objects: Option<&'a Vec<Object>>) -> Self {
        Self {
            inner_key: (!inner_key.is_empty()).then_some(inner_key),
            objects: objects.filter(|x| !x.is_empty()),
        }
    }
}
