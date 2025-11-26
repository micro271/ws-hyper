use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::Metadata,
    io::Read,
    os::unix::fs::MetadataExt,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use time::{OffsetDateTime, UtcOffset, serde::rfc3339::option};

use crate::bucket::utils::NormalizeForObjectName;

pub const EXTENSION_OBJECT: &str = "__object";

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

macro_rules! impl_canged {
    ($i:ident) => {
        impl crate::state::local_storage::Changed for $i {
            fn change(&self, other: &Self) -> bool {
                other.0.as_ref().and_then(|x| self.0.as_ref().map(|y| y > x)).unwrap_or_default()
            }
        }
    };
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(transparent)]
pub struct ObjectModified(#[serde(with = "option")] Option<OffsetDateTime>);

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(transparent)]
pub struct ObjectCreated(#[serde(with = "option")] Option<OffsetDateTime>);

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(transparent)]
pub struct ObjectAccessed(#[serde(with = "option")] Option<OffsetDateTime>);

from_transparent!(Option<OffsetDateTime>, ObjectModified);
from_transparent!(Option<OffsetDateTime>, ObjectAccessed);
from_transparent!(Option<OffsetDateTime>, ObjectCreated);
default_time!(local ObjectCreated);
default_time!(local ObjectAccessed);
default_time!(local ObjectModified);
impl_canged!(ObjectCreated);
impl_canged!(ObjectAccessed);
impl_canged!(ObjectModified);

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Object {
    pub name: String,
    pub size: i64,
    pub file_name: String,
    pub chechsum: String,
    pub seen_by: Option<Vec<String>>,
    pub taken_by: Option<Vec<String>>,
    pub modified: ObjectModified,
    pub accessed: ObjectAccessed,
    pub created: ObjectCreated,
}

impl Object {
    pub async fn new_from_object<T: AsRef<Path>>(path: T, obj: &Object) -> Self {
        let path = path.as_ref();
        let meta = path.metadata().ok();
        let (modified, accessed, created, size) = get_info_metadata(meta);

        Self {
            modified,
            accessed,
            created,
            size,
            name: obj.name.clone(),
            file_name: obj.file_name.clone(),
            chechsum: obj.chechsum.clone(),
            seen_by: obj.seen_by.clone(),
            taken_by: obj.taken_by.clone(),
        }
    }
    pub async fn new<T>(path: T) -> Self
    where
        T: AsRef<Path>,
    {
        let path = path.as_ref();
        let meta = path.metadata().ok();
        let (modified, accessed, created, size) = get_info_metadata(meta);

        let chechsum = CheckSum::new(path.to_path_buf())
            .check_sum_async()
            .await
            .unwrap();

        let file_name = NormalizeForObjectName::run(&path).await;
        let name = path.file_name().and_then(|x| x.to_str()).map(ToString::to_string).unwrap_or(file_name.clone());

        Self {
            name,
            file_name,
            chechsum,
            size,
            modified,
            accessed,
            created,
            ..Default::default()
        }
    }

    fn name(&self) -> &str {
        &self.name
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
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
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
