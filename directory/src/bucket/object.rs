use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    fs::Metadata,
    io::Read,
    os::unix::fs::MetadataExt,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use time::{OffsetDateTime, UtcOffset, serde::rfc3339::option};

macro_rules! from_transparent {
    ($from: ty, $to: ident) => {
        impl From<$from> for $to {
            fn from(value: $from) -> $to {
                $to(value)
            }
        }
    };
}

macro_rules! default_time {
    (local $to: ident) => {
        impl std::default::Default for $to {
            fn default() -> $to {
                $to(Some(OffsetDateTime::now_local().unwrap()))
            }
        }
    };

    ($to: ident) => {
        impl std::default::Default for $to {
            fn default() -> $to {
                $to(Some(OffsetDateTime::now()))
            }
        }
    };
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize, Default)]
pub struct ObjectName<'a>(Cow<'a, str>);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ObjectModified(#[serde(with = "option")] Option<OffsetDateTime>);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ObjectCreated(#[serde(with = "option")] Option<OffsetDateTime>);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
#[serde(transparent)]
pub struct ObjectAccessed(#[serde(with = "option")] Option<OffsetDateTime>);

from_transparent!(Option<OffsetDateTime>, ObjectModified);
from_transparent!(Option<OffsetDateTime>, ObjectAccessed);
from_transparent!(Option<OffsetDateTime>, ObjectCreated);
default_time!(local ObjectCreated);
default_time!(local ObjectAccessed);
default_time!(local ObjectModified);

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Object {
    pub _id: String,
    pub size: i64,
    pub name: String,
    pub seen_by: Option<Vec<String>>,
    pub taken_by: Option<Vec<String>>,
    pub deleted_by: Option<String>,
    pub modified: ObjectModified,
    pub accessed: ObjectAccessed,
    pub created: ObjectCreated,
}

impl Object {
    pub async fn new<T>(path: T) -> Self
    where
        T: AsRef<Path>,
    {
        let path = path.as_ref();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let hash = CheckSum::new(path.to_path_buf())
            .check_sum_async()
            .await
            .unwrap();
        let meta = path.metadata().ok();
        let (modified, accessed, created, size) = get_info_metadata(meta);

        Self {
            name,
            _id: hash,
            size,
            modified,
            accessed,
            created,
            ..Default::default()
        }
    }
}

impl<T: AsRef<Path>> From<T> for Object {
    fn from(value: T) -> Self {
        let value = value.as_ref();

        let (modified, accessed, created, size) = get_info_metadata(value.metadata().ok());

        Self {
            _id: CheckSum::new(value).check_sum().unwrap(),
            name: value
                .file_name()
                .map(|x| x.to_string_lossy().into_owned())
                .unwrap(),
            size,
            modified,
            accessed,
            created,
            ..Default::default()
        }
    }
}

impl<'a, T: AsRef<Path>> From<T> for ObjectName<'a> {
    fn from(value: T) -> Self {
        ObjectName(Cow::Owned(
            value
                .as_ref()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        ))
    }
}

impl<'a> std::ops::Deref for ObjectName<'a> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl<'a> std::fmt::Display for ObjectName<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn get_info_metadata(
    meta: Option<Metadata>,
) -> (ObjectModified, ObjectAccessed, ObjectCreated, i64) {
    match meta {
        Some(meta) => (
            meta.modified().map(from_systemtime).ok().into(),
            meta.accessed().map(from_systemtime).ok().into(),
            meta.created().map(from_systemtime).ok().into(),
            meta.size() as i64,
        ),
        _ => (
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        ),
    }
}

pub fn from_systemtime(value: SystemTime) -> OffsetDateTime {
    let tmp = value.duration_since(UNIX_EPOCH).unwrap();
    OffsetDateTime::from_unix_timestamp(tmp.as_secs() as i64)
        .unwrap()
        .to_offset(UtcOffset::from_hms(-3, 0, 0).unwrap())
        .replace_nanosecond(tmp.subsec_nanos())
        .unwrap()
}

pub struct CheckSum<T> {
    path: T,
}

impl<T: AsRef<Path>> CheckSum<T> {
    pub fn new(path: T) -> Self {
        Self { path: path }
    }

    fn check_sum(self) -> std::io::Result<String> {
        let file = std::fs::File::open(self.path)?;
        let mut reader = std::io::BufReader::new(file);
        let mut buffer = [0u8; 8192];
        let mut sha = Sha256::new();
        loop {
            let bits @ 1.. = reader.read(&mut buffer)? else {
                break;
            };

            sha.update(&buffer[..bits])
        }

        Ok(format!("{:x}", sha.finalize()))
    }
}

impl<T: AsRef<Path> + Send + 'static> CheckSum<T> {
    pub async fn check_sum_async(self) -> std::io::Result<String> {
        tokio::task::spawn_blocking(|| self.check_sum())
            .await
            .unwrap()
    }
}
