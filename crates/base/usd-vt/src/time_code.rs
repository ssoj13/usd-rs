//! Time code type, moved here from usd-sdf to break circular dependency.
//!
//! `TimeCode` represents a time value for time-based value resolution in USD.
//! It is a thin wrapper around `f64`. This corresponds to C++ `SdfTimeCode`.
//!
//! The type lives here (in `usd-vt`) so that `Value` can natively store and
//! dispatch on `TimeCode` without creating a circular dependency with `usd-sdf`.
//! `usd-sdf` re-exports this type for backward compatibility.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, Div, Mul, Sub};

/// A time value used for time-based value resolution.
///
/// `TimeCode` wraps `f64` and signals to the USD value resolution machinery
/// that the value should be resolved at a specific time. NaN represents the
/// special "default" time code (non-time-sampled), matching C++ `SdfTimeCode()`.
#[derive(Clone, Copy, Debug)]
pub struct TimeCode {
    time: f64,
}

impl Default for TimeCode {
    /// Returns the "default" time code (NaN sentinel), matching C++ `SdfTimeCode()`.
    ///
    /// This is intentionally NaN — callers that need t=0.0 should use `TimeCode::new(0.0)`.
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TimeCode {
    /// The default time code — represents non-time-sampled ("default") values.
    pub const DEFAULT: TimeCode = TimeCode { time: f64::NAN };

    /// Creates a new time code with the given time value.
    #[inline]
    pub const fn new(time: f64) -> Self {
        Self { time }
    }

    /// Returns a time code representing the default (non-time-sampled) value.
    #[inline]
    pub const fn default_time() -> Self {
        Self::DEFAULT
    }

    /// Returns true if this is the default time code (NaN).
    #[inline]
    pub fn is_default(&self) -> bool {
        self.time.is_nan()
    }

    /// Returns the underlying f64 time value.
    #[inline]
    pub const fn value(&self) -> f64 {
        self.time
    }

    /// Returns the hash of this time code.
    pub fn get_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

impl From<f64> for TimeCode {
    fn from(time: f64) -> Self {
        Self::new(time)
    }
}

impl From<TimeCode> for f64 {
    fn from(tc: TimeCode) -> Self {
        tc.time
    }
}

impl From<i32> for TimeCode {
    fn from(time: i32) -> Self {
        Self::new(time as f64)
    }
}

impl PartialEq for TimeCode {
    fn eq(&self, other: &Self) -> bool {
        // Two default (NaN) timecodes compare equal; otherwise use float equality.
        // This matches C++ GfIsClose / operator== semantics where Default == Default.
        match (self.time.is_nan(), other.time.is_nan()) {
            (true, true) => true,
            (true, false) | (false, true) => false,
            (false, false) => self.time == other.time,
        }
    }
}

impl Eq for TimeCode {}

impl PartialOrd for TimeCode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimeCode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time
            .partial_cmp(&other.time)
            .unwrap_or(Ordering::Equal)
    }
}

impl Hash for TimeCode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the bit pattern so equal NaNs hash the same.
        self.time.to_bits().hash(state);
    }
}

// --- Arithmetic with TimeCode × TimeCode ---

impl Add for TimeCode {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.time + rhs.time)
    }
}

impl Sub for TimeCode {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.time - rhs.time)
    }
}

impl Mul for TimeCode {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.time * rhs.time)
    }
}

impl Div for TimeCode {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        Self::new(self.time / rhs.time)
    }
}

// --- Arithmetic f64 op TimeCode ---

impl Add<TimeCode> for f64 {
    type Output = TimeCode;
    #[inline]
    fn add(self, rhs: TimeCode) -> Self::Output {
        TimeCode::new(self + rhs.time)
    }
}

impl Sub<TimeCode> for f64 {
    type Output = TimeCode;
    #[inline]
    fn sub(self, rhs: TimeCode) -> Self::Output {
        TimeCode::new(self - rhs.time)
    }
}

impl Mul<TimeCode> for f64 {
    type Output = TimeCode;
    #[inline]
    fn mul(self, rhs: TimeCode) -> Self::Output {
        TimeCode::new(self * rhs.time)
    }
}

impl Div<TimeCode> for f64 {
    type Output = TimeCode;
    #[inline]
    fn div(self, rhs: TimeCode) -> Self::Output {
        TimeCode::new(self / rhs.time)
    }
}

// --- Arithmetic TimeCode op f64 ---

impl Add<f64> for TimeCode {
    type Output = TimeCode;
    #[inline]
    fn add(self, rhs: f64) -> Self::Output {
        TimeCode::new(self.time + rhs)
    }
}

impl Sub<f64> for TimeCode {
    type Output = TimeCode;
    #[inline]
    fn sub(self, rhs: f64) -> Self::Output {
        TimeCode::new(self.time - rhs)
    }
}

impl Mul<f64> for TimeCode {
    type Output = TimeCode;
    #[inline]
    fn mul(self, rhs: f64) -> Self::Output {
        TimeCode::new(self.time * rhs)
    }
}

impl Div<f64> for TimeCode {
    type Output = TimeCode;
    #[inline]
    fn div(self, rhs: f64) -> Self::Output {
        TimeCode::new(self.time / rhs)
    }
}

// --- Mixed comparisons with f64 ---

impl PartialEq<f64> for TimeCode {
    fn eq(&self, other: &f64) -> bool {
        self.time == *other
    }
}

impl PartialEq<TimeCode> for f64 {
    fn eq(&self, other: &TimeCode) -> bool {
        *self == other.time
    }
}

impl PartialOrd<f64> for TimeCode {
    fn partial_cmp(&self, other: &f64) -> Option<Ordering> {
        self.time.partial_cmp(other)
    }
}

impl PartialOrd<TimeCode> for f64 {
    fn partial_cmp(&self, other: &TimeCode) -> Option<Ordering> {
        self.partial_cmp(&other.time)
    }
}

impl fmt::Display for TimeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let tc = TimeCode::new(1.5);
        assert_eq!(tc.value(), 1.5);
    }

    #[test]
    fn test_default_time() {
        let tc = TimeCode::default_time();
        assert!(tc.is_default());
        assert!(TimeCode::DEFAULT.is_default());
    }

    #[test]
    fn test_regular_not_default() {
        assert!(!TimeCode::new(0.0).is_default());
        assert!(!TimeCode::new(1.0).is_default());
        assert!(!TimeCode::new(-1.0).is_default());
    }

    #[test]
    fn test_from_f64() {
        let tc: TimeCode = 2.5_f64.into();
        assert_eq!(tc.value(), 2.5);
    }

    #[test]
    fn test_into_f64() {
        let tc = TimeCode::new(3.0);
        let value: f64 = tc.into();
        assert_eq!(value, 3.0);
    }

    #[test]
    fn test_arithmetic() {
        let t1 = TimeCode::new(3.0);
        let t2 = TimeCode::new(2.0);
        assert_eq!((t1 + t2).value(), 5.0);
        assert_eq!((t1 - t2).value(), 1.0);
        assert_eq!((t1 * t2).value(), 6.0);
        assert_eq!((t1 / t2).value(), 1.5);
    }

    #[test]
    fn test_arithmetic_with_f64() {
        let tc = TimeCode::new(10.0);
        assert_eq!((tc + 2.0).value(), 12.0);
        assert_eq!((tc - 2.0).value(), 8.0);
        assert_eq!((tc * 2.0).value(), 20.0);
        assert_eq!((tc / 2.0).value(), 5.0);
        assert_eq!((2.0 + tc).value(), 12.0);
    }

    #[test]
    fn test_hash_consistency() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(TimeCode::new(1.0));
        assert!(set.contains(&TimeCode::new(1.0)));
        assert!(!set.contains(&TimeCode::new(2.0)));
    }
}
