use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct Key(String);

impl Key {
    pub fn new<K: Into<String>>(inner: K) -> Self {
        Self(inner.into())
    }
}
