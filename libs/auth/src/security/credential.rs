use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};

use super::{
    budget::{Clock, ClockReading, healthy_reading},
    error::SecurityError,
};

pub const MAX_DECISION_AGE_MILLIS: u64 = 5000;
pub const CLOCK_ERROR_MILLIS: u64 = 1000;

/// A freshness interval, not proof of a successful authorization or a signature.
/// Its source must be authenticated before it can authorize any operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ValidityWire")]
pub struct DecisionValidity {
    checked_at: u64,
    valid_until: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidityWire {
    checked_at: u64,
    valid_until: u64,
}

impl TryFrom<ValidityWire> for DecisionValidity {
    type Error = SecurityError;
    fn try_from(value: ValidityWire) -> Result<Self, Self::Error> {
        let upper = value
            .checked_at
            .checked_add(MAX_DECISION_AGE_MILLIS)
            .ok_or(SecurityError::InvalidCredential)?;
        if value.valid_until <= value.checked_at || value.valid_until > upper {
            return Err(SecurityError::InvalidCredential);
        }
        Ok(Self {
            checked_at: value.checked_at,
            valid_until: value.valid_until,
        })
    }
}

impl DecisionValidity {
    /// checked_at is captured before the source's consistent primary-database query.
    pub fn from_query_start(
        checked_at: u64,
        credential_expires: u64,
        parent_expires: Option<u64>,
        inherited: Option<&Self>,
    ) -> Result<Self, SecurityError> {
        let mut valid_until = checked_at
            .checked_add(MAX_DECISION_AGE_MILLIS)
            .ok_or(SecurityError::InvalidCredential)?
            .min(credential_expires);
        if let Some(parent) = parent_expires {
            valid_until = valid_until.min(parent);
        }
        if let Some(original) = inherited {
            valid_until = valid_until.min(original.valid_until);
        }
        if valid_until <= checked_at {
            return Err(SecurityError::DeadlineExceeded);
        }
        Ok(Self {
            checked_at,
            valid_until,
        })
    }
    pub fn checked_at(&self) -> u64 {
        self.checked_at
    }
    pub fn valid_until(&self) -> u64 {
        self.valid_until
    }

    fn remaining_at(&self, reading: ClockReading) -> Result<Duration, SecurityError> {
        if !reading.synchronized {
            return Err(SecurityError::DependencyUnavailable);
        }
        let conservative_now = reading
            .unix_millis
            .checked_add(CLOCK_ERROR_MILLIS)
            .ok_or(SecurityError::DependencyUnavailable)?;
        if conservative_now < self.checked_at {
            return Err(SecurityError::DependencyUnavailable);
        }
        let millis = self
            .valid_until
            .checked_sub(conservative_now)
            .filter(|left| *left > 0)
            .ok_or(SecurityError::DeadlineExceeded)?;
        Ok(Duration::from_millis(millis))
    }

    /// Pin a received interval to the local monotonic clock once, on cache insertion.
    /// Cache hits reuse the returned object, never recreate its duration from now.
    pub fn bind_to_clock(&self, clock: Arc<dyn Clock>) -> Result<ActiveValidity, SecurityError> {
        let reading = healthy_reading(clock.as_ref())?;
        let left = self.remaining_at(reading)?;
        let deadline = reading
            .monotonic
            .checked_add(left)
            .ok_or(SecurityError::DeadlineExceeded)?;
        Ok(ActiveValidity {
            clock,
            validity: *self,
            bound_at: reading.monotonic,
            deadline,
        })
    }
}

#[derive(Clone)]
pub struct ActiveValidity {
    clock: Arc<dyn Clock>,
    validity: DecisionValidity,
    bound_at: Duration,
    deadline: Duration,
}
impl ActiveValidity {
    pub fn remaining(&self) -> Result<Duration, SecurityError> {
        let reading = healthy_reading(self.clock.as_ref())?;
        if reading.monotonic < self.bound_at {
            return Err(SecurityError::DependencyUnavailable);
        }
        let mono_left = self
            .deadline
            .checked_sub(reading.monotonic)
            .filter(|left| !left.is_zero())
            .ok_or(SecurityError::DeadlineExceeded)?;
        Ok(mono_left.min(self.validity.remaining_at(reading)?))
    }
    pub fn validity(&self) -> DecisionValidity {
        self.validity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::budget::tests::ManualClock;

    #[test]
    fn security_t06_parent_expiry_and_original_window_bound_decision() {
        let validity =
            DecisionValidity::from_query_start(100_000, 200_000, Some(103_000), None).unwrap();
        assert_eq!(validity.valid_until(), 103_000);
        let refreshed =
            DecisionValidity::from_query_start(101_000, 200_000, None, Some(&validity)).unwrap();
        assert_eq!(refreshed.valid_until(), 103_000);
        assert!(DecisionValidity::from_query_start(u64::MAX - 1, u64::MAX, None, None).is_err());
    }

    #[test]
    fn security_t06_delayed_response_and_expiry_have_no_grace() {
        let clock = Arc::new(ManualClock::new());
        let validity = DecisionValidity::from_query_start(100_000, 200_000, None, None).unwrap();
        assert_eq!(validity.valid_until(), 105_000);
        clock.advance(4000);
        assert!(validity.bind_to_clock(clock).is_err());
        let invalid = serde_json::json!({"checked_at":100_000,"valid_until":110_000});
        assert!(serde_json::from_value::<DecisionValidity>(invalid).is_err());
    }

    #[test]
    fn security_t06_cache_cannot_survive_wall_rollback_or_rebind_expired_decision() {
        let clock = Arc::new(ManualClock::new());
        let validity = DecisionValidity::from_query_start(100_000, 200_000, None, None).unwrap();
        let bound = validity.bind_to_clock(clock.clone()).unwrap();
        clock.advance(3999);
        assert_eq!(bound.remaining().unwrap(), Duration::from_millis(1));
        clock.advance(1);
        clock.0.lock().unwrap().unix_millis = 100_000;
        assert_eq!(bound.remaining(), Err(SecurityError::DeadlineExceeded));
        clock.0.lock().unwrap().unix_millis = 106_000;
        assert!(validity.bind_to_clock(clock).is_err());
    }

    #[test]
    fn security_t06_future_source_and_unhealthy_clock_fail_closed() {
        let clock = Arc::new(ManualClock::new());
        let future = DecisionValidity::from_query_start(110_000, 200_000, None, None).unwrap();
        assert!(future.bind_to_clock(clock.clone()).is_err());
        clock.0.lock().unwrap().synchronized = false;
        let valid = DecisionValidity::from_query_start(100_000, 200_000, None, None).unwrap();
        assert!(valid.bind_to_clock(clock).is_err());
    }
}
