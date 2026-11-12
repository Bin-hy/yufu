use std::{
    future::Future,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use super::error::SecurityError;

pub const MAX_SECURITY_BUDGET: Duration = Duration::from_secs(2);

/// A coherent reading from a trusted server clock. Monotonic time must never go back.
#[derive(Debug, Clone, Copy)]
pub struct ClockReading {
    pub monotonic: Duration,
    pub unix_millis: u64,
    pub synchronized: bool,
}

pub trait Clock: Send + Sync {
    fn now(&self) -> Result<ClockReading, SecurityError>;
}

/// The caller must supply its clock-health monitor; synchronization is never assumed.
pub struct SystemClock {
    origin: tokio::time::Instant,
    health: Arc<dyn Fn() -> bool + Send + Sync>,
}
impl SystemClock {
    pub fn new(health: Arc<dyn Fn() -> bool + Send + Sync>) -> Self {
        Self {
            origin: tokio::time::Instant::now(),
            health,
        }
    }
}
impl Clock for SystemClock {
    fn now(&self) -> Result<ClockReading, SecurityError> {
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SecurityError::DependencyUnavailable)?;
        Ok(ClockReading {
            monotonic: self.origin.elapsed(),
            unix_millis: u64::try_from(unix.as_millis())
                .map_err(|_| SecurityError::DependencyUnavailable)?,
            synchronized: (self.health)(),
        })
    }
}

pub(crate) fn healthy_reading(clock: &dyn Clock) -> Result<ClockReading, SecurityError> {
    let reading = clock.now()?;
    if !reading.synchronized {
        return Err(SecurityError::DependencyUnavailable);
    }
    Ok(reading)
}

/// Request-local deadline. This type is not a wire timestamp and cannot be deserialized.
#[derive(Clone)]
pub struct SecurityBudget {
    clock: Arc<dyn Clock>,
    started_at: Duration,
    deadline: Duration,
}
impl SecurityBudget {
    pub fn new(clock: Arc<dyn Clock>) -> Result<Self, SecurityError> {
        let now = healthy_reading(clock.as_ref())?;
        let deadline = now
            .monotonic
            .checked_add(MAX_SECURITY_BUDGET)
            .ok_or(SecurityError::DeadlineExceeded)?;
        Ok(Self {
            clock,
            started_at: now.monotonic,
            deadline,
        })
    }

    pub fn remaining(&self) -> Result<Duration, SecurityError> {
        let now = healthy_reading(self.clock.as_ref())?;
        self.remaining_at(now)
    }

    fn remaining_at(&self, now: ClockReading) -> Result<Duration, SecurityError> {
        if now.monotonic < self.started_at {
            return Err(SecurityError::DependencyUnavailable);
        }
        self.deadline
            .checked_sub(now.monotonic)
            .filter(|left| !left.is_zero())
            .ok_or(SecurityError::DeadlineExceeded)
    }

    /// A remote remaining-budget header may only tighten the local inherited deadline.
    pub fn child(&self, requested_remaining: Duration) -> Result<Self, SecurityError> {
        let now = healthy_reading(self.clock.as_ref())?;
        let left = self.remaining_at(now)?.min(requested_remaining);
        if left.is_zero() {
            return Err(SecurityError::DeadlineExceeded);
        }
        Ok(Self {
            clock: self.clock.clone(),
            started_at: now.monotonic,
            deadline: now
                .monotonic
                .checked_add(left)
                .ok_or(SecurityError::DeadlineExceeded)?,
        })
    }

    /// Restart this budget's original duration after a separately bounded CPU
    /// step. A child budget keeps the duration it was granted, so a remote
    /// remaining-budget header is never widened to the local maximum.
    pub fn renewed(&self) -> Result<Self, SecurityError> {
        let now = healthy_reading(self.clock.as_ref())?;
        let duration = self
            .deadline
            .checked_sub(self.started_at)
            .ok_or(SecurityError::DeadlineExceeded)?
            .min(MAX_SECURITY_BUDGET);
        if duration.is_zero() {
            return Err(SecurityError::DeadlineExceeded);
        }
        Ok(Self {
            clock: self.clock.clone(),
            started_at: now.monotonic,
            deadline: now
                .monotonic
                .checked_add(duration)
                .ok_or(SecurityError::DeadlineExceeded)?,
        })
    }

    /// Bound a security dependency wait, not the physical device operation itself.
    pub async fn run<F: Future>(&self, future: F) -> Result<F::Output, SecurityError> {
        let left = self.remaining()?;
        let result = tokio::time::timeout(left, future)
            .await
            .map_err(|_| SecurityError::DeadlineExceeded)?;
        self.remaining()?;
        Ok(result)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    pub(crate) struct ManualClock(pub Mutex<ClockReading>);
    impl ManualClock {
        pub fn new() -> Self {
            Self(Mutex::new(ClockReading {
                monotonic: Duration::ZERO,
                unix_millis: 100_000,
                synchronized: true,
            }))
        }
        pub fn advance(&self, millis: u64) {
            let mut r = self.0.lock().unwrap();
            r.monotonic += Duration::from_millis(millis);
            r.unix_millis += millis;
        }
    }
    impl Clock for ManualClock {
        fn now(&self) -> Result<ClockReading, SecurityError> {
            Ok(*self.0.lock().unwrap())
        }
    }

    #[test]
    fn security_t05_queue_and_children_share_original_deadline() {
        let clock = Arc::new(ManualClock::new());
        let parent = SecurityBudget::new(clock.clone()).unwrap();
        clock.advance(700);
        let child = parent.child(Duration::from_secs(30)).unwrap();
        assert_eq!(child.remaining().unwrap(), Duration::from_millis(1300));
        clock.advance(1300);
        assert_eq!(child.remaining(), Err(SecurityError::DeadlineExceeded));
        assert_eq!(parent.remaining(), Err(SecurityError::DeadlineExceeded));
    }

    #[test]
    fn security_t05_child_cannot_extend_shorter_deadline_or_use_unhealthy_clock() {
        let clock = Arc::new(ManualClock::new());
        let parent = SecurityBudget::new(clock.clone()).unwrap();
        let child = parent.child(Duration::from_millis(100)).unwrap();
        clock.advance(100);
        assert_eq!(
            child.child(Duration::from_secs(2)).err(),
            Some(SecurityError::DeadlineExceeded)
        );
        clock.0.lock().unwrap().synchronized = false;
        assert_eq!(
            parent.remaining(),
            Err(SecurityError::DependencyUnavailable)
        );
    }

    #[test]
    fn security_t05_renewed_child_keeps_its_granted_duration() {
        let clock = Arc::new(ManualClock::new());
        let parent = SecurityBudget::new(clock.clone()).unwrap();
        let child = parent.child(Duration::from_millis(100)).unwrap();
        clock.advance(50);
        let renewed = child.renewed().unwrap();
        assert_eq!(renewed.remaining().unwrap(), Duration::from_millis(100));
        clock.advance(100);
        assert_eq!(renewed.remaining(), Err(SecurityError::DeadlineExceeded));
    }

    #[test]
    fn security_t05_wall_clock_rollback_does_not_restore_budget() {
        let clock = Arc::new(ManualClock::new());
        let budget = SecurityBudget::new(clock.clone()).unwrap();
        clock.advance(2000);
        clock.0.lock().unwrap().unix_millis = 1;
        assert_eq!(budget.remaining(), Err(SecurityError::DeadlineExceeded));
    }

    #[tokio::test(start_paused = true)]
    async fn security_t05_async_wait_is_bounded() {
        let clock = Arc::new(SystemClock::new(Arc::new(|| true)));
        let budget = SecurityBudget::new(clock).unwrap();
        assert_eq!(
            budget.run(tokio::time::sleep(Duration::from_secs(3))).await,
            Err(SecurityError::DeadlineExceeded)
        );
    }
}
