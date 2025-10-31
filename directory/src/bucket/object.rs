use crate::manager::utils::FromDirEntyAsync;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{FileType as FT, Metadata},
    io::Read,
    os::unix::fs::MetadataExt,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use time::{OffsetDateTime, UtcOffset, serde::rfc3339::option};
use tokio::fs;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, PartialOrd)]
pub struct Object {
    key: String,
    r#type: ObjectType,
    size: u64,
    checksum: String,

    #[serde(with = "option")]
    modified: Option<time::OffsetDateTime>,
    #[serde(with = "option")]
    accessed: Option<time::OffsetDateTime>,
    #[serde(with = "option")]
    created: Option<time::OffsetDateTime>,
}

impl Object {
    pub fn is_dir(&self) -> bool {
        self.r#type == ObjectType::Dir
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn file_type(&self) -> ObjectType {
        self.r#type
    }
}

impl FromDirEntyAsync<fs::DirEntry> for Object {
    async fn from_entry(value: fs::DirEntry) -> Self {
        let file_type = value.file_type().await.unwrap();

        let (modified, accessed, created, size) = get_info_metadata(value.metadata().await.ok());

        Self {
            key: value.file_name().to_str().unwrap().to_string(),
            r#type: ObjectType::from(file_type),
            size: size.unwrap_or_default(),
            checksum: CheckSum::new(value.path()).check_sum_async().await,
            modified,
            accessed,
            created,
        }
    }
}

impl<T: AsRef<Path>> From<T> for Object {
    fn from(value: T) -> Self {
        let value = value.as_ref();
        let meta = value.metadata();
        let file_type = meta
            .map(|x| ObjectType::from(x.file_type()))
            .unwrap_or_default();
        let (modified, accessed, created, size) = get_info_metadata(value.metadata().ok());

        Self {
            key: value
                .file_name()
                .map(|x| x.to_string_lossy().into_owned())
                .unwrap(),
            r#type: file_type,
            checksum: CheckSum::new(value).check_sum(),
            size: size.unwrap_or_default(),
            modified,
            accessed,
            created,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, PartialOrd, Default)]
pub enum ObjectType {
    SymLink,
    Regular,
    Dir,
    #[default]
    Unknown,
}

impl From<FT> for ObjectType {
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
pub struct FromEntryToObjectErr;

impl std::fmt::Display for FromEntryToObjectErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "This is not a file")
    }
}

impl std::error::Error for FromEntryToObjectErr {}

pub fn from_systemtime(value: SystemTime) -> OffsetDateTime {
    let tmp = value.duration_since(UNIX_EPOCH).unwrap();
    OffsetDateTime::from_unix_timestamp(tmp.as_secs() as i64)
        .unwrap()
        .to_offset(UtcOffset::from_hms(-3, 0, 0).unwrap())
        .replace_nanosecond(tmp.subsec_nanos())
        .unwrap()
}

impl std::fmt::Display for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key)
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

pub struct CheckSum<T> {
    path: T,
}

impl<T: AsRef<Path>> CheckSum<T> {
    pub fn new(path: T) -> Self {
        Self { path: path }
    }

    fn check_sum(self) -> String {
        let file = std::fs::File::open(self.path).unwrap();
        let mut reader = std::io::BufReader::new(file);
        let mut buffer = [0u8; 8192];
        let mut sha = Sha256::new();
        loop {
            let bits_r = reader.read(&mut buffer);
            match bits_r {
                Ok(0) => {
                    break;
                }
                Ok(bits) => {
                    sha.update(&buffer[..bits]);
                }
                Err(er) => {
                    tracing::error!("{er}");
                    return "".to_string();
                }
            }
        }
        format!("{:x}", sha.finalize())
    }
}

impl<T: AsRef<Path> + Send + 'static> CheckSum<T> {
    pub async fn check_sum_async(self) -> String {
        tokio::task::spawn_blocking(|| self.check_sum())
            .await
            .unwrap()
    }
}
