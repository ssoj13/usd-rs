//! Time code range representation and iteration.
//!
//! Provides [`TimeCodeRange`] for representing ranges of USD time codes,
//! supporting iteration over the range with a configurable stride.

use std::fmt;
use std::str::FromStr;
use usd_core::time_code::TimeCode;

/// Represents a range of [`TimeCode`] values with start, end, and stride.
///
/// A `TimeCodeRange` can be iterated to retrieve all time code values in the range.
/// The range may be empty, contain a single time code, or represent multiple
/// time codes from start to end. The interval is closed on both ends.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeCodeRange {
    /// Start time code of the range.
    start_time_code: TimeCode,
    /// End time code of the range.
    end_time_code: TimeCode,
    /// Stride value for iteration.
    stride: f64,
}

impl TimeCodeRange {
    /// Creates an invalid empty range.
    pub fn empty() -> Self {
        Self {
            start_time_code: TimeCode::new(0.0),
            end_time_code: TimeCode::new(-1.0),
            stride: 1.0,
        }
    }

    /// Creates a range containing only the given time code.
    pub fn new_single(time_code: TimeCode) -> Self {
        Self::new(time_code, time_code)
    }

    /// Creates a range from start to end time codes.
    ///
    /// If end is greater than or equal to start, stride will be 1.0.
    /// Otherwise, stride will be -1.0.
    pub fn new(start_time_code: TimeCode, end_time_code: TimeCode) -> Self {
        let stride = if end_time_code >= start_time_code {
            1.0
        } else {
            -1.0
        };
        Self::new_with_stride(start_time_code, end_time_code, stride)
    }

    /// Creates a range with explicit stride.
    ///
    /// Returns an invalid empty range if:
    /// - Start or end is `EarliestTime` or `Default`
    /// - Stride is positive but end < start
    /// - Stride is negative but end > start
    /// - Stride is zero
    pub fn new_with_stride(
        start_time_code: TimeCode,
        end_time_code: TimeCode,
        stride: f64,
    ) -> Self {
        // Validate time codes
        if start_time_code.is_earliest_time() || start_time_code.is_default() {
            return Self::empty();
        }
        if end_time_code.is_earliest_time() || end_time_code.is_default() {
            return Self::empty();
        }

        // Validate stride and direction
        if stride > 0.0 {
            if end_time_code < start_time_code {
                return Self::empty();
            }
        } else if stride < 0.0 {
            if end_time_code > start_time_code {
                return Self::empty();
            }
        } else {
            return Self::empty();
        }

        Self {
            start_time_code,
            end_time_code,
            stride,
        }
    }

    /// Creates a time code range from a frame spec string.
    pub fn from_frame_spec(frame_spec: &str) -> Self {
        let trimmed = frame_spec.trim();

        if trimmed.is_empty() || trimmed == "NONE" {
            return Self::empty();
        }

        let (time_part, stride) = if let Some(pos) = trimmed.find('x') {
            let stride_str = &trimmed[pos + 1..];
            let stride = stride_str.parse::<f64>().unwrap_or(0.0);
            (&trimmed[..pos], stride)
        } else {
            (trimmed, 0.0)
        };

        if let Some(pos) = time_part.find(':') {
            let start_str = &time_part[..pos];
            let end_str = &time_part[pos + 1..];

            let start = start_str.parse::<f64>().unwrap_or(0.0);
            let end = end_str.parse::<f64>().unwrap_or(0.0);

            if stride != 0.0 {
                Self::new_with_stride(TimeCode::new(start), TimeCode::new(end), stride)
            } else {
                Self::new(TimeCode::new(start), TimeCode::new(end))
            }
        } else {
            let value = time_part.parse::<f64>().unwrap_or(0.0);
            Self::new_single(TimeCode::new(value))
        }
    }

    /// Returns the start time code of this range.
    pub fn get_start_time_code(&self) -> &TimeCode {
        &self.start_time_code
    }

    /// Returns the end time code of this range.
    pub fn get_end_time_code(&self) -> &TimeCode {
        &self.end_time_code
    }

    /// Returns the stride value of this range.
    pub fn get_stride(&self) -> f64 {
        self.stride
    }

    /// Returns true if this range contains no time codes.
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    /// Returns true if this range contains one or more time codes.
    pub fn is_valid(&self) -> bool {
        !self.is_empty()
    }

    /// Returns an iterator over the time codes in this range.
    pub fn iter(&self) -> TimeCodeRangeIterator {
        TimeCodeRangeIterator::new(self)
    }
}

impl Default for TimeCodeRange {
    fn default() -> Self {
        Self::empty()
    }
}

impl FromStr for TimeCodeRange {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from_frame_spec(s))
    }
}

impl fmt::Display for TimeCodeRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, "NONE")
        } else {
            let start = self.start_time_code.value();
            let end = self.end_time_code.value();

            if (start - end).abs() < f64::EPSILON {
                write!(f, "{}", start)
            } else if (self.stride.abs() - 1.0).abs() < f64::EPSILON {
                write!(f, "{}:{}", start, end)
            } else {
                write!(f, "{}:{}x{}", start, end, self.stride)
            }
        }
    }
}

impl IntoIterator for TimeCodeRange {
    type Item = TimeCode;
    type IntoIter = TimeCodeRangeIterator;

    fn into_iter(self) -> Self::IntoIter {
        TimeCodeRangeIterator::new_owned(self)
    }
}

impl IntoIterator for &TimeCodeRange {
    type Item = TimeCode;
    type IntoIter = TimeCodeRangeIterator;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over time codes in a [`TimeCodeRange`].
#[derive(Debug)]
pub struct TimeCodeRangeIterator {
    /// Owned range data for iteration.
    start_value: f64,
    stride: f64,
    /// Current step in iteration.
    curr_step: usize,
    /// Maximum number of steps.
    max_steps: usize,
    /// Whether the iterator is exhausted.
    exhausted: bool,
}

impl TimeCodeRangeIterator {
    /// Creates a new iterator from a time code range reference.
    pub fn new(range: &TimeCodeRange) -> Self {
        Self::new_impl(
            range.start_time_code.value(),
            range.end_time_code.value(),
            range.stride,
        )
    }

    /// Creates a new iterator from an owned time code range.
    pub fn new_owned(range: TimeCodeRange) -> Self {
        Self::new_impl(
            range.start_time_code.value(),
            range.end_time_code.value(),
            range.stride,
        )
    }

    fn new_impl(start_value: f64, end_value: f64, stride: f64) -> Self {
        let max_steps = if stride.abs() < f64::EPSILON {
            0
        } else {
            ((end_value - start_value + stride) / stride)
                .floor()
                .max(0.0) as usize
        };

        let exhausted = max_steps == 0;

        Self {
            start_value,
            stride,
            curr_step: 0,
            max_steps,
            exhausted,
        }
    }
}

impl Iterator for TimeCodeRangeIterator {
    type Item = TimeCode;

    fn next(&mut self) -> Option<Self::Item> {
        if self.exhausted {
            return None;
        }

        if self.curr_step >= self.max_steps {
            self.exhausted = true;
            return None;
        }

        let value = self.start_value + self.stride * self.curr_step as f64;
        self.curr_step += 1;

        Some(TimeCode::new(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_range() {
        let range = TimeCodeRange::empty();
        assert!(range.is_empty());
        assert!(!range.is_valid());
    }

    #[test]
    fn test_single_time_code() {
        let range = TimeCodeRange::new_single(TimeCode::new(101.0));
        assert!(range.is_valid());

        let values: Vec<f64> = range.iter().map(|tc| tc.value()).collect();
        assert_eq!(values, vec![101.0]);
    }

    #[test]
    fn test_range() {
        let range = TimeCodeRange::new(TimeCode::new(101.0), TimeCode::new(105.0));
        assert!(range.is_valid());

        let values: Vec<f64> = range.iter().map(|tc| tc.value()).collect();
        assert_eq!(values, vec![101.0, 102.0, 103.0, 104.0, 105.0]);
    }

    #[test]
    fn test_range_with_stride() {
        let range = TimeCodeRange::new_with_stride(TimeCode::new(101.0), TimeCode::new(109.0), 2.0);
        assert!(range.is_valid());

        let values: Vec<f64> = range.iter().map(|tc| tc.value()).collect();
        assert_eq!(values, vec![101.0, 103.0, 105.0, 107.0, 109.0]);
    }

    #[test]
    fn test_from_frame_spec() {
        let range = TimeCodeRange::from_frame_spec("101:109x2");
        let values: Vec<f64> = range.iter().map(|tc| tc.value()).collect();
        assert_eq!(values, vec![101.0, 103.0, 105.0, 107.0, 109.0]);
    }

    #[test]
    fn test_display() {
        let range = TimeCodeRange::from_frame_spec("101:109x2");
        assert_eq!(range.to_string(), "101:109x2");

        let single = TimeCodeRange::new_single(TimeCode::new(101.0));
        assert_eq!(single.to_string(), "101");

        let empty = TimeCodeRange::empty();
        assert_eq!(empty.to_string(), "NONE");
    }
}
