use serde::{Deserialize, Serialize};
use std::{
    fs::{FileType as FT, Metadata},
    os::unix::fs::MetadataExt,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use time::{OffsetDateTime, UtcOffset, serde::rfc3339::option};
use tokio::fs;

use crate::manager::utils::FromDirEntyAsync;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, PartialOrd)]
pub struct File {
    name: String,
    r#type: FileType,
    size: u64,

    #[serde(with = "option")]
    modified: Option<time::OffsetDateTime>,
    #[serde(with = "option")]
    accessed: Option<time::OffsetDateTime>,
    #[serde(with = "option")]
    created: Option<time::OffsetDateTime>,
}

impl File {
    pub fn is_dir(&self) -> bool {
        self.r#type == FileType::Dir
    }

    pub fn file_name(&self) -> &str {
        &self.name
    }

    pub fn file_type(&self) -> FileType {
        self.r#type
    }
}

impl FromDirEntyAsync<fs::DirEntry> for File {
    async fn from_entry(value: fs::DirEntry) -> Self {
        let file_type = value.file_type().await.unwrap();

        let (modified, accessed, created, size) = get_info_metadata(value.metadata().await.ok());

        Self {
            name: value.file_name().to_str().unwrap().to_string(),
            r#type: FileType::from(file_type),
            size: size.unwrap_or_default(),
            modified,
            accessed,
            created,
        }
    }
}

impl From<&PathBuf> for File {
    fn from(value: &PathBuf) -> Self {
        let meta = value.metadata();
        let file_type = meta
            .map(|x| FileType::from(x.file_type()))
            .unwrap_or_default();
        let (modified, accessed, created, size) = get_info_metadata(value.metadata().ok());

        Self {
            name: value
                .file_name()
                .and_then(|x| x.to_str().map(ToString::to_string))
                .unwrap(),
            r#type: file_type,
            size: size.unwrap_or_default(),
            modified,
            accessed,
            created,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, PartialOrd, Default)]
pub enum FileType {
    SymLink,
    Regular,
    Dir,
    #[default]
    Unknown,
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

fn get_info_metadata(
    meta: Option<Metadata>,
) -> (
    Option<OffsetDateTime>,
    Option<OffsetDateTime>,
    Option<OffsetDateTime>,
    Option<u64>,
) {
    match meta {
        Some(meta) => (
            meta.modified().map(from_systemtime).ok(),
            meta.accessed().map(from_systemtime).ok(),
            meta.created().map(from_systemtime).ok(),
            Some(meta.size()),
        ),
        _ => (None, None, None, None),
    }
}
