//! TsKnotMap - Sorted collection of knots.
//!
//! Port of pxr/base/ts/knotMap.h

use super::knot::Knot;
use super::knot_data::KnotValueType;
use super::types::{InterpMode, TsTime};
use std::ops::{Index, IndexMut};
use usd_gf::Interval;

/// A sorted collection of knots.
///
/// Stored as a vector but maintains uniqueness and sorting like a map.
/// A knot's time is stored within the knot itself but also used as a key.
#[derive(Debug, Clone, Default)]
pub struct KnotMap {
    knots: Vec<Knot>,
}

impl KnotMap {
    /// Creates an empty knot map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a knot map from a slice of knots.
    pub fn from_knots(knots: impl IntoIterator<Item = Knot>) -> Self {
        let mut map = Self::new();
        for knot in knots {
            map.insert(knot);
        }
        map
    }

    /// Returns true if the map is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.knots.is_empty()
    }

    /// Returns the number of knots.
    #[inline]
    pub fn len(&self) -> usize {
        self.knots.len()
    }

    /// Clears all knots.
    pub fn clear(&mut self) {
        self.knots.clear();
    }

    /// Returns an iterator over the knots.
    pub fn iter(&self) -> impl Iterator<Item = &Knot> {
        self.knots.iter()
    }

    /// Returns a mutable iterator over the knots.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Knot> {
        self.knots.iter_mut()
    }

    /// Returns a reverse iterator over the knots.
    pub fn iter_rev(&self) -> impl Iterator<Item = &Knot> {
        self.knots.iter().rev()
    }

    /// Returns the first knot, if any.
    pub fn first(&self) -> Option<&Knot> {
        self.knots.first()
    }

    /// Returns the last knot, if any.
    pub fn last(&self) -> Option<&Knot> {
        self.knots.last()
    }

    /// Returns a mutable reference to the first knot.
    pub fn first_mut(&mut self) -> Option<&mut Knot> {
        self.knots.first_mut()
    }

    /// Returns a mutable reference to the last knot.
    pub fn last_mut(&mut self) -> Option<&mut Knot> {
        self.knots.last_mut()
    }

    /// Returns the time range of the knots.
    pub fn time_range(&self) -> Option<Interval> {
        if self.knots.is_empty() {
            return None;
        }
        let first = self.knots.first()?.time();
        let last = self.knots.last()?.time();
        Some(Interval::new(first, last, true, true))
    }

    /// Finds the index where a knot with the given time would be inserted.
    fn find_insert_pos(&self, time: TsTime) -> usize {
        self.knots
            .binary_search_by(|k| k.time().partial_cmp(&time).expect("value expected"))
            .unwrap_or_else(|i| i)
    }

    /// Returns true if a knot exists at the given time.
    pub fn contains(&self, time: TsTime) -> bool {
        self.find(time).is_some()
    }

    /// Finds a knot by time.
    pub fn find(&self, time: TsTime) -> Option<&Knot> {
        let idx = self.find_insert_pos(time);
        if idx < self.knots.len() && (self.knots[idx].time() - time).abs() < 1e-10 {
            Some(&self.knots[idx])
        } else {
            None
        }
    }

    /// Finds a mutable knot by time.
    pub fn find_mut(&mut self, time: TsTime) -> Option<&mut Knot> {
        let idx = self.find_insert_pos(time);
        if idx < self.knots.len() && (self.knots[idx].time() - time).abs() < 1e-10 {
            Some(&mut self.knots[idx])
        } else {
            None
        }
    }

    /// Inserts a knot, replacing any existing knot at the same time.
    /// Returns true if a knot was replaced.
    pub fn insert(&mut self, knot: Knot) -> bool {
        let time = knot.time();
        let idx = self.find_insert_pos(time);

        if idx < self.knots.len() && (self.knots[idx].time() - time).abs() < 1e-10 {
            self.knots[idx] = knot;
            true
        } else {
            self.knots.insert(idx, knot);
            false
        }
    }

    /// Removes the knot at the given time.
    /// Returns the removed knot if it existed.
    pub fn remove(&mut self, time: TsTime) -> Option<Knot> {
        let idx = self.find_insert_pos(time);
        if idx < self.knots.len() && (self.knots[idx].time() - time).abs() < 1e-10 {
            Some(self.knots.remove(idx))
        } else {
            None
        }
    }

    /// Removes knots in the given time range.
    pub fn remove_range(&mut self, start: TsTime, end: TsTime) {
        self.knots.retain(|k| k.time() < start || k.time() > end);
    }

    /// Returns the knot at or before the given time.
    pub fn lower_bound(&self, time: TsTime) -> Option<&Knot> {
        let idx = self.find_insert_pos(time);
        if idx < self.knots.len() && (self.knots[idx].time() - time).abs() < 1e-10 {
            Some(&self.knots[idx])
        } else if idx > 0 {
            Some(&self.knots[idx - 1])
        } else {
            None
        }
    }

    /// Returns the knot at or after the given time.
    pub fn upper_bound(&self, time: TsTime) -> Option<&Knot> {
        let idx = self.find_insert_pos(time);
        if idx < self.knots.len() {
            Some(&self.knots[idx])
        } else {
            None
        }
    }

    /// Returns the knot whose time most closely matches the specified time.
    ///
    /// In case of ties (equal distance to two knots), returns the later knot.
    /// Returns `None` if the map is empty.
    ///
    /// Matches C++ `TsKnotMap::FindClosest(TsTime)`.
    #[must_use]
    pub fn find_closest(&self, time: TsTime) -> Option<&Knot> {
        if self.knots.is_empty() {
            return None;
        }

        let idx = self.find_insert_pos(time);

        // Time before first knot -> return first
        if idx == 0 {
            return Some(&self.knots[0]);
        }

        // Time after last knot -> return last
        if idx >= self.knots.len() {
            return Some(self.knots.last().unwrap());
        }

        // Exact match
        if (self.knots[idx].time() - time).abs() < 1e-10 {
            return Some(&self.knots[idx]);
        }

        // Between knots: compare distances, ties go to later knot
        let prev_gap = time - self.knots[idx - 1].time();
        let next_gap = self.knots[idx].time() - time;
        if next_gap > prev_gap {
            Some(&self.knots[idx - 1])
        } else {
            Some(&self.knots[idx])
        }
    }

    /// Returns the value type of the knots, or `None` if empty.
    ///
    /// Matches C++ `TsKnotMap::GetValueType()`.
    #[must_use]
    pub fn value_type(&self) -> Option<KnotValueType> {
        self.knots.first().map(|k| k.value_type())
    }

    /// Returns whether there are any segments with curve interpolation.
    ///
    /// Matches C++ `TsKnotMap::HasCurveSegments()`.
    #[must_use]
    pub fn has_curve_segments(&self) -> bool {
        self.knots
            .windows(2)
            .any(|w| w[0].interp_mode() == InterpMode::Curve)
    }

    /// Returns surrounding knots (before and after) for interpolation.
    pub fn surrounding(&self, time: TsTime) -> (Option<&Knot>, Option<&Knot>) {
        if self.knots.is_empty() {
            return (None, None);
        }

        let idx = self.find_insert_pos(time);

        // Exact match
        if idx < self.knots.len() && (self.knots[idx].time() - time).abs() < 1e-10 {
            return (Some(&self.knots[idx]), Some(&self.knots[idx]));
        }

        let before = if idx > 0 {
            Some(&self.knots[idx - 1])
        } else {
            None
        };
        let after = if idx < self.knots.len() {
            Some(&self.knots[idx])
        } else {
            None
        };

        (before, after)
    }

    /// Offsets all knot times by the given delta.
    pub fn offset_time(&mut self, delta: TsTime) {
        for knot in &mut self.knots {
            let new_time = knot.time() + delta;
            knot.set_time(new_time);
        }
    }

    /// Scales all knot times by the given factor around the given pivot.
    pub fn scale_time(&mut self, factor: f64, pivot: TsTime) {
        for knot in &mut self.knots {
            let t = knot.time();
            let new_time = pivot + (t - pivot) * factor;
            knot.set_time(new_time);
        }
    }

    /// Offsets all knot values by the given delta.
    pub fn offset_value(&mut self, delta: f64) {
        for knot in &mut self.knots {
            knot.set_value(knot.value() + delta);
        }
    }

    /// Scales all knot values by the given factor.
    pub fn scale_value(&mut self, factor: f64) {
        for knot in &mut self.knots {
            knot.set_value(knot.value() * factor);
        }
    }

    /// Returns knots as a slice.
    pub fn as_slice(&self) -> &[Knot] {
        &self.knots
    }

    /// Returns knots as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [Knot] {
        &mut self.knots
    }

    /// Takes ownership of all knots.
    pub fn into_vec(self) -> Vec<Knot> {
        self.knots
    }
}

impl PartialEq for KnotMap {
    fn eq(&self, other: &Self) -> bool {
        if self.knots.len() != other.knots.len() {
            return false;
        }
        self.knots
            .iter()
            .zip(other.knots.iter())
            .all(|(a, b)| a == b)
    }
}

impl Index<usize> for KnotMap {
    type Output = Knot;

    fn index(&self, idx: usize) -> &Self::Output {
        &self.knots[idx]
    }
}

impl IndexMut<usize> for KnotMap {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        &mut self.knots[idx]
    }
}

impl IntoIterator for KnotMap {
    type Item = Knot;
    type IntoIter = std::vec::IntoIter<Knot>;

    fn into_iter(self) -> Self::IntoIter {
        self.knots.into_iter()
    }
}

impl<'a> IntoIterator for &'a KnotMap {
    type Item = &'a Knot;
    type IntoIter = std::slice::Iter<'a, Knot>;

    fn into_iter(self) -> Self::IntoIter {
        self.knots.iter()
    }
}

impl FromIterator<Knot> for KnotMap {
    fn from_iter<I: IntoIterator<Item = Knot>>(iter: I) -> Self {
        Self::from_knots(iter)
    }
}

impl From<Vec<Knot>> for KnotMap {
    fn from(knots: Vec<Knot>) -> Self {
        Self::from_knots(knots)
    }
}
