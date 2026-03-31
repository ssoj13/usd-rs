//! A set of non-intersecting intervals representing a subset of the real number line.
//!
//! [`MultiInterval`] represents a subset of the real number line as an
//! ordered set of non-intersecting [`Interval`]s.
//!
//! # Examples
//!
//! ```
//! use usd_gf::{MultiInterval, Interval};
//!
//! let mut mi = MultiInterval::new();
//! mi.add(Interval::closed(0.0, 5.0));
//! mi.add(Interval::closed(10.0, 15.0));
//!
//! assert!(mi.contains_value(3.0));
//! assert!(mi.contains_value(12.0));
//! assert!(!mi.contains_value(7.0));
//!
//! // Intervals are automatically merged when they overlap
//! mi.add(Interval::closed(4.0, 11.0));
//! assert_eq!(mi.len(), 1); // Now one continuous interval [0, 15]
//! ```

use crate::interval::Interval;
use std::collections::BTreeSet;
use std::fmt;
use std::hash::{Hash, Hasher};

/// A set of non-intersecting intervals.
///
/// Represents a subset of the real number line as an ordered set of
/// non-overlapping intervals. Intervals are automatically merged when
/// they overlap or touch.
#[derive(Clone, Debug, Default)]
pub struct MultiInterval {
    /// The set of intervals, ordered and non-overlapping.
    intervals: BTreeSet<Interval>,
}

impl MultiInterval {
    /// Creates an empty multi-interval.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a multi-interval containing a single interval.
    #[must_use]
    pub fn from_interval(interval: Interval) -> Self {
        let mut mi = Self::new();
        mi.add(interval);
        mi
    }

    /// Creates a multi-interval from a list of intervals.
    ///
    /// Intervals are merged as they are added.
    #[must_use]
    pub fn from_intervals(intervals: &[Interval]) -> Self {
        let mut mi = Self::new();
        for interval in intervals {
            mi.add(*interval);
        }
        mi
    }

    /// Returns the full interval (-inf, inf).
    #[must_use]
    pub fn full() -> Self {
        Self::from_interval(Interval::full())
    }

    /// Returns true if the multi-interval is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.intervals.is_empty()
    }

    /// Returns the number of intervals in the set.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.intervals.len()
    }

    /// Returns an interval bounding the entire multi-interval.
    ///
    /// Returns an empty interval if the multi-interval is empty.
    #[must_use]
    pub fn bounds(&self) -> Interval {
        if self.intervals.is_empty() {
            return Interval::new_empty();
        }

        let first = self
            .intervals
            .iter()
            .next()
            .unwrap_or_else(|| unreachable!());
        let last = self
            .intervals
            .iter()
            .next_back()
            .unwrap_or_else(|| unreachable!());

        Interval::new(
            first.get_min(),
            last.get_max(),
            first.is_min_closed(),
            last.is_max_closed(),
        )
    }

    /// Returns true if the multi-interval contains the given value.
    #[must_use]
    pub fn contains_value(&self, d: f64) -> bool {
        // Find intervals that might contain d
        let point = Interval::from_point(d);

        // Check intervals around d
        for interval in self.intervals.range(..=point) {
            if interval.contains(d) {
                return true;
            }
        }

        // Check the first interval >= point
        if let Some(interval) = self.intervals.range(point..).next() {
            if interval.contains(d) {
                return true;
            }
        }

        false
    }

    /// Returns true if the multi-interval contains the given interval.
    #[must_use]
    pub fn contains_interval(&self, interval: &Interval) -> bool {
        if interval.is_empty() {
            return false;
        }

        // Find intervals that might contain the given interval
        for existing in self.intervals.iter() {
            if existing.contains_interval(interval) {
                return true;
            }
        }

        false
    }

    /// Returns true if this multi-interval contains all intervals in the other.
    #[must_use]
    pub fn contains(&self, other: &MultiInterval) -> bool {
        if other.is_empty() {
            return false;
        }

        for interval in &other.intervals {
            if !self.contains_interval(interval) {
                return false;
            }
        }

        true
    }

    /// Clears the multi-interval.
    pub fn clear(&mut self) {
        self.intervals.clear();
    }

    /// Adds an interval to the multi-interval, merging with existing intervals as needed.
    pub fn add(&mut self, interval: Interval) {
        if interval.is_empty() {
            return;
        }

        let mut merged = interval;

        // Collect intervals to remove (those that will be merged)
        let mut to_remove = Vec::new();

        // Find and merge with subsequent overlapping intervals
        for existing in self.intervals.range(merged..) {
            if merged.intersects(existing) || self.intervals_touch(&merged, existing) {
                merged |= *existing;
                to_remove.push(*existing);
            } else {
                break;
            }
        }

        // Find and merge with prior overlapping intervals
        let prior: Vec<_> = self.intervals.range(..merged).rev().cloned().collect();
        for existing in prior {
            if merged.intersects(&existing) || self.intervals_touch(&existing, &merged) {
                merged |= existing;
                to_remove.push(existing);
            } else {
                break;
            }
        }

        // Remove merged intervals
        for interval in to_remove {
            self.intervals.remove(&interval);
        }

        // Insert final merged result
        self.intervals.insert(merged);
    }

    /// Check if two intervals touch (max of a equals min of b with at least one closed)
    fn intervals_touch(&self, a: &Interval, b: &Interval) -> bool {
        a.get_max() == b.get_min() && !(a.is_max_open() && b.is_min_open())
    }

    /// Adds all intervals from another multi-interval.
    pub fn add_multi(&mut self, other: &MultiInterval) {
        for interval in &other.intervals {
            self.add(*interval);
        }
    }

    /// Removes an interval from this multi-interval.
    pub fn remove(&mut self, interval: Interval) {
        if interval.is_empty() {
            return;
        }

        // Collect intervals to remove and potentially split
        let mut to_remove = Vec::new();
        let mut to_add = Vec::new();

        for existing in self.intervals.iter() {
            if existing.intersects(&interval) {
                to_remove.push(*existing);

                // Create intervals for parts not covered by the removed interval
                let lo = Interval::new(
                    existing.get_min(),
                    interval.get_min(),
                    existing.is_min_closed(),
                    !interval.is_min_closed(),
                );
                let hi = Interval::new(
                    interval.get_max(),
                    existing.get_max(),
                    !interval.is_max_closed(),
                    existing.is_max_closed(),
                );

                if !lo.is_empty() {
                    to_add.push(lo);
                }
                if !hi.is_empty() {
                    to_add.push(hi);
                }
            }
        }

        for interval in to_remove {
            self.intervals.remove(&interval);
        }
        for interval in to_add {
            self.intervals.insert(interval);
        }
    }

    /// Removes all intervals from another multi-interval.
    pub fn remove_multi(&mut self, other: &MultiInterval) {
        for interval in &other.intervals {
            self.remove(*interval);
        }
    }

    /// Intersects this multi-interval with an interval.
    pub fn intersect(&mut self, interval: Interval) {
        self.intersect_multi(&MultiInterval::from_interval(interval));
    }

    /// Intersects this multi-interval with another.
    pub fn intersect_multi(&mut self, other: &MultiInterval) {
        self.remove_multi(&other.complement());
    }

    /// Returns the complement of this set (all real numbers not in this set).
    #[must_use]
    pub fn complement(&self) -> MultiInterval {
        let mut result = MultiInterval::new();
        let mut working = Interval::full();

        for interval in &self.intervals {
            // Insert the portion before this interval
            let prior = Interval::new(
                working.get_min(),
                interval.get_min(),
                working.is_min_closed(),
                !interval.is_min_closed(),
            );
            if !prior.is_empty() {
                result.intervals.insert(prior);
            }

            // Set up next working interval
            working = Interval::new(
                interval.get_max(),
                f64::INFINITY,
                !interval.is_max_closed(),
                false,
            );
        }

        // Insert the final portion after all intervals
        if !working.is_empty() {
            result.intervals.insert(working);
        }

        result
    }

    /// Uses the given interval to extend the multi-interval in the interval arithmetic sense.
    ///
    /// Each existing interval i is replaced by i + interval.
    pub fn arithmetic_add(&mut self, interval: Interval) {
        let mut result = MultiInterval::new();
        for existing in &self.intervals {
            result.add(*existing + interval);
        }
        *self = result;
    }

    /// Swaps this multi-interval with another.
    pub fn swap(&mut self, other: &mut MultiInterval) {
        std::mem::swap(&mut self.intervals, &mut other.intervals);
    }

    /// Returns an iterator over the intervals.
    pub fn iter(&self) -> impl Iterator<Item = &Interval> {
        self.intervals.iter()
    }

    /// Returns the first interval whose minimum value is >= x.
    #[must_use]
    pub fn lower_bound(&self, x: f64) -> Option<&Interval> {
        let point = Interval::from_point(x);
        self.intervals.range(point..).next()
    }

    /// Returns the first interval whose minimum value is > x.
    #[must_use]
    pub fn upper_bound(&self, x: f64) -> Option<&Interval> {
        let point = Interval::new(x, x, false, true);
        self.intervals
            .range((std::ops::Bound::Excluded(point), std::ops::Bound::Unbounded))
            .next()
    }

    /// Returns the first interval whose min is > x and does not contain x.
    ///
    /// C++ parity: `GfMultiInterval::GetNextNonContainingInterval(double)`.
    #[must_use]
    pub fn get_next_non_containing_interval(&self, x: f64) -> Option<&Interval> {
        // C++: upper_bound( GfInterval(x, x, false, true) )
        let probe = Interval::new(x, x, false, true);
        self.intervals
            .range((std::ops::Bound::Excluded(probe), std::ops::Bound::Unbounded))
            .next()
    }

    /// Returns the last interval whose max is < x and does not contain x.
    ///
    /// Returns `None` if no such interval exists.
    /// C++ parity: `GfMultiInterval::GetPriorNonContainingInterval(double)`.
    #[must_use]
    pub fn get_prior_non_containing_interval(&self, x: f64) -> Option<&Interval> {
        let lb_point = Interval::from_point(x);
        let mut cursor = self.intervals.range(..lb_point);

        if let Some(prev) = cursor.next_back() {
            if !prev.contains(x) {
                return Some(prev);
            }
            // prev contains x; try one more step back
            if let Some(prev2) = cursor.next_back() {
                debug_assert!(!prev2.contains(x), "non-overlapping set invariant violated");
                return Some(prev2);
            }
        }
        None
    }

    /// Returns the interval that contains x, or None.
    #[must_use]
    pub fn get_containing_interval(&self, x: f64) -> Option<&Interval> {
        // Check intervals that might contain x
        let point = Interval::from_point(x);

        // Check intervals <= point
        for interval in self.intervals.range(..=point).rev() {
            if interval.contains(x) {
                return Some(interval);
            }
        }

        // Check intervals > point
        if let Some(interval) = self.lower_bound(x) {
            if interval.contains(x) {
                return Some(interval);
            }
        }

        None
    }
}

impl PartialEq for MultiInterval {
    fn eq(&self, other: &Self) -> bool {
        self.intervals == other.intervals
    }
}

impl Eq for MultiInterval {}

impl PartialOrd for MultiInterval {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MultiInterval {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.intervals.cmp(&other.intervals)
    }
}

impl Hash for MultiInterval {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for interval in &self.intervals {
            interval.hash(state);
        }
    }
}

impl fmt::Display for MultiInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        let mut first = true;
        for interval in &self.intervals {
            if !first {
                write!(f, ", ")?;
            }
            write!(f, "{}", interval)?;
            first = false;
        }
        write!(f, "]")
    }
}

impl<'a> IntoIterator for &'a MultiInterval {
    type Item = &'a Interval;
    type IntoIter = std::collections::btree_set::Iter<'a, Interval>;

    fn into_iter(self) -> Self::IntoIter {
        self.intervals.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let mi = MultiInterval::new();
        assert!(mi.is_empty());
        assert_eq!(mi.len(), 0);
    }

    #[test]
    fn test_from_interval() {
        let mi = MultiInterval::from_interval(Interval::closed(0.0, 10.0));
        assert!(!mi.is_empty());
        assert_eq!(mi.len(), 1);
    }

    #[test]
    fn test_add_non_overlapping() {
        let mut mi = MultiInterval::new();
        mi.add(Interval::closed(0.0, 5.0));
        mi.add(Interval::closed(10.0, 15.0));
        assert_eq!(mi.len(), 2);
    }

    #[test]
    fn test_add_overlapping() {
        let mut mi = MultiInterval::new();
        mi.add(Interval::closed(0.0, 5.0));
        mi.add(Interval::closed(3.0, 10.0));
        assert_eq!(mi.len(), 1);

        let bounds = mi.bounds();
        assert_eq!(bounds.get_min(), 0.0);
        assert_eq!(bounds.get_max(), 10.0);
    }

    #[test]
    fn test_add_touching() {
        let mut mi = MultiInterval::new();
        mi.add(Interval::closed(0.0, 5.0));
        mi.add(Interval::closed(5.0, 10.0));
        // Should merge because they touch at closed boundary
        assert_eq!(mi.len(), 1);
    }

    #[test]
    fn test_add_touching_open() {
        let mut mi = MultiInterval::new();
        mi.add(Interval::new(0.0, 5.0, true, false)); // [0, 5)
        mi.add(Interval::new(5.0, 10.0, false, true)); // (5, 10]
        // Should NOT merge because both boundaries at 5 are open
        assert_eq!(mi.len(), 2);
    }

    #[test]
    fn test_contains_value() {
        let mut mi = MultiInterval::new();
        mi.add(Interval::closed(0.0, 5.0));
        mi.add(Interval::closed(10.0, 15.0));

        assert!(mi.contains_value(0.0));
        assert!(mi.contains_value(3.0));
        assert!(mi.contains_value(5.0));
        assert!(!mi.contains_value(7.0));
        assert!(mi.contains_value(10.0));
        assert!(mi.contains_value(12.0));
    }

    #[test]
    fn test_contains_interval() {
        let mut mi = MultiInterval::new();
        mi.add(Interval::closed(0.0, 10.0));

        assert!(mi.contains_interval(&Interval::closed(2.0, 8.0)));
        assert!(!mi.contains_interval(&Interval::closed(5.0, 15.0)));
    }

    #[test]
    fn test_remove() {
        let mut mi = MultiInterval::new();
        mi.add(Interval::closed(0.0, 10.0));
        mi.remove(Interval::closed(3.0, 7.0));

        assert_eq!(mi.len(), 2);
        assert!(mi.contains_value(1.0));
        assert!(!mi.contains_value(5.0));
        assert!(mi.contains_value(9.0));
    }

    #[test]
    fn test_complement() {
        let mut mi = MultiInterval::new();
        mi.add(Interval::closed(0.0, 10.0));

        let comp = mi.complement();

        assert!(!comp.contains_value(5.0));
        assert!(comp.contains_value(-5.0));
        assert!(comp.contains_value(15.0));
    }

    #[test]
    fn test_bounds() {
        let mut mi = MultiInterval::new();
        mi.add(Interval::closed(5.0, 10.0));
        mi.add(Interval::closed(0.0, 3.0));

        let bounds = mi.bounds();
        assert_eq!(bounds.get_min(), 0.0);
        assert_eq!(bounds.get_max(), 10.0);
    }

    #[test]
    fn test_empty_bounds() {
        let mi = MultiInterval::new();
        let bounds = mi.bounds();
        assert!(bounds.is_empty());
    }

    #[test]
    fn test_display() {
        let mut mi = MultiInterval::new();
        mi.add(Interval::closed(0.0, 5.0));
        mi.add(Interval::closed(10.0, 15.0));

        let s = format!("{}", mi);
        assert!(s.starts_with('['));
        assert!(s.ends_with(']'));
    }

    #[test]
    fn test_arithmetic_add() {
        let mut mi = MultiInterval::new();
        mi.add(Interval::closed(0.0, 5.0));
        mi.add(Interval::closed(10.0, 15.0));

        mi.arithmetic_add(Interval::closed(1.0, 2.0));

        // [0,5] + [1,2] = [1,7]
        // [10,15] + [1,2] = [11,17]
        let bounds = mi.bounds();
        assert_eq!(bounds.get_min(), 1.0);
        assert_eq!(bounds.get_max(), 17.0);
    }

    #[test]
    fn test_equality() {
        let mut mi1 = MultiInterval::new();
        mi1.add(Interval::closed(0.0, 5.0));

        let mut mi2 = MultiInterval::new();
        mi2.add(Interval::closed(0.0, 5.0));

        assert_eq!(mi1, mi2);
    }

    #[test]
    fn test_iter() {
        let mut mi = MultiInterval::new();
        mi.add(Interval::closed(0.0, 5.0));
        mi.add(Interval::closed(10.0, 15.0));

        let intervals: Vec<_> = mi.iter().collect();
        assert_eq!(intervals.len(), 2);
    }

    #[test]
    fn test_full() {
        let mi = MultiInterval::full();
        assert!(mi.contains_value(0.0));
        assert!(mi.contains_value(1e100));
        assert!(mi.contains_value(-1e100));
    }

    #[test]
    fn test_intersect() {
        let mut mi = MultiInterval::new();
        mi.add(Interval::closed(0.0, 10.0));
        mi.add(Interval::closed(20.0, 30.0));

        mi.intersect(Interval::closed(5.0, 25.0));

        assert!(!mi.contains_value(3.0));
        assert!(mi.contains_value(7.0));
        assert!(!mi.contains_value(15.0));
        assert!(mi.contains_value(22.0));
        assert!(!mi.contains_value(27.0));
    }
}
