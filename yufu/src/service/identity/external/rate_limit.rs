//! Process-local guardrails. The identity service is a single binary; a shared
//! store can replace this if multiple replicas are ever deployed.
use crate::domain::identity::error::IdentityError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const MAX_BUCKETS: usize = 10_000;
const MAX_KEY_BYTES: usize = 512;

struct Window {
    started: Instant,
    count: u32,
}

pub struct ExternalRateLimiter {
    buckets: Mutex<HashMap<String, Window>>,
    upstream: Arc<Semaphore>,
}

impl ExternalRateLimiter {
    pub fn new(upstream_concurrency: usize) -> Result<Self, IdentityError> {
        if upstream_concurrency == 0 || upstream_concurrency > 1_024 {
            return Err(IdentityError::InvalidInput);
        }
        Ok(Self {
            buckets: Mutex::new(HashMap::new()),
            upstream: Arc::new(Semaphore::new(upstream_concurrency)),
        })
    }

    pub fn check(&self, key: &str, limit: u32, window: Duration) -> Result<(), IdentityError> {
        if key.is_empty() || key.len() > MAX_KEY_BYTES || limit == 0 || window.is_zero() {
            return Err(IdentityError::InvalidInput);
        }
        let now = Instant::now();
        let mut buckets = self
            .buckets
            .lock()
            .map_err(|_| IdentityError::DependencyUnavailable)?;
        if buckets.len() >= MAX_BUCKETS {
            buckets.retain(|_, bucket| now.duration_since(bucket.started) < window);
        }
        let bucket = buckets.entry(key.to_owned()).or_insert(Window {
            started: now,
            count: 0,
        });
        if now.duration_since(bucket.started) >= window {
            bucket.started = now;
            bucket.count = 0;
        }
        if bucket.count >= limit {
            return Err(IdentityError::CapacityExceeded);
        }
        bucket.count += 1;
        Ok(())
    }

    pub fn try_acquire_upstream(&self) -> Result<OwnedSemaphorePermit, IdentityError> {
        self.upstream
            .clone()
            .try_acquire_owned()
            .map_err(|_| IdentityError::CapacityExceeded)
    }

    pub fn upstream_available(&self) -> usize {
        self.upstream.available_permits()
    }
}

impl std::fmt::Debug for ExternalRateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalRateLimiter")
            .field("upstream_available", &self.upstream.available_permits())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limiter_is_bounded_per_key_and_window() {
        let limiter = ExternalRateLimiter::new(2).unwrap();
        for _ in 0..3 {
            limiter
                .check("github:login:browser", 3, Duration::from_secs(60))
                .unwrap();
        }
        assert_eq!(
            limiter.check("github:login:browser", 3, Duration::from_secs(60)),
            Err(IdentityError::CapacityExceeded)
        );
        limiter
            .check("github:bind:browser", 3, Duration::from_secs(60))
            .unwrap();
        assert!(limiter.check("", 3, Duration::from_secs(60)).is_err());
        assert!(limiter.check("key", 0, Duration::from_secs(60)).is_err());
        assert!(limiter.check("key", 1, Duration::ZERO).is_err());
    }

    #[test]
    fn limiter_resets_after_window_and_caps_concurrency() {
        let limiter = ExternalRateLimiter::new(1).unwrap();
        limiter
            .check("github:login:browser", 1, Duration::from_millis(50))
            .unwrap();
        assert!(
            limiter
                .check("github:login:browser", 1, Duration::from_millis(50))
                .is_err()
        );
        std::thread::sleep(Duration::from_millis(70));
        limiter
            .check("github:login:browser", 1, Duration::from_millis(50))
            .unwrap();
        let first = limiter.try_acquire_upstream().unwrap();
        assert_eq!(
            limiter.try_acquire_upstream().unwrap_err(),
            IdentityError::CapacityExceeded
        );
        drop(first);
        assert_eq!(limiter.upstream_available(), 1);
        assert!(ExternalRateLimiter::new(0).is_err());
    }
}
