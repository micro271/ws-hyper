use serde::{Deserialize, Serialize};



#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key(String);