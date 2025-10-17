use serde::{Deserialize, Serialize};
use std::{
    fs::{DirEntry, FileType as FT},
    os::unix::fs::MetadataExt,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use time::{OffsetDateTime, UtcOffset, serde::rfc3339};
use tokio::fs;

use crate::manager::utils::FromDirEntyAsync;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, PartialOrd)]
pub struct File {
    name: String,
    r#type: FileType,
    size: u64,

    #[serde(with = "rfc3339")]
    modified: time::OffsetDateTime,
    #[serde(with = "rfc3339")]
    accessed: time::OffsetDateTime,
    #[serde(with = "rfc3339")]
    created: time::OffsetDateTime,
}

impl File {
    pub fn id_dir(&self) -> bool {
        self.r#type == FileType::Dir
    }

    pub fn file_name(&self) -> &str {
        &self.name
    }
}

impl FromDirEntyAsync<fs::DirEntry> for File {
    fn from_entry(value: fs::DirEntry) -> impl Future<Output = Self> {
        async move {
            let file_type = value.file_type().await.unwrap();
            let metadata = value.metadata().await.unwrap();

            Self {
                name: value.file_name().to_str().unwrap().to_string(),
                r#type: FileType::from(file_type),
                size: metadata.size(),
                modified: from_systemtime(metadata.modified().unwrap()),
                accessed: from_systemtime(metadata.accessed().unwrap()),
                created: from_systemtime(metadata.created().unwrap()),
            }
        }
    }
}

impl From<&PathBuf> for File {
    fn from(value: &PathBuf) -> Self {
        let meta = value.metadata().unwrap();
        let file_type = meta.file_type();

        Self {
            name: value
                .file_name()
                .and_then(|x| x.to_str().map(ToString::to_string))
                .unwrap(),
            r#type: FileType::from(file_type),
            size: meta.size(),
            modified: from_systemtime(meta.modified().unwrap()),
            accessed: from_systemtime(meta.accessed().unwrap()),
            created: from_systemtime(meta.created().unwrap()),
        }
    }
}

impl TryFrom<DirEntry> for File {
    type Error = FromEntryToFileErr;
    fn try_from(value: DirEntry) -> Result<Self, Self::Error> {
        let file_type = value.file_type().unwrap();

        if file_type.is_dir() {
            return Err(FromEntryToFileErr);
        }
        let metadata = value.metadata().unwrap();

        Ok(Self {
            name: value.file_name().to_str().unwrap().to_string(),
            r#type: FileType::from(file_type),
            size: metadata.size(),
            modified: from_systemtime(metadata.modified().unwrap()),
            accessed: from_systemtime(metadata.accessed().unwrap()),
            created: from_systemtime(metadata.created().unwrap()),
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, PartialOrd)]
enum FileType {
    SymLink,
    Regular,
    Dir,
}

impl From<FT> for FileType {
    fn from(value: FT) -> Self {
        if value.is_dir() {
            Self::Dir
        } else if value.is_file() {
            Self::Regular
        } else {
            Self::SymLink
        }
    }
}

#[derive(Debug)]
pub struct FromEntryToFileErr;

impl std::fmt::Display for FromEntryToFileErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "This is not a file")
    }
}

impl std::error::Error for FromEntryToFileErr {}

pub fn from_systemtime(value: SystemTime) -> OffsetDateTime {
    let tmp = value.duration_since(UNIX_EPOCH).unwrap();
    OffsetDateTime::from_unix_timestamp(tmp.as_secs() as i64)
        .unwrap()
        .to_offset(UtcOffset::from_hms(-3, 0, 0).unwrap())
        .replace_nanosecond(tmp.subsec_nanos())
        .unwrap()
}

impl std::fmt::Display for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
