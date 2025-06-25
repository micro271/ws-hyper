use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct UploadLog {
    pub file_name: String,
    pub channel: String,
    pub program_tv: String,
    pub elapsed_upload: Option<u64>,
    pub size: usize,
}
