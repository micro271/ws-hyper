use serde::Deserialize;
use sqlx::prelude::FromRow;
use uuid::Uuid;

use crate::repository::{Table, Types};


#[derive(Debug, FromRow, Deserialize)]
pub struct Files {
    pub id: Uuid,
    pub create_at: time::OffsetDateTime,
    pub stem: String,
    pub extension: String,
    pub elapsed_upload: Option<usize>,
    pub id_tvshow: Uuid,
}

impl Table for Files {
    fn name() -> &'static str {
        "files"
    }

    fn columnds_value(self) -> Vec<Types> {
        vec![
            self.id.into(),
            self.create_at.into(),
            self.stem.into(),
            self.extension.into(),
            self.elapsed_upload.into(),
            self.id_tvshow.into(),
        ]
    }

    fn columns_name() -> Vec<&'static str> {
        vec![
            "id",
            "create_at",
            "stem",
            "extension",
            "elapse_upload",
            "id_tvshow",
        ]
    }
}