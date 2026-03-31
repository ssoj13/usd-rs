//! USD value utility functions.
//!
//! Port of pxr/usd/usd/valueUtils.h
//!
//! Helper functions for working with USD values, including:
//! - Block detection and clearing (SdfValueBlock, SdfAnimationBlock)
//! - Default value resolution
//! - Time sample querying and merging
//! - Layer offset application to time-mapped values
//! - List position insertion helpers
//! - Dictionary value resolution

use crate::common::ListPosition;
use std::collections::BTreeMap;
use usd_gf::Interval;
use usd_sdf::{LayerOffset, TimeCode};
use usd_vt::Value;

/// Result of checking for a default value on a spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultValueResult {
    /// No default value found.
    None,
    /// A default value was found.
    Found,
    /// The default value is blocked (SdfValueBlock).
    Blocked,
    /// The default value is an animation block.
    BlockedAnimation,
}

/// Returns true if the value is a "value block" (sentinel that blocks
/// inheritance of a value).
pub fn value_contains_block(value: &Value) -> bool {
    value.type_name() == Some("SdfValueBlock")
}

/// Returns true if the value is an animation block.
pub fn value_contains_animation_block(value: &Value) -> bool {
    value.type_name() == Some("SdfAnimationBlock")
}

/// If the value contains a block, clears it and returns true.
/// Otherwise returns false.
pub fn clear_value_if_blocked(value: &mut Value) -> bool {
    if value_contains_block(value) {
        *value = Value::default();
        true
    } else {
        false
    }
}

/// If the value contains an animation block, clears it and returns true.
pub fn clear_value_if_animation_blocked(value: &mut Value) -> bool {
    if value_contains_animation_block(value) {
        *value = Value::default();
        true
    } else {
        false
    }
}

/// Merges additional time samples into an existing sorted vector.
///
/// Both `time_samples` and `additional` must be pre-sorted.
/// The result in `time_samples` will contain the sorted union.
pub fn merge_time_samples(time_samples: &mut Vec<f64>, additional: &[f64]) {
    if additional.is_empty() {
        return;
    }
    if time_samples.is_empty() {
        time_samples.extend_from_slice(additional);
        return;
    }

    let mut merged = Vec::with_capacity(time_samples.len() + additional.len());
    let mut i = 0;
    let mut j = 0;

    while i < time_samples.len() && j < additional.len() {
        if time_samples[i] < additional[j] {
            merged.push(time_samples[i]);
            i += 1;
        } else if time_samples[i] > additional[j] {
            merged.push(additional[j]);
            j += 1;
        } else {
            // Equal: take one copy.
            merged.push(time_samples[i]);
            i += 1;
            j += 1;
        }
    }
    merged.extend_from_slice(&time_samples[i..]);
    merged.extend_from_slice(&additional[j..]);

    *time_samples = merged;
}

/// Copies time samples from `samples` that fall within `interval` to `output`.
///
/// Uses half-open interval semantics: `[min, max)` by default.
/// Matches `Usd_CopyTimeSamplesInInterval` in `pxr/usd/usd/valueUtils.h`.
///
/// `samples_sorted` must be sorted ascending (as from `ListTimeSamplesForPath` / ordered set).
pub fn usd_copy_time_samples_in_interval(samples_sorted: &[f64], interval: &Interval) -> Vec<f64> {
    if samples_sorted.is_empty() {
        return Vec::new();
    }
    let start = if interval.is_min_open() {
        samples_sorted.partition_point(|&t| t <= interval.get_min())
    } else {
        samples_sorted.partition_point(|&t| t < interval.get_min())
    };
    let end = if interval.is_max_open() {
        samples_sorted.partition_point(|&t| t < interval.get_max())
    } else {
        samples_sorted.partition_point(|&t| t <= interval.get_max())
    };
    if start < end {
        samples_sorted[start..end].to_vec()
    } else {
        Vec::new()
    }
}

pub fn copy_time_samples_in_interval(
    samples: &[f64],
    interval_min: f64,
    interval_max: f64,
    min_open: bool,
    max_open: bool,
    output: &mut Vec<f64>,
) {
    for &s in samples {
        let above_min = if min_open {
            s > interval_min
        } else {
            s >= interval_min
        };
        let below_max = if max_open {
            s < interval_max
        } else {
            s <= interval_max
        };
        if above_min && below_max {
            output.push(s);
        }
    }
}

/// Applies a layer offset to a TimeCode value.
pub fn apply_layer_offset_to_time_code(value: &mut TimeCode, offset: &LayerOffset) {
    let new_time = offset.offset() + offset.scale() * value.value();
    *value = TimeCode::new(new_time);
}

/// Applies a layer offset to a vector of TimeCode values.
pub fn apply_layer_offset_to_time_codes(values: &mut [TimeCode], offset: &LayerOffset) {
    for tc in values.iter_mut() {
        apply_layer_offset_to_time_code(tc, offset);
    }
}

/// Applies a layer offset to a time sample map.
///
/// Both keys (times) and time-valued values are offset.
pub fn apply_layer_offset_to_time_sample_map(
    samples: &mut BTreeMap<ordered_float::OrderedFloat<f64>, Value>,
    offset: &LayerOffset,
) {
    let orig: BTreeMap<_, _> = std::mem::take(samples);
    for (time, value) in orig {
        let new_time = offset.offset() + offset.scale() * time.0;
        samples.insert(ordered_float::OrderedFloat(new_time), value);
    }
}

/// Inserts an item into a list proxy at the position specified by `ListPosition`.
///
/// If the list op is in explicit mode, the item goes into the explicit list
/// regardless of the position enum. If the item already exists but not at
/// the requested position, it is moved.
pub fn insert_list_item<T: Clone + PartialEq>(list: &mut Vec<T>, item: T, position: ListPosition) {
    // Remove existing occurrence if present.
    if let Some(pos) = list.iter().position(|v| v == &item) {
        list.remove(pos);
    }

    match position {
        ListPosition::FrontOfPrependList | ListPosition::FrontOfAppendList => {
            list.insert(0, item);
        }
        ListPosition::BackOfPrependList | ListPosition::BackOfAppendList => {
            list.push(item);
        }
    }
}

/// Resolves all values in a dictionary using the given resolver function.
///
/// Recursively descends into nested dictionaries.
pub fn resolve_values_in_dictionary<F>(
    dict: &mut std::collections::HashMap<String, Value>,
    resolve: &F,
) where
    F: Fn(&mut Value),
{
    for value in dict.values_mut() {
        resolve(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_time_samples() {
        let mut a = vec![1.0, 3.0, 5.0];
        let b = vec![2.0, 3.0, 4.0];
        merge_time_samples(&mut a, &b);
        assert_eq!(a, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn test_merge_empty() {
        let mut a = vec![1.0, 2.0];
        merge_time_samples(&mut a, &[]);
        assert_eq!(a, vec![1.0, 2.0]);

        let mut b: Vec<f64> = vec![];
        merge_time_samples(&mut b, &[3.0, 4.0]);
        assert_eq!(b, vec![3.0, 4.0]);
    }

    #[test]
    fn test_copy_time_samples_in_interval() {
        let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mut output = Vec::new();
        copy_time_samples_in_interval(&samples, 2.0, 4.0, false, true, &mut output);
        assert_eq!(output, vec![2.0, 3.0]);
    }

    #[test]
    fn test_insert_list_item_front() {
        let mut list = vec![1, 2, 3];
        insert_list_item(&mut list, 4, ListPosition::FrontOfPrependList);
        assert_eq!(list[0], 4);
    }

    #[test]
    fn test_insert_list_item_back() {
        let mut list = vec![1, 2, 3];
        insert_list_item(&mut list, 4, ListPosition::BackOfAppendList);
        assert_eq!(list[list.len() - 1], 4);
    }

    #[test]
    fn test_insert_list_item_move() {
        let mut list = vec![1, 2, 3];
        insert_list_item(&mut list, 2, ListPosition::FrontOfPrependList);
        assert_eq!(list, vec![2, 1, 3]);
    }

    #[test]
    fn test_default_value_result() {
        assert_ne!(DefaultValueResult::None, DefaultValueResult::Found);
        assert_ne!(
            DefaultValueResult::Blocked,
            DefaultValueResult::BlockedAnimation
        );
    }
}
