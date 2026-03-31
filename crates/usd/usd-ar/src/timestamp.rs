//! Asset timestamp type.
//!
//! Represents a timestamp for an asset, typically used to track
//! when an asset was last modified.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Represents a timestamp for an asset.
///
/// Timestamps are represented by Unix time - the number of seconds
/// elapsed since 00:00:00 UTC 1/1/1970. An invalid timestamp is
/// represented by NaN.
///
/// # Examples
///
/// ```
/// use usd_ar::Timestamp;
///
/// // Create a valid timestamp
/// let ts = Timestamp::new(1609459200.0); // 2021-01-01 00:00:00 UTC
/// assert!(ts.is_valid());
///
/// // Create an invalid timestamp
/// let invalid = Timestamp::invalid();
/// assert!(!invalid.is_valid());
/// ```
#[derive(Clone, Copy)]
pub struct Timestamp {
    /// Unix timestamp in seconds (NaN if invalid).
    time: f64,
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::invalid()
    }
}

impl Timestamp {
    /// Creates a new timestamp with the given Unix time value.
    ///
    /// # Arguments
    ///
    /// * `time` - Unix timestamp in seconds since epoch
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::Timestamp;
    ///
    /// let ts = Timestamp::new(1609459200.0);
    /// assert!(ts.is_valid());
    /// assert_eq!(ts.get_time(), 1609459200.0);
    /// ```
    pub fn new(time: f64) -> Self {
        Self { time }
    }

    /// Creates an invalid timestamp.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::Timestamp;
    ///
    /// let ts = Timestamp::invalid();
    /// assert!(!ts.is_valid());
    /// ```
    pub fn invalid() -> Self {
        Self { time: f64::NAN }
    }

    /// Creates a timestamp from the current system time.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::Timestamp;
    ///
    /// let ts = Timestamp::now();
    /// assert!(ts.is_valid());
    /// assert!(ts.get_time() > 0.0);
    /// ```
    pub fn now() -> Self {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => Self::new(duration.as_secs_f64()),
            Err(_) => Self::invalid(),
        }
    }

    /// Creates a timestamp from a `SystemTime`.
    ///
    /// Returns an invalid timestamp if the time is before the Unix epoch.
    ///
    /// # Arguments
    ///
    /// * `time` - The system time to convert
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::Timestamp;
    /// use std::time::SystemTime;
    ///
    /// let ts = Timestamp::from_system_time(SystemTime::now());
    /// assert!(ts.is_valid());
    /// ```
    pub fn from_system_time(time: SystemTime) -> Self {
        match time.duration_since(UNIX_EPOCH) {
            Ok(duration) => Self::new(duration.as_secs_f64()),
            Err(_) => Self::invalid(),
        }
    }

    /// Returns `true` if this timestamp is valid, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::Timestamp;
    ///
    /// let valid = Timestamp::new(1609459200.0);
    /// assert!(valid.is_valid());
    ///
    /// let invalid = Timestamp::invalid();
    /// assert!(!invalid.is_valid());
    /// ```
    pub fn is_valid(&self) -> bool {
        !self.time.is_nan()
    }

    /// Returns the time represented by this timestamp as a `f64`.
    ///
    /// # Panics
    ///
    /// Panics if the timestamp is invalid. Use `try_get_time()` for
    /// a non-panicking version.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::Timestamp;
    ///
    /// let ts = Timestamp::new(1609459200.0);
    /// assert_eq!(ts.get_time(), 1609459200.0);
    /// ```
    #[deprecated(note = "use try_get_time() instead")]
    pub fn get_time(&self) -> f64 {
        if !self.is_valid() {
            panic!("Cannot get time from invalid timestamp");
        }
        self.time
    }

    /// Returns the time represented by this timestamp, or `None` if invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::Timestamp;
    ///
    /// let valid = Timestamp::new(1609459200.0);
    /// assert_eq!(valid.try_get_time(), Some(1609459200.0));
    ///
    /// let invalid = Timestamp::invalid();
    /// assert_eq!(invalid.try_get_time(), None);
    /// ```
    pub fn try_get_time(&self) -> Option<f64> {
        if self.is_valid() {
            Some(self.time)
        } else {
            None
        }
    }

    /// Converts this timestamp to a `SystemTime`.
    ///
    /// Returns `None` if the timestamp is invalid or cannot be represented
    /// as a `SystemTime`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::Timestamp;
    ///
    /// let ts = Timestamp::new(1609459200.0);
    /// let system_time = ts.to_system_time();
    /// assert!(system_time.is_some());
    /// ```
    pub fn to_system_time(&self) -> Option<SystemTime> {
        if !self.is_valid() || self.time < 0.0 {
            return None;
        }
        Some(UNIX_EPOCH + Duration::from_secs_f64(self.time))
    }

    /// Returns the raw time value (may be NaN for invalid timestamps).
    ///
    /// Use this method when you need to access the raw value without
    /// validation checking.
    pub fn raw_time(&self) -> f64 {
        self.time
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            f.debug_struct("Timestamp")
                .field("time", &self.time)
                .finish()
        } else {
            f.debug_struct("Timestamp")
                .field("time", &"invalid")
                .finish()
        }
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "{}", self.time)
        } else {
            write!(f, "<invalid>")
        }
    }
}

impl Hash for Timestamp {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the bits of the f64 value
        self.time.to_bits().hash(state);
    }
}

impl PartialEq for Timestamp {
    fn eq(&self, other: &Self) -> bool {
        // Two invalid timestamps are considered equal
        // Two valid timestamps are equal if their times match
        (!self.is_valid() && !other.is_valid())
            || (self.is_valid() && other.is_valid() && self.time == other.time)
    }
}

impl Eq for Timestamp {}

impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        // Invalid timestamps are considered less than valid timestamps
        match (self.is_valid(), other.is_valid()) {
            (false, false) => Ordering::Equal,
            (false, true) => Ordering::Less,
            (true, false) => Ordering::Greater,
            (true, true) => self
                .time
                .partial_cmp(&other.time)
                .unwrap_or(Ordering::Equal),
        }
    }
}

impl From<f64> for Timestamp {
    fn from(time: f64) -> Self {
        Self::new(time)
    }
}

impl From<SystemTime> for Timestamp {
    fn from(time: SystemTime) -> Self {
        Self::from_system_time(time)
    }
}

impl TryFrom<Timestamp> for f64 {
    type Error = &'static str;

    fn try_from(ts: Timestamp) -> Result<Self, Self::Error> {
        ts.try_get_time().ok_or("Invalid timestamp")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let ts = Timestamp::new(1609459200.0);
        assert!(ts.is_valid());
        assert_eq!(ts.try_get_time(), Some(1609459200.0));
    }

    #[test]
    fn test_invalid() {
        let ts = Timestamp::invalid();
        assert!(!ts.is_valid());
    }

    #[test]
    fn test_default_is_invalid() {
        let ts = Timestamp::default();
        assert!(!ts.is_valid());
    }

    #[test]
    fn test_now() {
        let ts = Timestamp::now();
        assert!(ts.is_valid());
        // Should be after 2020
        assert!(ts.try_get_time().expect("valid") > 1577836800.0);
    }

    #[test]
    fn test_try_get_time() {
        let valid = Timestamp::new(1609459200.0);
        assert_eq!(valid.try_get_time(), Some(1609459200.0));

        let invalid = Timestamp::invalid();
        assert_eq!(invalid.try_get_time(), None);
    }

    #[test]
    fn test_equality() {
        let ts1 = Timestamp::new(1609459200.0);
        let ts2 = Timestamp::new(1609459200.0);
        let ts3 = Timestamp::new(1609459201.0);

        assert_eq!(ts1, ts2);
        assert_ne!(ts1, ts3);
    }

    #[test]
    fn test_invalid_equality() {
        let invalid1 = Timestamp::invalid();
        let invalid2 = Timestamp::invalid();
        let valid = Timestamp::new(1609459200.0);

        // Two invalid timestamps are equal
        assert_eq!(invalid1, invalid2);
        // Invalid and valid are not equal
        assert_ne!(invalid1, valid);
    }

    #[test]
    fn test_ordering() {
        let ts1 = Timestamp::new(1609459200.0);
        let ts2 = Timestamp::new(1609459201.0);
        let invalid = Timestamp::invalid();

        assert!(ts1 < ts2);
        assert!(ts2 > ts1);
        // Invalid is less than valid
        assert!(invalid < ts1);
        assert!(ts1 > invalid);
    }

    #[test]
    fn test_from_f64() {
        let ts: Timestamp = 1609459200.0.into();
        assert!(ts.is_valid());
        assert_eq!(ts.try_get_time(), Some(1609459200.0));
    }

    #[test]
    fn test_from_system_time() {
        let now = SystemTime::now();
        let ts: Timestamp = now.into();
        assert!(ts.is_valid());
    }

    #[test]
    fn test_to_system_time() {
        let ts = Timestamp::new(1609459200.0);
        let system_time = ts.to_system_time();
        assert!(system_time.is_some());

        // Round-trip
        let ts2 = Timestamp::from_system_time(system_time.expect("should be valid"));
        assert_eq!(ts.try_get_time(), ts2.try_get_time());
    }

    #[test]
    fn test_to_system_time_invalid() {
        let ts = Timestamp::invalid();
        assert!(ts.to_system_time().is_none());
    }

    #[test]
    fn test_display() {
        let valid = Timestamp::new(1609459200.0);
        assert_eq!(format!("{}", valid), "1609459200");

        let invalid = Timestamp::invalid();
        assert_eq!(format!("{}", invalid), "<invalid>");
    }

    #[test]
    fn test_debug() {
        let valid = Timestamp::new(1609459200.0);
        let debug = format!("{:?}", valid);
        assert!(debug.contains("Timestamp"));
        assert!(debug.contains("1609459200"));

        let invalid = Timestamp::invalid();
        let debug = format!("{:?}", invalid);
        assert!(debug.contains("invalid"));
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let ts1 = Timestamp::new(1609459200.0);
        let ts2 = Timestamp::new(1609459200.0);

        let mut set = HashSet::new();
        set.insert(ts1.raw_time().to_bits());
        assert!(set.contains(&ts2.raw_time().to_bits()));
    }

    #[test]
    fn test_clone() {
        let ts1 = Timestamp::new(1609459200.0);
        let ts2 = ts1;
        assert_eq!(ts1, ts2);
    }

    #[test]
    fn test_try_from() {
        let ts = Timestamp::new(1609459200.0);
        let result: Result<f64, _> = ts.try_into();
        assert_eq!(result, Ok(1609459200.0));

        let invalid = Timestamp::invalid();
        let result: Result<f64, _> = invalid.try_into();
        assert!(result.is_err());
    }
}
