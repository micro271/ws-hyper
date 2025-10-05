use crate::repository::Types;
use std::collections::HashMap;

use super::{Deserialize, Role, UserState};

#[derive(Debug, Deserialize)]
pub struct UpdateUser {
    pub username: Option<String>,
    pub passwd: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub user_state: Option<UserState>,
    pub role: Option<Role>,
    pub resources: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSelf {
    pub passwd: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

impl From<UpdateSelf> for HashMap<&str, Types> {
    fn from(value: UpdateSelf) -> Self {
        [
            ("email", value.email),
            ("phone", value.phone),
            ("passwd", value.passwd),
        ]
        .into_iter()
        .filter_map(|(k, v)| v.map(|x| (k, (!x.is_empty()).then_some(x).into())))
        .collect::<HashMap<&'static str, Types>>()
    }
}

impl From<UpdateUser> for HashMap<&str, Types> {
    fn from(value: UpdateUser) -> Self {
        let role = value.role;
        let state = value.user_state;
        let mut resp = [
            ("username", value.username),
            ("passwd", value.passwd),
            ("email", value.email),
            ("phone", value.phone),
            ("resources", value.resources),
            ("description", value.description),
        ]
        .into_iter()
        .filter_map(|(k, v)| v.map(|x| (k, (!x.is_empty()).then_some(x).into())))
        .collect::<HashMap<&'static str, Types>>();

        if let Some(role) = role {
            resp.insert("role", role.into());
        }
        if let Some(state) = state {
            resp.insert("user_state", state.into());
        }

        resp
    }
}
