#![allow(dead_code)]

//! HdStBufferArrayRegistry - Registry managing buffer array pools by key.
//!
//! Maintains a concurrent map of aggregation ID -> list of buffer arrays.
//! When a new range is requested, the registry finds (or creates) a buffer
//! array with matching specs and tries to assign the range into it.
//!
//! Port of pxr/imaging/hdSt/bufferArrayRegistry.h

use crate::strategy_base::{AggregationId, HdStAggregationStrategy};
use usd_hd::resource::{
    HdBufferArrayHandle, HdBufferArrayRangeHandle, HdBufferArrayUsageHint, HdBufferSpecVector,
};
use usd_tf::Token;
use std::collections::HashMap;
use std::sync::Mutex;

/// Entry in the buffer array cache.
///
/// Each entry holds a list of buffer arrays that share the same aggregation ID
/// (i.e., same buffer spec format). Multiple buffer arrays exist when one
/// becomes full and a new one must be created.
struct Entry {
    buffer_arrays: Vec<HdBufferArrayHandle>,
}

impl Entry {
    fn new() -> Self {
        Self {
            buffer_arrays: Vec::new(),
        }
    }
}

/// Registry managing buffer array pools.
///
/// Thread-safe registry that maps aggregation IDs to lists of buffer arrays.
/// Used by the resource registry to allocate ranges from appropriately-typed
/// buffer arrays.
///
/// # Allocation Flow
///
/// 1. Compute aggregation ID from buffer specs
/// 2. Look up or create entry for that ID
/// 3. Try to assign range to existing buffer arrays in the entry
/// 4. If all full, create a new buffer array and retry
///
/// # Garbage Collection
///
/// Periodically call `garbage_collect()` to free buffer arrays that
/// no longer contain any active ranges.
///
/// Port of HdStBufferArrayRegistry from pxr/imaging/hdSt/bufferArrayRegistry.h
pub struct HdStBufferArrayRegistry {
    entries: Mutex<HashMap<AggregationId, Entry>>,
}

impl HdStBufferArrayRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Allocate a new buffer array range using the given strategy.
    ///
    /// Finds or creates a buffer array matching the specs/usage, then
    /// assigns a range within it.
    ///
    /// Returns None if specs are empty or assignment fails after retries.
    pub fn allocate_range(
        &self,
        strategy: &dyn HdStAggregationStrategy,
        role: &Token,
        buffer_specs: &HdBufferSpecVector,
        usage_hint: HdBufferArrayUsageHint,
    ) -> Option<HdBufferArrayRangeHandle> {
        if buffer_specs.is_empty() {
            return None;
        }

        let aggr_id = strategy.compute_aggregation_id(buffer_specs, usage_hint);
        let range = strategy.create_buffer_array_range();

        let mut entries = self.entries.lock().unwrap();
        let entry = entries.entry(aggr_id).or_insert_with(Entry::new);

        // If entry is empty, create the first buffer array
        if entry.buffer_arrays.is_empty() {
            let new_array = strategy.create_buffer_array(role, buffer_specs, usage_hint);
            entry.buffer_arrays.push(new_array);
        }

        // Try to assign range to existing buffer arrays
        let mut iterations = 0;
        let max_iterations = 100;

        loop {
            let mut assigned = false;

            for array in &entry.buffer_arrays {
                if array.try_assign_range(range.clone()) {
                    assigned = true;
                    break;
                }
            }

            if assigned {
                return Some(range);
            }

            // All arrays full, create a new one
            let new_array = strategy.create_buffer_array(role, buffer_specs, usage_hint);
            entry.buffer_arrays.push(new_array);

            iterations += 1;
            if iterations > max_iterations {
                log::warn!(
                    "Too many iterations assigning range for buffer '{}', likely invalid buffer size",
                    if buffer_specs.is_empty() {
                        "<empty>"
                    } else {
                        buffer_specs[0].name.as_str()
                    }
                );
                return None;
            }
        }
    }

    /// Trigger reallocation on all buffer arrays managed by the registry.
    ///
    /// Used after buffer arrays are resized or ranges are migrated.
    /// Handles over-aggregation by splitting buffer arrays when needed.
    pub fn reallocate_all(&self, strategy: &dyn HdStAggregationStrategy) {
        let mut entries = self.entries.lock().unwrap();

        for entry in entries.values_mut() {
            let mut new_arrays = Vec::new();

            for array in &entry.buffer_arrays {
                if !array.needs_reallocation() {
                    continue;
                }

                // Collect valid ranges and reallocate
                let max_elements = array.get_max_num_elements();
                let range_count = array.get_range_count();
                let mut ranges: Vec<HdBufferArrayRangeHandle> = Vec::with_capacity(range_count);
                let mut total_elements = 0usize;

                for idx in 0..range_count {
                    // get_range returns Option<Weak<...>>, upgrade to Arc
                    let range = match array.get_range(idx).and_then(|w| w.upgrade()) {
                        Some(r) => r,
                        None => continue,
                    };

                    let num_elements = range.get_num_elements();

                    // Check for over-aggregation
                    if total_elements + num_elements > max_elements && !ranges.is_empty() {
                        // Split: create new buffer array for overflow
                        let specs = strategy.get_buffer_specs(array);
                        let new_array = strategy.create_buffer_array(
                            array.get_role(),
                            &specs,
                            array.get_usage_hint(),
                        );
                        new_array.reallocate(&ranges, Some(array.clone()));
                        new_arrays.push(new_array);

                        total_elements = 0;
                        ranges.clear();
                    }

                    total_elements += num_elements;
                    ranges.push(range);
                }

                // Reallocate remaining ranges in original array
                if !ranges.is_empty() {
                    array.reallocate(&ranges, Some(array.clone()));
                }
            }

            // Add any newly-created split arrays
            entry.buffer_arrays.extend(new_arrays);
        }
    }

    /// Free buffer arrays that no longer contain any allocated ranges.
    pub fn garbage_collect(&self) {
        let mut entries = self.entries.lock().unwrap();

        entries.retain(|_id, entry| {
            entry
                .buffer_arrays
                .retain(|array| !array.garbage_collect());
            !entry.buffer_arrays.is_empty()
        });
    }

    /// Get total GPU memory used across all managed buffer arrays.
    pub fn get_resource_allocation(
        &self,
        strategy: &dyn HdStAggregationStrategy,
        result: &mut HashMap<String, usize>,
    ) -> usize {
        let entries = self.entries.lock().unwrap();
        let mut total = 0usize;

        for entry in entries.values() {
            for array in &entry.buffer_arrays {
                total += strategy.get_resource_allocation(array, result);
            }
        }

        total
    }

    /// Get total number of buffer arrays across all entries.
    pub fn get_buffer_array_count(&self) -> usize {
        let entries = self.entries.lock().unwrap();
        entries.values().map(|e| e.buffer_arrays.len()).sum()
    }

    /// Get number of distinct aggregation entries.
    pub fn get_entry_count(&self) -> usize {
        self.entries.lock().unwrap().len()
    }
}

impl Default for HdStBufferArrayRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = HdStBufferArrayRegistry::new();
        assert_eq!(registry.get_entry_count(), 0);
        assert_eq!(registry.get_buffer_array_count(), 0);
    }

    #[test]
    fn test_garbage_collect_empty() {
        let registry = HdStBufferArrayRegistry::new();
        registry.garbage_collect(); // should not panic on empty
        assert_eq!(registry.get_entry_count(), 0);
    }
}
