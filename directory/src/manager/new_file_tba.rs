use std::{
    collections::HashMap,
    sync::atomic::{AtomicI64, AtomicU8, Ordering},
};

use time::OffsetDateTime;
use tokio::sync::{RwLock, Semaphore};
use uuid::Uuid;

const MAX_CREATE_TOKENS: u8 = 2;
const RATE_REFIL: f64 = 1.0;

#[derive(Debug)]
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

#[derive(Debug)]
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
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                Some(std::cmp::min(capacity, current + tokens_to_add))
            })
        {
            tracing::error!("[Bucket Refill] We've manage to update the value from: {err}");
        }
    }

    pub fn get_token(&self) -> Result<(), CreateRateError> {
        if self
            .token
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                current.checked_sub(1)
            })
            .is_err()
        {
            tracing::error!("Error to obtaine one token");
            Err(CreateRateError::BucketEmpty)
        } else {
            Ok(())
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
