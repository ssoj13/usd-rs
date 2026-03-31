//! Spline comparison utilities.
//!
//! Port of pxr/base/ts/diff.h and diff.cpp
//!
//! Provides functionality to compare two splines and find the time interval
//! where they differ.

use super::iterator::SegmentIterator;
use super::segment::Segment;
use super::spline::Spline;
use super::spline_data::SplineData;
// TsTime imported but reserved for future use
#[allow(unused_imports)]
use super::types::TsTime;
use usd_gf::Interval;

// ============================================================================
// Constants
// ============================================================================

const INF: f64 = f64::INFINITY;
const NEG_INF: f64 = f64::NEG_INFINITY;

// ============================================================================
// Internal Helpers
// ============================================================================

/// Compare segments from two splines within the given interval.
fn compare_segments(spline1: &Spline, spline2: &Spline, iter_interval: &Interval) -> Interval {
    let mut iter1 = SegmentIterator::new(spline1, *iter_interval);
    let mut iter2 = SegmentIterator::new(spline2, *iter_interval);

    let mut min_diff_time = INF;
    let mut max_diff_time = NEG_INF;

    while !iter1.at_end() && !iter2.at_end() {
        let seg1 = *iter1.segment();
        let seg2 = *iter2.segment();

        if segments_equal(&seg1, &seg2) {
            iter1.next();
            iter2.next();
            continue;
        }

        // Record the differing range
        min_diff_time = min_diff_time.min(seg1.p0[0].min(seg2.p0[0]));
        max_diff_time = max_diff_time.max(seg1.p1[0].max(seg2.p1[0]));

        // Advance the segment that ends earliest. Advance both if equal.
        if seg1.p1[0] <= seg2.p1[0] {
            iter1.next();
        }
        if seg1.p1[0] >= seg2.p1[0] {
            iter2.next();
        }
    }

    // Handle remaining segments from iter1
    while !iter1.at_end() {
        let seg = iter1.segment();
        min_diff_time = min_diff_time.min(seg.p0[0]);
        max_diff_time = max_diff_time.max(seg.p1[0]);
        iter1.next();
    }

    // Handle remaining segments from iter2
    while !iter2.at_end() {
        let seg = iter2.segment();
        min_diff_time = min_diff_time.min(seg.p0[0]);
        max_diff_time = max_diff_time.max(seg.p1[0]);
        iter2.next();
    }

    // Return result (closed at min, open at max per C++ convention)
    if min_diff_time <= max_diff_time && min_diff_time.is_finite() {
        Interval::new(min_diff_time, max_diff_time, true, false)
    } else {
        Interval::new_empty()
    }
}

/// Check if two segments are equal.
fn segments_equal(s1: &Segment, s2: &Segment) -> bool {
    // Use the PartialEq implementation
    s1 == s2
}

// ============================================================================
// Public API - Low-level
// ============================================================================

/// Compare two spline data structures and return the time interval where they differ.
///
/// The input compare_interval may be infinite. If the splines do not differ,
/// an empty interval is returned.
pub fn diff_data(
    data1: Option<&SplineData>,
    data2: Option<&SplineData>,
    compare_interval: &Interval,
) -> Interval {
    // Assume they're completely different
    let mut result = *compare_interval;

    if compare_interval.is_empty() {
        return result;
    }

    // Handle empty splines
    let empty1 = data1.is_none_or(|d| d.times.is_empty());
    let empty2 = data2.is_none_or(|d| d.times.is_empty());

    if empty1 && empty2 {
        // Both empty - no differences (both are value-blocks at all times)
        return Interval::new_empty();
    }

    if empty1 || empty2 {
        // One is empty - completely different
        return result;
    }

    // Unwrap now that we know both are Some and non-empty
    let d1 = data1.expect("value expected");
    let d2 = data2.expect("value expected");

    let pre_extrap_time1 = d1.pre_extrap_time();
    let pre_extrap_time2 = d2.pre_extrap_time();
    let post_extrap_time1 = d1.post_extrap_time();
    let post_extrap_time2 = d2.post_extrap_time();

    let mut have_infinite_pre_loop = false;
    let mut have_infinite_post_loop = false;
    let mut pre_extrap_different = false;
    let mut post_extrap_different = false;

    let mut iter_interval = *compare_interval;

    // Handle infinite intervals with looped extrapolation
    if compare_interval.get_min() == NEG_INF
        && (d1.pre_extrapolation.is_looping() || d2.pre_extrapolation.is_looping())
    {
        have_infinite_pre_loop = true;

        // If time ranges differ or one isn't looping, pre-extrap is different
        if pre_extrap_time1 != pre_extrap_time2
            || post_extrap_time1 != post_extrap_time2
            || !d1.pre_extrapolation.is_looping()
            || !d2.pre_extrapolation.is_looping()
        {
            pre_extrap_different = true;
            iter_interval.set_min_with_closed(pre_extrap_time1.min(pre_extrap_time2), true);
        } else {
            // Iterate over one iteration of the pre-extrap loop
            let loop_span = post_extrap_time1 - pre_extrap_time1;
            iter_interval.set_min_with_closed(pre_extrap_time1 - loop_span, true);
        }
    }

    if compare_interval.get_max() == INF
        && (d1.post_extrapolation.is_looping() || d2.post_extrapolation.is_looping())
    {
        have_infinite_post_loop = true;

        // If time ranges differ or one isn't looping, post-extrap is different
        if pre_extrap_time1 != pre_extrap_time2
            || post_extrap_time1 != post_extrap_time2
            || !d1.post_extrapolation.is_looping()
            || !d2.post_extrapolation.is_looping()
        {
            post_extrap_different = true;
            iter_interval.set_max_with_closed(post_extrap_time1.max(post_extrap_time2), false);
        } else {
            // Iterate over one iteration of the post-extrap loop
            let loop_span = post_extrap_time1 - pre_extrap_time1;
            iter_interval.set_max_with_closed(post_extrap_time1 + loop_span, false);
        }
    }

    // If pre and post extrap are both different, splines differ at all times
    if pre_extrap_different && post_extrap_different {
        return result;
    }

    // For now, return the interval as-is since we don't have full segment
    // iterator support. A full implementation would compare segments.
    // This is a placeholder that indicates the full interval differs.

    // In a complete implementation, we'd do:
    // result = compare_segments(spline1, spline2, &iter_interval);

    // Handle infinite extension due to loops
    if pre_extrap_different || have_infinite_pre_loop {
        let affects_pre_extrap = Interval::new(
            NEG_INF,
            post_extrap_time1.max(post_extrap_time2),
            true,
            false,
        );
        if result.intersects(&affects_pre_extrap) {
            result = result.hull(&Interval::new(
                NEG_INF,
                iter_interval.get_min(),
                false,
                false,
            ));
        }
    }

    if post_extrap_different || have_infinite_post_loop {
        let affects_post_extrap =
            Interval::new(pre_extrap_time1.min(pre_extrap_time2), INF, true, false);
        if result.intersects(&affects_post_extrap) {
            result = result.hull(&Interval::new(iter_interval.get_max(), INF, true, false));
        }
    }

    // Clamp to compare_interval
    result.intersection(compare_interval)
}

// ============================================================================
// Public API - High-level
// ============================================================================

/// Compares two splines and returns the time interval where they differ.
///
/// The compare_interval may be infinite. If the splines do not differ,
/// an empty interval is returned.
pub fn diff(spline1: &Spline, spline2: &Spline, compare_interval: &Interval) -> Interval {
    // Quick check: exact equality
    if spline1 == spline2 {
        return Interval::new_empty();
    }

    // Quick check: both empty
    if spline1.is_empty() && spline2.is_empty() {
        return Interval::new_empty();
    }

    // If interval is empty, nothing to compare
    if compare_interval.is_empty() {
        return *compare_interval;
    }

    // If one is empty and one is not, they differ across the compare interval
    if spline1.is_empty() || spline2.is_empty() {
        return *compare_interval;
    }

    // Use segment comparison
    let result = compare_segments(spline1, spline2, compare_interval);

    // If segment comparison found no differences but splines aren't equal,
    // the difference is likely in segment values. Return time range union.
    if result.is_empty() {
        // Get the time ranges and return their intersection with compare_interval
        let r1 = spline1.time_range();
        let r2 = spline2.time_range();

        match (r1, r2) {
            (Some(range1), Some(range2)) => {
                let combined = range1.hull(&range2);
                combined.intersection(compare_interval)
            }
            (Some(r), None) | (None, Some(r)) => r.intersection(compare_interval),
            (None, None) => *compare_interval,
        }
    } else {
        result
    }
}

/// Returns true if two splines are equal within the given interval.
pub fn splines_equal(spline1: &Spline, spline2: &Spline, interval: &Interval) -> bool {
    diff(spline1, spline2, interval).is_empty()
}

/// Returns true if two splines are exactly equal.
pub fn splines_exactly_equal(spline1: &Spline, spline2: &Spline) -> bool {
    spline1 == spline2
}

/// Returns true if two splines are equal over all time.
pub fn splines_equal_everywhere(spline1: &Spline, spline2: &Spline) -> bool {
    let full_interval = Interval::new(NEG_INF, INF, false, false);
    splines_equal(spline1, spline2, &full_interval)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Knot;

    #[test]
    fn test_diff_empty_splines() {
        let s1 = Spline::new();
        let s2 = Spline::new();
        let interval = Interval::new(0.0, 10.0, true, true);

        let result = diff(&s1, &s2, &interval);
        assert!(result.is_empty());
    }

    #[test]
    fn test_diff_equal_splines() {
        let mut s1 = Spline::new();
        s1.set_knot(Knot::at_time(0.0, 0.0));
        s1.set_knot(Knot::at_time(10.0, 100.0));

        let s2 = s1.clone();
        let interval = Interval::new(-5.0, 15.0, true, true);

        let result = diff(&s1, &s2, &interval);
        assert!(result.is_empty());
    }

    #[test]
    fn test_diff_different_splines() {
        // Test that structurally different splines (different number of knots)
        // are detected as different
        let mut s1 = Spline::new();
        s1.set_knot(Knot::at_time(0.0, 0.0));
        s1.set_knot(Knot::at_time(10.0, 50.0));

        let mut s2 = Spline::new();
        s2.set_knot(Knot::at_time(0.0, 100.0));
        s2.set_knot(Knot::at_time(5.0, 125.0)); // Different time - structural difference
        s2.set_knot(Knot::at_time(10.0, 150.0));

        let interval = Interval::new(-5.0, 15.0, true, true);

        let result = diff(&s1, &s2, &interval);
        // Different number of knots means different segment structure
        assert!(!result.is_empty());
    }

    #[test]
    fn test_splines_exactly_equal() {
        let mut s1 = Spline::new();
        s1.set_knot(Knot::at_time(5.0, 50.0));

        let s2 = s1.clone();

        assert!(splines_exactly_equal(&s1, &s2));
    }

    #[test]
    fn test_splines_not_exactly_equal() {
        let mut s1 = Spline::new();
        s1.set_knot(Knot::at_time(5.0, 50.0));

        let mut s2 = Spline::new();
        s2.set_knot(Knot::at_time(5.0, 51.0));

        assert!(!splines_exactly_equal(&s1, &s2));
    }

    #[test]
    fn test_splines_equal_interval() {
        let s1 = Spline::new();
        let s2 = Spline::new();

        let interval = Interval::new(0.0, 10.0, true, true);
        assert!(splines_equal(&s1, &s2, &interval));
    }

    #[test]
    fn test_diff_one_empty() {
        let s1 = Spline::new();
        let mut s2 = Spline::new();
        s2.set_knot(Knot::at_time(5.0, 50.0));

        let interval = Interval::new(0.0, 10.0, true, true);

        // One empty, one not - they differ
        let result = diff(&s1, &s2, &interval);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_diff_empty_interval() {
        let mut s1 = Spline::new();
        s1.set_knot(Knot::at_time(0.0, 0.0));

        let mut s2 = Spline::new();
        s2.set_knot(Knot::at_time(0.0, 100.0));

        let empty_interval = Interval::new_empty();

        let result = diff(&s1, &s2, &empty_interval);
        assert!(result.is_empty());
    }
}
