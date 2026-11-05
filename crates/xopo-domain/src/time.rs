//! Wall-clock timestamps.
//!
//! Timestamps are stored as signed milliseconds since the Unix epoch in UTC.
//! The domain keeps no timezone, locale or display formatting: presentation
//! layers format timestamps for the user.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::DomainError;

/// A moment in time, as milliseconds since the Unix epoch (UTC).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Builds a timestamp from milliseconds since the Unix epoch.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::TimestampOutOfRange`] for negative values.
    pub fn from_millis(millis: i64) -> Result<Self, DomainError> {
        if millis < 0 {
            return Err(DomainError::TimestampOutOfRange);
        }
        Ok(Self(millis))
    }

    /// Reads the current system clock.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::ClockBeforeEpoch`] when the clock is set before
    /// 1970-01-01, which indicates a broken or manipulated system clock.
    pub fn now() -> Result<Self, DomainError> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| DomainError::ClockBeforeEpoch)?;
        let millis =
            i64::try_from(elapsed.as_millis()).map_err(|_| DomainError::TimestampOutOfRange)?;
        Ok(Self(millis))
    }

    /// Milliseconds since the Unix epoch.
    #[must_use]
    pub fn as_millis(&self) -> i64 {
        self.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = i64::deserialize(deserializer)?;
        Self::from_millis(millis).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_timestamps_are_rejected() {
        assert_eq!(
            Timestamp::from_millis(-1),
            Err(DomainError::TimestampOutOfRange)
        );
    }

    #[test]
    fn negative_timestamps_are_rejected_during_deserialization() {
        let result = serde_json::from_str::<Timestamp>("-5");
        assert!(result.is_err());
    }

    #[test]
    fn now_is_after_the_epoch() {
        let now = Timestamp::now().expect("clock after epoch");
        assert!(now.as_millis() > 0);
    }
}
