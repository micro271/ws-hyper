use std::{
    collections::HashMap, path::PathBuf, sync::atomic::{AtomicI64, AtomicU8, Ordering}
};

use regex::Regex;
use time::OffsetDateTime;
use tokio::{sync::{RwLock, Semaphore}, fs};
use uuid::Uuid;

const MAX_CREATE_TOKENS: u8 = 2;
const RATE_REFIL: f64 = 1.0;

pub struct CreateRateLimit {
    inner: RwLock<HashMap<Uuid, Bucket>>,
}

impl CreateRateLimit {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn add_client(&self, id: Uuid) {
        let mut inner = self.inner.write().await;
        inner.insert(id, Bucket::new(2, RATE_REFIL));
    }
}

pub struct Bucket {
    capacity: AtomicU8,
    token: AtomicU8,
    rate: f64,
    last_refill: AtomicI64,
    semaphore: Semaphore,
}

impl Bucket {
    pub fn new(size: u8, rate: f64) -> Self {
        Self {
            capacity: AtomicU8::new(size),
            token: AtomicU8::new(MAX_CREATE_TOKENS),
            rate,
            last_refill: AtomicI64::new(OffsetDateTime::now_utc().unix_timestamp()),
            semaphore: Semaphore::const_new(1),
        }
    }

    pub fn set_capacity(&self, size: u8) {
        self.capacity.store(size, Ordering::SeqCst);
    }

    pub async fn refill(&self) {
        let semaphore = self.semaphore.acquire().await;

        if let Err(err) = semaphore {
            tracing::error!(
                "[Refill Bucket] We've updated the token count - Semaphore error: {err}"
            );
            return;
        }

        let time = OffsetDateTime::now_utc().unix_timestamp();
        let tokens_to_add = ((time - self.last_refill.swap(time, Ordering::Relaxed)) as f64
            / self.rate)
            .floor()
            .min(f64::from(u8::MAX)) as u8;
        let capacity = self.capacity.load(Ordering::Relaxed);
        if let Err(err) = self
            .token
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(std::cmp::min(capacity, current + tokens_to_add))
            })
        {
            tracing::error!("[Bucket Refill] We've manage to update the value from: {err}");
        }
    }

    pub fn get_token(&self) -> Result<(), CreateRateError> {
        match self
            .token
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_sub(1)
            }) {
            Ok(_) => Ok(()),
            Err(_) => Err(CreateRateError::BucketEmpty),
        }
    }
}

impl std::default::Default for CreateRateLimit {
    fn default() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }
}

#[derive(Debug)]
pub enum CreateRateError {
    BucketEmpty,
    Update,
}

impl std::fmt::Display for CreateRateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CreateRateError::BucketEmpty => write!(f, "Bucket empty"),
            CreateRateError::Update => write!(f, "Refill error"),
        }
    }
}

impl std::error::Error for CreateRateError {}

pub struct ValidateError;

pub async fn validate_name_and_replace(path: PathBuf, to: &str) -> Result<(), ValidateError> {
    let re = Regex::new(r"(^\.[^.])|(^\.\.)|(\s+)|(^$)").map_err(|_| ValidateError)?;

    if !path.exists() {
        return Err(ValidateError);
    }

    if re.is_match(to) {
        tracing::info!("[Validate Task] {{ Auto rename excecuted }} invalid file name: {to:?}");
        let new_to_file_name = re
            .replace_all(to, |caps: &regex::Captures<'_>| {
                if caps.get(1).is_some() {
                    "[DOT]".to_string()
                } else if caps.get(2).is_some() {
                    "[DOT][DOT]".to_string()
                } else if caps.get(3).is_some() {
                    "_".to_string()
                } else if caps.get(4).is_some() {
                    uuid::Uuid::new_v4().to_string()
                } else {
                    caps.get(0).unwrap().as_str().to_string()
                }
            })
            .to_string();

        let mut path_from = PathBuf::from(&path);
        path_from.push(to);
        let mut path_to = PathBuf::from(&path);
        path_to.push(&new_to_file_name);

        tracing::debug!("[Validate Task] Attempt to rename from: {path_from:?} - to: {path_to:?}");

        if let Err(err) = fs::rename(&path_from, &path_to).await {
            tracing::error!(
                "[Validate Task] Auto rename error from: {path_from:?} to: {path_to:?}, error: {err}"
            );
        }
        tracing::warn!("[Validate Task] Auto rename from: {path_from:?} - to: {path_to:?}");
    }
    Ok(())
}
