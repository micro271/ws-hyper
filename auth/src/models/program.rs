use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    models::user::User,
    repository::{InnerJoin, TableName},
};

#[derive(Debug, Deserialize, Serialize)]
pub struct Programa {
    pub id: Uuid,
    pub icon: String,
    pub user_id: Uuid,
    pub name: String,
    pub description: String,
}

impl TableName for Programa {
    fn name() -> &'static str {
        "programs"
    }
}

impl InnerJoin<User> for Programa {
    fn fields() -> String {
        let name = Programa::name();
        format!("{name}.id , {name}.name , {name}.icon , {name}.description")
    }
}
