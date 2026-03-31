//! USD Time Code - represents a time value for time-sampled data.
//!
//! Port of pxr/usd/usd/timeCode.h
//!
//! UsdTimeCode represents a time value, which may be either numeric, holding a double
//! value, or a sentinel value UsdTimeCode::Default().
//!
//! A UsdTimeCode does not represent an SMPTE timecode, although we may, in future,
//! support conversion functions between the two. Instead, UsdTimeCode is an abstraction
//! that acknowledges that in the principal domains of use for USD, there are many
//! different ways of encoding time, and USD must be able to capture and translate
//! between all of them for interchange, retaining as much intent of the authoring
//! application as possible.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use usd_sdf::TimeCode as SdfTimeCode;

/// Tokens for UsdTimeCode sentinel values.
///
/// Matches C++ `UsdTimeCodeTokens`.
pub mod tokens {
    use usd_tf::Token;

    /// Default time token.
    pub const DEFAULT: &str = "DEFAULT";
    /// Earliest time token.
    pub const EARLIEST: &str = "EARLIEST";
    /// Pre-time token.
    pub const PRE_TIME: &str = "PRE_TIME";

    /// Get the default time token as a Token.
    pub fn default_token() -> Token {
        Token::from(DEFAULT)
    }

    /// Get the earliest time token as a Token.
    pub fn earliest_token() -> Token {
        Token::from(EARLIEST)
    }

    /// Get the pre-time token as a Token.
    pub fn pre_time_token() -> Token {
        Token::from(PRE_TIME)
    }
}

/// Represents a time value, which may be either numeric, holding a double
/// value, or a sentinel value UsdTimeCode::Default().
///
/// Matches C++ `UsdTimeCode`.
#[derive(Clone, Copy, Debug)]
pub struct TimeCode {
    /// The numeric time value, or NaN for Default().
    value: f64,
    /// Whether this represents a pre-time value.
    is_pre_time: bool,
}

impl TimeCode {
    /// Construct with optional time value. Implicitly convert from double.
    ///
    /// Matches C++ `UsdTimeCode(double t = 0.0)`.
    pub const fn new(t: f64) -> Self {
        Self {
            value: t,
            is_pre_time: false,
        }
    }

    /// Construct and implicitly cast from SdfTimeCode.
    ///
    /// Matches C++ `UsdTimeCode(const SdfTimeCode &sdfTimeCode)`.
    pub const fn from_sdf_time_code(sdf_time_code: &SdfTimeCode) -> Self {
        Self {
            value: sdf_time_code.value(),
            is_pre_time: false,
        }
    }

    /// Produces a UsdTimeCode representing a pre-time at `t`.
    ///
    /// Matches C++ `UsdTimeCode::PreTime(double t)`.
    pub const fn pre_time(t: f64) -> Self {
        Self {
            value: t,
            is_pre_time: true,
        }
    }

    /// Produces a UsdTimeCode representing a pre-time using SdfTimeCode.
    ///
    /// Matches C++ `UsdTimeCode::PreTime(const SdfTimeCode& timeCode)`.
    pub const fn pre_time_from_sdf(sdf_time_code: &SdfTimeCode) -> Self {
        Self {
            value: sdf_time_code.value(),
            is_pre_time: true,
        }
    }

    /// Produce a UsdTimeCode representing the lowest/earliest possible timeCode.
    ///
    /// Matches C++ `UsdTimeCode::EarliestTime()`.
    pub const fn earliest_time() -> Self {
        Self {
            value: f64::MIN,
            is_pre_time: false,
        }
    }

    /// Produce a UsdTimeCode representing the sentinel value for 'default'.
    ///
    /// Matches C++ `UsdTimeCode::Default()`.
    pub const fn default() -> Self {
        Self {
            value: f64::NAN,
            is_pre_time: false,
        }
    }

    /// Produce a UsdTimeCode representing the sentinel value for 'default'.
    ///
    /// Alias for `default()` matching sdf::TimeCode naming.
    #[inline]
    pub const fn default_time() -> Self {
        Self::default()
    }

    /// Produce a safe step value such that for any numeric UsdTimeCode t in
    /// [-maxValue, maxValue], t +/- (step / maxCompression) != t with a safety
    /// factor of 2.
    ///
    /// Matches C++ `UsdTimeCode::SafeStep(double maxValue, double maxCompression)`.
    pub fn safe_step(max_value: f64, max_compression: f64) -> f64 {
        f64::EPSILON * max_value * max_compression * 2.0
    }

    /// Return true if this timeCode represents a pre-value, false otherwise.
    ///
    /// Matches C++ `IsPreTime()`.
    pub const fn is_pre_time(&self) -> bool {
        self.is_pre_time
    }

    /// Return true if this time represents the lowest/earliest possible timeCode.
    ///
    /// Matches C++ `IsEarliestTime()`.
    pub fn is_earliest_time(&self) -> bool {
        self.is_numeric() && self.value == f64::MIN
    }

    /// Return true if this time represents the 'default' sentinel value.
    ///
    /// Matches C++ `IsDefault()`.
    pub fn is_default(&self) -> bool {
        self.value.is_nan()
    }

    /// Return true if this time represents a numeric value.
    ///
    /// Matches C++ `IsNumeric()`.
    pub fn is_numeric(&self) -> bool {
        !self.is_default()
    }

    /// Return the numeric value for this time. If this time IsDefault(),
    /// return a quiet NaN value.
    ///
    /// Matches C++ `GetValue()`.
    pub fn value(&self) -> f64 {
        if self.is_default() {
            self._issue_get_value_on_default_error();
        }
        self.value
    }

    /// Issue an error when GetValue() is called on a Default UsdTimeCode.
    ///
    /// Matches C++ `_IssueGetValueOnDefaultError()`.
    fn _issue_get_value_on_default_error(&self) {
        // In C++, this calls TF_CODING_ERROR. In Rust, we could log a warning
        // or use a diagnostics system. For now, we just return NaN.
    }
}

impl Default for TimeCode {
    /// Returns the "default" (NaN sentinel) time code, matching C++ `UsdTimeCode::Default()`.
    ///
    /// This follows C++ semantics — `UsdTimeCode()` is equivalent to `UsdTimeCode::Default()`.
    /// Code that explicitly needs t=0.0 should use `TimeCode::new(0.0)`.
    fn default() -> Self {
        Self::default()
    }
}

impl From<f64> for TimeCode {
    fn from(t: f64) -> Self {
        Self::new(t)
    }
}

impl From<SdfTimeCode> for TimeCode {
    fn from(sdf_time_code: SdfTimeCode) -> Self {
        Self::from_sdf_time_code(&sdf_time_code)
    }
}

impl PartialEq for TimeCode {
    fn eq(&self, other: &Self) -> bool {
        if self.is_default() && other.is_default() {
            return true;
        }
        self.value == other.value && self.is_pre_time == other.is_pre_time
    }
}

impl Eq for TimeCode {}

impl PartialOrd for TimeCode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimeCode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Default() times are less than all numeric times
        if self.is_default() || other.is_default() {
            return if self.is_default() && !other.is_default() {
                std::cmp::Ordering::Less
            } else if !self.is_default() && other.is_default() {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            };
        }

        // Numeric times are ordered by their value
        match self.value.partial_cmp(&other.value) {
            Some(std::cmp::Ordering::Equal) => {
                // If numeric times are equal, pre-time times are less than non pre-time times
                match (self.is_pre_time, other.is_pre_time) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                }
            }
            Some(ord) => ord,
            None => {
                // NaN comparison (shouldn't happen if we checked is_default above)
                std::cmp::Ordering::Equal
            }
        }
    }
}

impl Hash for TimeCode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the value and is_pre_time flag
        // For NaN, use a special value
        if self.value.is_nan() {
            state.write_u64(0x7FF8_0000_0000_0000u64); // NaN bit pattern
        } else {
            self.value.to_bits().hash(state);
        }
        self.is_pre_time.hash(state);
    }
}

impl fmt::Display for TimeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_default() {
            write!(f, "{}", tokens::DEFAULT)
        } else {
            if self.is_pre_time {
                write!(f, "{} ", tokens::PRE_TIME)?;
            }
            if self.is_earliest_time() {
                write!(f, "{}", tokens::EARLIEST)
            } else {
                write!(f, "{}", self.value)
            }
        }
    }
}

impl FromStr for TimeCode {
    type Err = String;

    /// Parse a TimeCode from a string.
    ///
    /// Matches C++ `operator>>(std::istream& is, UsdTimeCode& time)`.
    ///
    /// Supports parsing:
    /// - "DEFAULT" for default time
    /// - "EARLIEST" for earliest time
    /// - "PRE_TIME <value>" for pre-time values
    /// - Numeric values as strings
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split_whitespace();
        let first = parts.next().ok_or_else(|| "Empty string".to_string())?;

        let is_pre_time = first == tokens::PRE_TIME;
        let value_str = if is_pre_time {
            parts
                .next()
                .ok_or_else(|| "PRE_TIME requires a value".to_string())?
        } else {
            first
        };

        if value_str == tokens::DEFAULT {
            if is_pre_time {
                return Err("PRE_TIME cannot be used with DEFAULT".to_string());
            }
            return Ok(TimeCode::default());
        }

        if value_str == tokens::EARLIEST {
            return Ok(if is_pre_time {
                TimeCode::pre_time(TimeCode::earliest_time().value())
            } else {
                TimeCode::earliest_time()
            });
        }

        // Try to parse as a number
        match value_str.parse::<f64>() {
            Ok(value) => {
                if value_str.chars().all(|c| {
                    c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E'
                }) {
                    Ok(if is_pre_time {
                        TimeCode::pre_time(value)
                    } else {
                        TimeCode::new(value)
                    })
                } else {
                    Err(format!("Invalid time value: {}", value_str))
                }
            }
            Err(_) => Err(format!("Invalid time value: {}", value_str)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let tc = TimeCode::default();
        assert!(tc.is_default());
        assert!(!tc.is_numeric());
        assert!(!tc.is_pre_time());
    }

    #[test]
    fn test_numeric() {
        let tc = TimeCode::new(1.5);
        assert!(!tc.is_default());
        assert!(tc.is_numeric());
        assert_eq!(tc.value(), 1.5);
    }

    #[test]
    fn test_earliest_time() {
        let tc = TimeCode::earliest_time();
        assert!(tc.is_earliest_time());
        assert!(tc.is_numeric());
        assert_eq!(tc.value(), f64::MIN);
    }

    #[test]
    fn test_pre_time() {
        let tc = TimeCode::pre_time(2.0);
        assert!(tc.is_pre_time());
        assert_eq!(tc.value(), 2.0);
    }

    #[test]
    fn test_comparison() {
        let default = TimeCode::default();
        let early = TimeCode::earliest_time();
        let t1 = TimeCode::new(1.0);
        let t2 = TimeCode::new(2.0);

        // Default is less than all numeric times
        assert!(default < early);
        assert!(default < t1);
        assert!(default < t2);

        // Numeric times are ordered correctly
        assert!(t1 < t2);
        assert!(early < t1);

        // Pre-time is less than non-pre-time at same value
        let pre_t1 = TimeCode::pre_time(1.0);
        assert!(pre_t1 < t1);
    }

    #[test]
    fn test_equality() {
        let t1 = TimeCode::new(1.0);
        let t2 = TimeCode::new(1.0);
        let t3 = TimeCode::new(2.0);
        let default1 = TimeCode::default();
        let default2 = TimeCode::default();

        assert_eq!(t1, t2);
        assert_ne!(t1, t3);
        assert_eq!(default1, default2);
        assert_ne!(default1, t1);
    }

    #[test]
    fn test_from_f64() {
        let tc: TimeCode = 3.14.into();
        assert_eq!(tc.value(), 3.14);
        assert!(tc.is_numeric());
    }

    #[test]
    fn test_safe_step() {
        let step = TimeCode::safe_step(1e6, 10.0);
        assert!(step > 0.0);
        assert!(step < 1.0);
    }
}
