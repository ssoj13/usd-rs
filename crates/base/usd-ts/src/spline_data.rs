//! Internal spline data storage.
//!
//! Port of pxr/base/ts/splineData.h
//!
//! This module provides the internal typed storage for splines.
//! SplineData is the base structure with overall parameters.
//! TypedSplineData<T> extends it with typed knot storage.

use super::knot_data::TypedKnotData;
use super::types::{CurveType, Extrapolation, InterpMode, LoopParams, TsTime};
use num_traits::NumCast;
use std::collections::HashMap;

/// Base spline data with overall parameters.
///
/// This is the unit of data managed by shared_ptr/Arc for copy-on-write.
#[derive(Debug, Clone)]
pub struct SplineData {
    /// If true, the value type is authoritative.
    pub is_typed: bool,
    /// Whether to apply offset/scale to values (for time-valued attributes).
    pub time_valued: bool,
    /// Curve type (Bezier or Hermite).
    pub curve_type: CurveType,
    /// Pre-extrapolation settings.
    pub pre_extrapolation: Extrapolation,
    /// Post-extrapolation settings.
    pub post_extrapolation: Extrapolation,
    /// Inner loop parameters.
    pub loop_params: LoopParams,
    /// Duplicate of knot times for fast binary search.
    pub times: Vec<TsTime>,
    /// Custom data for knots, keyed by time.
    pub custom_data: HashMap<OrderedTime, CustomData>,
}

/// Custom data stored per knot.
pub type CustomData = HashMap<String, CustomValue>;

/// Values storable in custom data.
#[derive(Debug, Clone, PartialEq)]
pub enum CustomValue {
    /// Boolean value.
    Bool(bool),
    /// Integer value.
    Int(i64),
    /// Double precision floating point value.
    Double(f64),
    /// String value.
    String(String),
    /// Vector of custom values.
    Vec(Vec<CustomValue>),
    /// Dictionary mapping strings to custom values.
    Dict(HashMap<String, CustomValue>),
}

/// Wrapper for f64 that implements Hash and Eq.
#[derive(Debug, Clone, Copy)]
pub struct OrderedTime(pub TsTime);

impl PartialEq for OrderedTime {
    fn eq(&self, other: &Self) -> bool {
        (self.0 - other.0).abs() < 1e-10
    }
}

impl Eq for OrderedTime {}

impl std::hash::Hash for OrderedTime {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the bits of the f64, handling NaN consistently
        self.0.to_bits().hash(state);
    }
}

impl Default for SplineData {
    fn default() -> Self {
        Self {
            is_typed: false,
            time_valued: false,
            curve_type: CurveType::Bezier,
            pre_extrapolation: Extrapolation::held(),
            post_extrapolation: Extrapolation::held(),
            loop_params: LoopParams::default(),
            times: Vec::new(),
            custom_data: HashMap::new(),
        }
    }
}

impl SplineData {
    /// Creates new spline data.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if inner loops are configured.
    pub fn has_inner_loops(&self) -> bool {
        self.loop_params.is_enabled() && self.first_inner_proto_index().is_some()
    }

    /// Returns the first inner prototype knot index if loops are configured.
    pub fn first_inner_proto_index(&self) -> Option<usize> {
        if !self.loop_params.is_enabled() {
            return None;
        }

        // Find first prototype knot index
        for (i, &time) in self.times.iter().enumerate() {
            if time >= self.loop_params.proto_start {
                return Some(i);
            }
        }

        None
    }

    /// Get knot data as f64 (for use in evaluation).
    pub fn get_knot_data_as_double(&self, index: usize) -> Option<TypedKnotData<f64>> {
        // This is a placeholder - in a full implementation,
        // we'd need to store the actual typed data and convert.
        // For now, return None to indicate no data available.
        let _ = index;
        None
    }

    /// Set knot from f64 data.
    pub fn set_knot_from_double(&mut self, knot: &TypedKnotData<f64>) {
        let time = knot.time();
        let idx = self.lower_bound(time);
        let overwrite = idx < self.times.len() && (self.times[idx] - time).abs() < 1e-10;

        if overwrite {
            self.times[idx] = time;
        } else {
            self.times.insert(idx, time);
        }
    }

    /// Returns the time at which pre-extrapolation ends.
    pub fn pre_extrap_time(&self) -> TsTime {
        self.times.first().copied().unwrap_or(0.0)
    }

    /// Returns the time at which post-extrapolation begins.
    pub fn post_extrap_time(&self) -> TsTime {
        self.times.last().copied().unwrap_or(0.0)
    }

    /// Finds the index for a given time using binary search.
    pub fn find_time_index(&self, time: TsTime) -> Option<usize> {
        self.times
            .binary_search_by(|t| t.partial_cmp(&time).expect("value expected"))
            .ok()
    }

    /// Finds the lower bound index for a time.
    pub fn lower_bound(&self, time: TsTime) -> usize {
        match self
            .times
            .binary_search_by(|t| t.partial_cmp(&time).expect("value expected"))
        {
            Ok(i) => i,
            Err(i) => i,
        }
    }
}

impl PartialEq for SplineData {
    fn eq(&self, other: &Self) -> bool {
        self.is_typed == other.is_typed
            && self.time_valued == other.time_valued
            && self.curve_type == other.curve_type
            && self.pre_extrapolation == other.pre_extrapolation
            && self.post_extrapolation == other.post_extrapolation
            && self.loop_params == other.loop_params
            && self.times == other.times
    }
}

/// Typed spline data with value-specific knot storage.
#[derive(Debug, Clone)]
pub struct TypedSplineData<T: Clone + Default + PartialEq> {
    /// Base data with overall parameters.
    pub base: SplineData,
    /// Per-knot typed data.
    pub knots: Vec<TypedKnotData<T>>,
}

impl<T: Clone + Default + PartialEq> Default for TypedSplineData<T> {
    fn default() -> Self {
        Self {
            base: SplineData::default(),
            knots: Vec::new(),
        }
    }
}

impl<T: Clone + Default + PartialEq + NumCast + std::ops::DivAssign<T> + 'static>
    TypedSplineData<T>
{
    /// Creates new typed spline data.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates typed data with specified curve type.
    pub fn with_curve_type(curve_type: CurveType) -> Self {
        Self {
            base: SplineData {
                curve_type,
                is_typed: true,
                ..Default::default()
            },
            knots: Vec::new(),
        }
    }

    /// Returns the number of knots.
    pub fn knot_count(&self) -> usize {
        self.knots.len()
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.knots.is_empty()
    }

    /// Reserves capacity for the given number of knots.
    pub fn reserve(&mut self, count: usize) {
        self.base.times.reserve(count);
        self.knots.reserve(count);
    }

    /// Appends a knot (assumes times are already sorted).
    pub fn push_knot(&mut self, knot: TypedKnotData<T>, custom: Option<CustomData>) {
        let time = knot.base.time;
        self.base.times.push(time);
        self.knots.push(knot);

        if let Some(data) = custom {
            if !data.is_empty() {
                self.base.custom_data.insert(OrderedTime(time), data);
            }
        }
    }

    /// Sets or replaces a knot at the given time.
    pub fn set_knot(&mut self, knot: TypedKnotData<T>, custom: Option<CustomData>) -> usize {
        let time = knot.base.time;
        let idx = self.base.lower_bound(time);

        let overwrite = idx < self.base.times.len() && (self.base.times[idx] - time).abs() < 1e-10;

        if overwrite {
            self.base.times[idx] = time;
            self.knots[idx] = knot;
        } else {
            self.base.times.insert(idx, time);
            self.knots.insert(idx, knot);
        }

        if let Some(data) = custom {
            if !data.is_empty() {
                self.base.custom_data.insert(OrderedTime(time), data);
            }
        }

        idx
    }

    /// Removes a knot at the given time.
    pub fn remove_knot(&mut self, time: TsTime) -> bool {
        if let Some(idx) = self.base.find_time_index(time) {
            self.base.times.remove(idx);
            self.base.custom_data.remove(&OrderedTime(time));
            self.knots.remove(idx);
            true
        } else {
            false
        }
    }

    /// Clears all knots.
    pub fn clear(&mut self) {
        self.base.times.clear();
        self.base.custom_data.clear();
        self.knots.clear();
    }

    /// Gets a knot by index.
    pub fn get_knot(&self, index: usize) -> Option<&TypedKnotData<T>> {
        self.knots.get(index)
    }

    /// Gets a mutable knot by index.
    pub fn get_knot_mut(&mut self, index: usize) -> Option<&mut TypedKnotData<T>> {
        self.knots.get_mut(index)
    }

    /// Finds a knot by time.
    pub fn find_knot(&self, time: TsTime) -> Option<&TypedKnotData<T>> {
        self.base.find_time_index(time).map(|idx| &self.knots[idx])
    }

    /// Applies time offset and scale to all knots.
    ///
    /// Scale must be positive.
    pub fn apply_offset_and_scale(&mut self, offset: TsTime, scale: f64) {
        if scale <= 0.0 {
            return; // Invalid scale
        }

        // Scale extrapolation slopes
        if self.base.pre_extrapolation.mode == super::types::ExtrapMode::Sloped {
            self.base.pre_extrapolation.slope /= scale;
        }
        if self.base.post_extrapolation.mode == super::types::ExtrapMode::Sloped {
            self.base.post_extrapolation.slope /= scale;
        }

        // Process loop params
        if self.base.loop_params.proto_end > self.base.loop_params.proto_start {
            self.base.loop_params.proto_start = self.base.loop_params.proto_start * scale + offset;
            self.base.loop_params.proto_end = self.base.loop_params.proto_end * scale + offset;
        }

        // Process times
        for time in &mut self.base.times {
            *time = *time * scale + offset;
        }

        // Process knots
        let scale_t: T = NumCast::from(scale).unwrap_or_default();
        for knot in &mut self.knots {
            knot.base.time = knot.base.time * scale + offset;
            knot.base.pre_tan_width *= scale;
            knot.base.post_tan_width *= scale;
            // Slopes are in TypedKnotData - scale inversely
            knot.pre_tan_slope /= scale_t.clone();
            knot.post_tan_slope /= scale_t.clone();
        }

        // Re-index custom data
        let mut new_custom = HashMap::new();
        for (old_time, data) in self.base.custom_data.drain() {
            let new_time = old_time.0 * scale + offset;
            new_custom.insert(OrderedTime(new_time), data);
        }
        self.base.custom_data = new_custom;
    }

    /// Returns true if any knot has a value block.
    pub fn has_value_blocks(&self) -> bool {
        if self.knots.is_empty() {
            return false;
        }

        if self.base.pre_extrapolation.mode == super::types::ExtrapMode::ValueBlock
            || self.base.post_extrapolation.mode == super::types::ExtrapMode::ValueBlock
        {
            return true;
        }

        self.knots
            .iter()
            .any(|k| k.base.next_interp == InterpMode::ValueBlock)
    }

    /// Returns true if there's a value block at the given time.
    pub fn has_value_block_at(&self, time: TsTime) -> bool {
        if self.knots.is_empty() {
            return false;
        }

        let idx = self.base.lower_bound(time);

        if idx >= self.base.times.len() {
            return self.base.post_extrapolation.mode == super::types::ExtrapMode::ValueBlock;
        }

        if (self.base.times[idx] - time).abs() < 1e-10 {
            return self.knots[idx].base.next_interp == InterpMode::ValueBlock;
        }

        if idx == 0 {
            return self.base.pre_extrapolation.mode == super::types::ExtrapMode::ValueBlock;
        }

        // Between knots - check previous knot's interpolation
        self.knots[idx - 1].base.next_interp == InterpMode::ValueBlock
    }
}

impl<T: Clone + Default + PartialEq> PartialEq for TypedSplineData<T> {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && self.knots == other.knots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spline_data_default() {
        let data = SplineData::new();
        assert!(!data.is_typed);
        assert!(!data.time_valued);
        assert!(data.times.is_empty());
    }

    #[test]
    fn test_typed_spline_data_push() {
        let mut data: TypedSplineData<f64> = TypedSplineData::new();

        let mut knot = TypedKnotData::new();
        knot.base.time = 1.0;
        knot.value = 10.0;
        data.push_knot(knot, None);

        let mut knot2 = TypedKnotData::new();
        knot2.base.time = 2.0;
        knot2.value = 20.0;
        data.push_knot(knot2, None);

        assert_eq!(data.knot_count(), 2);
        assert_eq!(data.base.times, vec![1.0, 2.0]);
    }

    #[test]
    fn test_typed_spline_data_set() {
        let mut data: TypedSplineData<f64> = TypedSplineData::new();

        let mut knot1 = TypedKnotData::new();
        knot1.base.time = 2.0;
        knot1.value = 20.0;
        data.set_knot(knot1, None);

        let mut knot2 = TypedKnotData::new();
        knot2.base.time = 1.0;
        knot2.value = 10.0;
        data.set_knot(knot2, None);

        // Should be sorted
        assert_eq!(data.base.times, vec![1.0, 2.0]);
        assert_eq!(data.knots[0].value, 10.0);
        assert_eq!(data.knots[1].value, 20.0);
    }

    #[test]
    fn test_typed_spline_data_remove() {
        let mut data: TypedSplineData<f64> = TypedSplineData::new();

        let mut knot = TypedKnotData::new();
        knot.base.time = 1.0;
        knot.value = 10.0;
        data.push_knot(knot, None);

        assert_eq!(data.knot_count(), 1);
        assert!(data.remove_knot(1.0));
        assert_eq!(data.knot_count(), 0);
    }

    #[test]
    fn test_apply_offset_and_scale() {
        let mut data: TypedSplineData<f64> = TypedSplineData::new();

        let mut knot = TypedKnotData::new();
        knot.base.time = 1.0;
        knot.value = 10.0;
        data.push_knot(knot, None);

        data.apply_offset_and_scale(5.0, 2.0);

        // time = 1.0 * 2.0 + 5.0 = 7.0
        assert_eq!(data.base.times[0], 7.0);
        assert_eq!(data.knots[0].base.time, 7.0);
    }

    #[test]
    fn test_custom_value() {
        let v1 = CustomValue::Int(42);
        let v2 = CustomValue::String("test".into());
        let v3 = CustomValue::Vec(vec![v1.clone(), v2.clone()]);

        assert_eq!(v1, CustomValue::Int(42));
        assert_ne!(v1, v2);

        if let CustomValue::Vec(items) = v3 {
            assert_eq!(items.len(), 2);
        } else {
            panic!("Expected Vec");
        }
    }
}
