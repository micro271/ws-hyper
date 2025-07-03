use std::collections::HashSet;

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
}
