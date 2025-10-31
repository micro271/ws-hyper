use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct Key(String);

impl Key {
    pub fn new<K: Into<String>>(inner: K) -> Self {
        Self(inner.into())
    }

    pub fn inner(self) -> String {
        self.0
    }
}

impl AsRef<str> for Key {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
