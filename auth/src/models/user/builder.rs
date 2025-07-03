use std::collections::HashSet;

use bcrypt::hash;

use crate::models::user::{User, resourece::Resource};

use super::{Role, UserState, Verbs};

pub struct UserBuilder {
    pub username: Option<String>,
    pub passwd: Option<String>,
    pub email: Option<String>,
    pub verbs: Option<HashSet<Verbs>>,
    pub phone: Option<String>,
    pub role: Option<Role>,
    pub user_state: Option<UserState>,
    pub resources: Option<String>,
}

impl UserBuilder {
    fn user_state(mut self, state: UserState) -> Self {
        self.user_state = Some(state);
        self
    }

    fn resources(mut self, res: &str) -> Result<Self, &'static str> {
        let res = Resource::from_str(res)?;
        self.resources = Some(res.to_str());
        Ok(self)
    }

    fn phone(mut self, phone: String) -> Self {
        self.phone = Some(phone);
        self
    }

    fn email(mut self, email: String) -> Self {
        self.email = Some(email);
        self
    }

    fn passwd(mut self, passwd: &str) -> Result<Self, String> {
        self.passwd = Some(hash(passwd, bcrypt::DEFAULT_COST).map_err(|x| x.to_string())?);
        Ok(self)
    }

    fn role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }

    fn username(mut self, username: String) -> Self {
        self.username = Some(username);
        self
    }

    fn verb(mut self, verb: Verbs) -> Self {
        let verbs = verb.level();
        let verbs = verbs.get_normalize();
        if let Some(vec) = self.verbs.as_mut() {
            vec.insert(verb);
        } else {
            self.verbs = Some(HashSet::from([verb]))
        }

        self
    }

    fn verbs(mut self, verbs: Vec<Verbs>) -> Self {
        if let Some(elems) = self.verbs.as_mut() {
            elems.extend(verbs);
        } else {
            self.verbs = Some(verbs.into_iter().collect());
        }
        self
    }

    fn build(self) -> Result<User, &'static str> {
        if self.username.is_none() || self.passwd.is_none() {
            return Err("");
        }

        Ok(User {
            id: None,
            username: self.username.unwrap(),
            passwd: self.passwd.unwrap(),
            email: self.email,
            verbos: self
                .verbs
                .map(|x| x.into_iter().collect::<Vec<Verbs>>())
                .unwrap_or(vec![Default::default()]),
            phone: self.phone,
            user_state: self.user_state.unwrap_or_default(),
            role: self.role.unwrap_or_default(),
            resources: self.resources,
        })
    }
}
