pub mod channel;
pub mod program;
pub mod user;

use serde::{Deserialize, Serialize};

use crate::models::user::{UserState, Verbs};

#[derive(Debug, Deserialize, Serialize)]
pub struct GetUserPubAll {
    username: String,
    email: String,
    role: String,
    state: UserState,
    phone: String,
    verbs: Verbs,
    resources: String,
    user_description: String,
    program: String,
    program_icon: String,
    program_description: String,
    channel: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetUserOwn {
    username: String,
    email: String,
    phone: String,
    program_icon: String,
    program_name: String,
    channel: String,
}
