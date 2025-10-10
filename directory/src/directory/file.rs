use std::{borrow::Cow, fs::{DirEntry,FileType as FT}, os::unix::fs::MetadataExt, time::{SystemTime, UNIX_EPOCH}};

use serde::Deserialize;
use time::{format_description::modifier, OffsetDateTime};


#[derive(Debug, Deserialize)]
pub struct File<'a> {
    name: Cow<'a, str>,
    r#type: FileType,
    size: u64,
    modified: time::OffsetDateTime,
    accessed: time::OffsetDateTime,
    created: time::OffsetDateTime,
}

impl<'a> TryFrom<DirEntry> for File<'a> {
    type Error = FromEntryToFileErr;
    fn try_from(value: DirEntry) -> Result<Self, Self::Error> {
        let file_type = value.file_type().unwrap();

        if file_type.is_dir() {
            return Err(FromEntryToFileErr);
        }
        let metadata = value.metadata().unwrap();

        Ok(
            Self {
                name: Cow::Owned(value.file_name().to_str().unwrap().to_string()),
                r#type: FileType::from(file_type),
                size: metadata.size(),
                modified: from_systemtime(metadata.modified().unwrap()),
                accessed: from_systemtime(metadata.accessed().unwrap()),
                created: from_systemtime(metadata.created().unwrap()),
            }
        )
    }
}

#[derive(Debug, Deserialize)]
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
    OffsetDateTime::from_unix_timestamp(tmp.as_secs() as i64).unwrap().replace_nanosecond(tmp.subsec_nanos()).unwrap()
}