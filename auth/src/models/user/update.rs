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
