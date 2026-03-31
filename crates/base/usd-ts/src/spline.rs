//! TsSpline - Spline curve for animation.
//!
//! Port of pxr/base/ts/spline.h and spline.cpp
//!
//! A mathematical description of a curved function from time to value.
//! Splines are copy-on-write for efficiency.

use super::diff;
use super::knot::Knot;
use super::knot_data::KnotValueType;
use super::knot_map::KnotMap;
use super::regression_preventer::RegressionPreventerBatch;
use super::types::{
    AntiRegressionMode, CurveType, ExtrapMode, Extrapolation, InterpMode, LoopParams, TsTime,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use usd_gf::Interval;

// ============================================================================
// Constants
// ============================================================================

/// Default anti-regression authoring mode.
const DEFAULT_ANTI_REGRESSION_MODE: AntiRegressionMode = AntiRegressionMode::KeepRatio;

// ============================================================================
// Inner data for copy-on-write spline
// ============================================================================

/// Inner data for copy-on-write spline.
#[derive(Debug, Clone)]
struct SplineInnerData {
    /// Knots ordered by time.
    knots: BTreeMap<OrderedTime, Knot>,
    /// Value type for all knots.
    value_type: Option<KnotValueType>,
    /// Curve type (Bezier or Hermite).
    curve_type: CurveType,
    /// Pre-extrapolation mode.
    pre_extrapolation: Extrapolation,
    /// Post-extrapolation mode.
    post_extrapolation: Extrapolation,
    /// Inner loop parameters.
    inner_loop_params: LoopParams,
    /// Whether this spline holds time values (applies offset/scale).
    time_valued: bool,
    /// Whether value type is authoritative.
    is_typed: bool,
}

impl Default for SplineInnerData {
    fn default() -> Self {
        Self {
            knots: BTreeMap::new(),
            value_type: None,
            curve_type: CurveType::Bezier,
            pre_extrapolation: Extrapolation::held(),
            post_extrapolation: Extrapolation::held(),
            inner_loop_params: LoopParams::default(),
            time_valued: false,
            is_typed: false,
        }
    }
}

// ============================================================================
// OrderedTime wrapper for BTreeMap
// ============================================================================

/// Wrapper for f64 that implements Ord (handles NaN).
#[derive(Debug, Clone, Copy, PartialEq)]
struct OrderedTime(TsTime);

impl Eq for OrderedTime {}

impl PartialOrd for OrderedTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedTime {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

// ============================================================================
// Sample output structures
// ============================================================================

/// A single sample point for spline sampling.
#[derive(Debug, Clone, Copy, Default)]
pub struct SplineSample {
    /// Time value.
    pub time: TsTime,
    /// Value at this time.
    pub value: f64,
}

/// Source region for a spline sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplineSampleSource {
    /// Normal interpolation between knots.
    Interpolation,
    /// Pre-extrapolation region.
    PreExtrapolation,
    /// Post-extrapolation region.
    PostExtrapolation,
    /// Inner loop region.
    InnerLoop,
    /// Pre-loop region.
    PreLoop,
    /// Post-loop region.
    PostLoop,
    /// Value block.
    ValueBlock,
}

/// A polyline segment from sampling.
#[derive(Debug, Clone, Default)]
pub struct SplinePolyline {
    /// Sample points forming the polyline.
    pub samples: Vec<SplineSample>,
}

/// Collection of sampled polylines.
#[derive(Debug, Clone, Default)]
pub struct SplineSamples {
    /// Polylines making up the sampled spline.
    pub polylines: Vec<SplinePolyline>,
}

/// Collection of sampled polylines with source info.
#[derive(Debug, Clone, Default)]
pub struct SplineSamplesWithSources {
    /// Polylines making up the sampled spline.
    pub polylines: Vec<SplinePolyline>,
    /// Source for each polyline.
    pub sources: Vec<SplineSampleSource>,
}

// ============================================================================
// SplineData - external wrapper matching spline_data.rs
// ============================================================================

/// Provides access to internal spline data for external helpers.
pub struct SplineData<'a> {
    spline: &'a Spline,
}

impl<'a> SplineData<'a> {
    /// Returns the times vector.
    pub fn times(&self) -> Vec<TsTime> {
        self.spline.data.knots.keys().map(|k| k.0).collect()
    }

    /// Returns true if spline has inner loops.
    pub fn has_inner_loops(&self) -> bool {
        self.spline.has_inner_loops()
    }

    /// Returns the pre-extrapolation.
    pub fn pre_extrapolation(&self) -> &Extrapolation {
        &self.spline.data.pre_extrapolation
    }

    /// Returns the post-extrapolation.
    pub fn post_extrapolation(&self) -> &Extrapolation {
        &self.spline.data.post_extrapolation
    }

    /// Returns first knot time.
    pub fn pre_extrap_time(&self) -> TsTime {
        self.spline.first_time().unwrap_or(0.0)
    }

    /// Returns last knot time.
    pub fn post_extrap_time(&self) -> TsTime {
        self.spline.last_time().unwrap_or(0.0)
    }
}

// ============================================================================
// Spline - main class
// ============================================================================

/// A mathematical description of a curved function from time to value.
///
/// Splines are defined by knots. The curve passes through each knot,
/// and in between, the shape is controlled by tangents at the knots.
///
/// This class is copy-on-write for efficiency.
#[derive(Debug, Clone)]
pub struct Spline {
    data: Arc<SplineInnerData>,
}

impl Default for Spline {
    fn default() -> Self {
        Self::new()
    }
}

impl Spline {
    // =========================================================================
    // Construction
    // =========================================================================

    /// Creates a new empty spline.
    pub fn new() -> Self {
        Self {
            data: Arc::new(SplineInnerData::default()),
        }
    }

    /// Creates a spline with a specified value type.
    pub fn with_value_type(value_type: KnotValueType) -> Self {
        Self {
            data: Arc::new(SplineInnerData {
                value_type: Some(value_type),
                is_typed: true,
                ..Default::default()
            }),
        }
    }

    // =========================================================================
    // Value types
    // =========================================================================

    /// Returns whether the given value type is supported.
    pub fn is_supported_value_type(value_type: KnotValueType) -> bool {
        matches!(
            value_type,
            KnotValueType::Double | KnotValueType::Float | KnotValueType::Half
        )
    }

    /// Returns the value type, or None if not yet established.
    pub fn value_type(&self) -> Option<KnotValueType> {
        self.data.value_type
    }

    /// Returns true if the spline is holding the specified type.
    pub fn is_holding<T: 'static>(&self) -> bool {
        match self.data.value_type {
            Some(KnotValueType::Double) => {
                std::any::TypeId::of::<T>() == std::any::TypeId::of::<f64>()
            }
            Some(KnotValueType::Float) => {
                std::any::TypeId::of::<T>() == std::any::TypeId::of::<f32>()
            }
            _ => false,
        }
    }

    // =========================================================================
    // Curve types
    // =========================================================================

    /// Returns the curve type.
    pub fn curve_type(&self) -> CurveType {
        self.data.curve_type
    }

    /// Sets the curve type.
    pub fn set_curve_type(&mut self, curve_type: CurveType) {
        if self.data.curve_type != curve_type {
            self.make_unique().curve_type = curve_type;
            // If switching to Hermite, update tangent widths
            if curve_type == CurveType::Hermite {
                self.update_all_tangents();
            }
        }
    }

    // =========================================================================
    // Basic queries
    // =========================================================================

    /// Returns true if the spline has no knots.
    pub fn is_empty(&self) -> bool {
        self.data.knots.is_empty()
    }

    /// Returns the number of knots.
    pub fn knot_count(&self) -> usize {
        self.data.knots.len()
    }

    // =========================================================================
    // Extrapolation
    // =========================================================================

    /// Returns the pre-extrapolation mode.
    pub fn pre_extrapolation(&self) -> &Extrapolation {
        &self.data.pre_extrapolation
    }

    /// Returns the post-extrapolation mode.
    pub fn post_extrapolation(&self) -> &Extrapolation {
        &self.data.post_extrapolation
    }

    /// Sets the pre-extrapolation mode.
    pub fn set_pre_extrapolation(&mut self, extrap: Extrapolation) {
        self.make_unique().pre_extrapolation = extrap;
    }

    /// Sets the post-extrapolation mode.
    pub fn set_post_extrapolation(&mut self, extrap: Extrapolation) {
        self.make_unique().post_extrapolation = extrap;
    }

    // =========================================================================
    // Inner loops
    // =========================================================================

    /// Returns the inner loop parameters.
    pub fn inner_loop_params(&self) -> &LoopParams {
        &self.data.inner_loop_params
    }

    /// Sets the inner loop parameters.
    pub fn set_inner_loop_params(&mut self, mut params: LoopParams) {
        // Ignore negative loop counts
        if params.num_pre_loops < 0 {
            params.num_pre_loops = 0;
        }
        if params.num_post_loops < 0 {
            params.num_post_loops = 0;
        }
        self.make_unique().inner_loop_params = params;
    }

    // =========================================================================
    // Time range
    // =========================================================================

    /// Returns the time range spanned by the knots.
    pub fn time_range(&self) -> Option<Interval> {
        if self.data.knots.is_empty() {
            return None;
        }
        let first = self.data.knots.first_key_value().map(|(k, _)| k.0)?;
        let last = self.data.knots.last_key_value().map(|(k, _)| k.0)?;
        Some(Interval::new(first, last, true, true))
    }

    /// Returns the first knot time, if any.
    pub fn first_time(&self) -> Option<TsTime> {
        self.data.knots.first_key_value().map(|(k, _)| k.0)
    }

    /// Returns the last knot time, if any.
    pub fn last_time(&self) -> Option<TsTime> {
        self.data.knots.last_key_value().map(|(k, _)| k.0)
    }

    // =========================================================================
    // Knot access
    // =========================================================================

    /// Returns true if a knot exists at the given time.
    pub fn has_knot_at(&self, time: TsTime) -> bool {
        self.data.knots.contains_key(&OrderedTime(time))
    }

    /// Returns the knot at the given time, if any.
    pub fn get_knot(&self, time: TsTime) -> Option<&Knot> {
        self.data.knots.get(&OrderedTime(time))
    }

    /// Returns an iterator over all knots.
    pub fn knots(&self) -> impl Iterator<Item = &Knot> {
        self.data.knots.values()
    }

    /// Returns an iterator over (time, knot) pairs.
    pub fn knots_with_times(&self) -> impl Iterator<Item = (TsTime, &Knot)> {
        self.data.knots.iter().map(|(t, k)| (t.0, k))
    }

    /// Returns knots as a vector.
    pub fn knots_vec(&self) -> Vec<Knot> {
        self.data.knots.values().cloned().collect()
    }

    /// Returns all knots as a `KnotMap`.
    ///
    /// Matches C++ `TsSpline::GetKnots()`.
    #[must_use]
    pub fn knots_map(&self) -> KnotMap {
        KnotMap::from_knots(self.knots_vec())
    }

    /// Returns knots that affect the specified time interval.
    ///
    /// Returns a `KnotMap` containing the knots that affect the curve within
    /// the time interval. This may include knots outside the interval if they
    /// affect the curve inside it. For example, knots at times 10, 20, and 30
    /// with an interval [15..25] may return all 3 knots since the knots at 10
    /// and 30 affect the shape inside the interval.
    ///
    /// Matches C++ `TsSpline::GetKnots(const GfInterval& timeInterval)`.
    pub fn knots_in_interval(&self, interval: &Interval) -> KnotMap {
        if self.data.knots.is_empty() {
            return KnotMap::new();
        }

        let min_t = interval.get_min();
        let max_t = interval.get_max();

        // Find start: greatest knot time <= min (knot that starts segment containing min)
        let start_key = self
            .data
            .knots
            .range(..=OrderedTime(min_t))
            .next_back()
            .map(|(k, _)| *k);

        let start_key = match start_key {
            Some(k) => k,
            // min is before first knot; start at first knot
            None => *self.data.knots.first_key_value().unwrap().0,
        };

        // Find end: smallest knot time >= max (knot that ends segment containing max)
        let end_key = self
            .data
            .knots
            .range(OrderedTime(max_t)..)
            .next()
            .map(|(k, _)| *k);

        let end_key = match end_key {
            Some(k) => k,
            // max is after last knot; end at last knot
            None => *self.data.knots.last_key_value().unwrap().0,
        };

        let knots: Vec<Knot> = self
            .data
            .knots
            .range(start_key..=end_key)
            .map(|(_, k)| k.clone())
            .collect();

        KnotMap::from_knots(knots)
    }

    /// Gets knot data at index.
    pub fn get_knot_at_index(&self, index: usize) -> Option<&Knot> {
        self.data.knots.values().nth(index)
    }

    // =========================================================================
    // Knot modification
    // =========================================================================

    /// Checks if a knot can be set.
    pub fn can_set_knot(&self, knot: &Knot) -> Result<(), String> {
        if self.data.is_typed {
            if let Some(value_type) = self.data.value_type {
                if knot.value_type() != value_type {
                    return Err(format!(
                        "Cannot set knot of value type '{:?}' into spline of value type '{:?}'",
                        knot.value_type(),
                        value_type
                    ));
                }
            }
        }
        Ok(())
    }

    /// Sets a knot at its time.
    ///
    /// If a knot already exists at that time, it is replaced.
    pub fn set_knot(&mut self, knot: Knot) {
        let time = OrderedTime(knot.time());

        // Establish value type from first knot
        let data = self.make_unique();
        if data.value_type.is_none() {
            data.value_type = Some(knot.value_type());
            data.is_typed = true;
        }

        let idx = data.knots.keys().take_while(|k| **k < time).count();
        data.knots.insert(time, knot);

        // Update tangents and de-regress
        self.update_knot_tangents(idx);
    }

    /// Sets a knot without updating tangents (internal use).
    /// Used by RegressionPreventer per C++ reference.
    #[allow(dead_code)] // Note: For regression_preventer.rs.
    fn set_knot_unchecked(&mut self, knot: Knot) {
        let time = OrderedTime(knot.time());
        let data = self.make_unique();
        if data.value_type.is_none() {
            data.value_type = Some(knot.value_type());
            data.is_typed = true;
        }
        data.knots.insert(time, knot);
    }

    /// Removes the knot at the given time.
    ///
    /// Returns true if a knot was removed.
    pub fn remove_knot(&mut self, time: TsTime) -> bool {
        self.make_unique()
            .knots
            .remove(&OrderedTime(time))
            .is_some()
    }

    /// Clears all knots.
    pub fn clear(&mut self) {
        self.make_unique().knots.clear();
    }

    // =========================================================================
    // Evaluation
    // =========================================================================

    /// Evaluates the spline at the given time.
    pub fn eval(&self, time: TsTime) -> Option<f64> {
        if self.data.knots.is_empty() {
            return None;
        }

        let time_key = OrderedTime(time);

        // Exact knot hit
        if let Some(knot) = self.data.knots.get(&time_key) {
            return Some(knot.value());
        }

        // Find surrounding knots
        let mut before: Option<&Knot> = None;
        let mut after: Option<&Knot> = None;

        for (t, knot) in &self.data.knots {
            if t.0 < time {
                before = Some(knot);
            } else {
                after = Some(knot);
                break;
            }
        }

        match (before, after) {
            (Some(k0), Some(k1)) => Some(self.interpolate(k0, k1, time)),
            (Some(k), None) => Some(self.extrapolate_post(k, time)),
            (None, Some(k)) => Some(self.extrapolate_pre(k, time)),
            (None, None) => None,
        }
    }

    /// Evaluates the derivative at the given time.
    pub fn eval_derivative(&self, time: TsTime) -> Option<f64> {
        if self.data.knots.len() < 2 {
            return Some(0.0);
        }

        // Finite difference approximation
        let epsilon = 1e-6;
        let v0 = self.eval(time - epsilon)?;
        let v1 = self.eval(time + epsilon)?;
        Some((v1 - v0) / (2.0 * epsilon))
    }

    /// Evaluates the pre-value (left-side value) at the given time.
    pub fn eval_pre_value(&self, time: TsTime) -> Option<f64> {
        if self.data.knots.is_empty() {
            return None;
        }

        // Exact knot hit - return pre_value
        if let Some(knot) = self.data.knots.get(&OrderedTime(time)) {
            return Some(knot.pre_value());
        }

        // Otherwise same as eval
        self.eval(time)
    }

    /// Evaluates the pre-derivative at the given time.
    pub fn eval_pre_derivative(&self, time: TsTime) -> Option<f64> {
        if self.data.knots.len() < 2 {
            return Some(0.0);
        }

        let epsilon = 1e-6;
        let v0 = self.eval_pre_value(time - epsilon)?;
        let v1 = self.eval_pre_value(time)?;
        Some((v1 - v0) / epsilon)
    }

    /// Evaluates as held (step function) at the given time.
    pub fn eval_held(&self, time: TsTime) -> Option<f64> {
        if self.data.knots.is_empty() {
            return None;
        }

        // Find the knot at or before this time
        for (t, knot) in self.data.knots.iter().rev() {
            if t.0 <= time {
                return Some(knot.value());
            }
        }

        // Before first knot
        self.data.knots.first_key_value().map(|(_, k)| k.value())
    }

    /// Returns true if left and right values differ at the given time.
    pub fn do_sides_differ(&self, time: TsTime) -> bool {
        if let Some(knot) = self.data.knots.get(&OrderedTime(time)) {
            knot.is_dual_valued()
        } else {
            false
        }
    }

    // =========================================================================
    // Sampling
    // =========================================================================

    /// Samples the spline over the given interval.
    ///
    /// Delegates to sample.rs Sampler which handles extrapolation loops,
    /// inner loops, regression prevention, and adaptive Bezier subdivision.
    pub fn sample(
        &self,
        time_interval: &Interval,
        time_scale: f64,
        value_scale: f64,
        tolerance: f64,
    ) -> Option<SplineSamples> {
        if time_interval.is_empty() || time_scale <= 0.0 || value_scale <= 0.0 || tolerance <= 0.0 {
            return None;
        }

        if self.data.knots.is_empty() {
            return Some(SplineSamples::default());
        }

        // Use sample.rs Sampler for proper loop/extrap/regression handling.
        use super::eval::SampleVertex;
        use usd_gf::Vec2d;

        let data = super::sample::spline_to_typed_data(self);
        let sampler =
            super::sample::Sampler::new(&data, *time_interval, time_scale, value_scale, tolerance);
        let mut eval_samples: super::eval::SplineSamples<Vec2d> = super::eval::SplineSamples::new();
        sampler.sample(&mut eval_samples);

        // Convert eval::SplineSamples<Vec2d> -> spline::SplineSamples
        let mut result = SplineSamples::default();
        for polyline in &eval_samples.polylines {
            let mut sp = SplinePolyline::default();
            for v in polyline {
                sp.samples.push(SplineSample {
                    time: v.time(),
                    value: v.value(),
                });
            }
            if !sp.samples.is_empty() {
                result.polylines.push(sp);
            }
        }

        Some(result)
    }

    /// Samples the spline with source information.
    ///
    /// Delegates to sample.rs Sampler for proper source tracking.
    pub fn sample_with_sources(
        &self,
        time_interval: &Interval,
        time_scale: f64,
        value_scale: f64,
        tolerance: f64,
    ) -> Option<SplineSamplesWithSources> {
        if time_interval.is_empty() || time_scale <= 0.0 || value_scale <= 0.0 || tolerance <= 0.0 {
            return None;
        }

        if self.data.knots.is_empty() {
            return Some(SplineSamplesWithSources::default());
        }

        use super::eval::SampleVertex;
        use usd_gf::Vec2d;

        let data = super::sample::spline_to_typed_data(self);
        let sampler =
            super::sample::Sampler::new(&data, *time_interval, time_scale, value_scale, tolerance);
        let mut eval_samples: super::eval::SplineSamplesWithSources<Vec2d> =
            super::eval::SplineSamplesWithSources::new();
        sampler.sample(&mut eval_samples);

        // Convert eval types -> spline types
        let mut result = SplineSamplesWithSources::default();
        for polyline in &eval_samples.polylines {
            let mut sp = SplinePolyline::default();
            for v in polyline {
                sp.samples.push(SplineSample {
                    time: v.time(),
                    value: v.value(),
                });
            }
            if !sp.samples.is_empty() {
                result.polylines.push(sp);
            }
        }
        // Map eval source types to spline source types
        for src in &eval_samples.sources {
            result.sources.push(match src {
                super::types::SplineSampleSource::PreExtrap => SplineSampleSource::PreExtrapolation,
                super::types::SplineSampleSource::PostExtrap => {
                    SplineSampleSource::PostExtrapolation
                }
                super::types::SplineSampleSource::PreExtrapLoop => SplineSampleSource::PreLoop,
                super::types::SplineSampleSource::PostExtrapLoop => SplineSampleSource::PostLoop,
                super::types::SplineSampleSource::InnerLoopPreEcho
                | super::types::SplineSampleSource::InnerLoopProto
                | super::types::SplineSampleSource::InnerLoopPostEcho => {
                    SplineSampleSource::InnerLoop
                }
                _ => SplineSampleSource::Interpolation,
            });
        }

        Some(result)
    }

    // =========================================================================
    // Spline comparison
    // =========================================================================

    /// Compares this spline with another.
    ///
    /// Returns the time interval where they differ.
    pub fn diff(&self, other: &Spline) -> Interval {
        diff::diff(self, other, &Interval::full())
    }

    /// Compares this spline with another over a specific interval.
    pub fn diff_in_interval(&self, other: &Spline, compare_interval: &Interval) -> Interval {
        diff::diff(self, other, compare_interval)
    }

    // =========================================================================
    // Whole-spline queries
    // =========================================================================

    /// Returns true if spline is time-valued.
    pub fn is_time_valued(&self) -> bool {
        self.data.time_valued
    }

    /// Sets whether spline is time-valued.
    pub fn set_time_valued(&mut self, time_valued: bool) {
        self.make_unique().time_valued = time_valued;
    }

    /// Returns true if any segment is a value block.
    pub fn has_value_blocks(&self) -> bool {
        if self.data.pre_extrapolation.mode == ExtrapMode::ValueBlock
            || self.data.post_extrapolation.mode == ExtrapMode::ValueBlock
        {
            return true;
        }
        self.data
            .knots
            .values()
            .any(|k| k.interp_mode() == InterpMode::ValueBlock)
    }

    /// Returns true if there's a value block at the given time.
    pub fn has_value_block_at(&self, time: TsTime) -> bool {
        if self.data.knots.is_empty() {
            return false;
        }

        // Check extrapolation regions
        if let Some(first) = self.first_time() {
            if time < first && self.data.pre_extrapolation.mode == ExtrapMode::ValueBlock {
                return true;
            }
        }

        if let Some(last) = self.last_time() {
            if time > last && self.data.post_extrapolation.mode == ExtrapMode::ValueBlock {
                return true;
            }
        }

        // Find the knot at or before this time
        for (t, knot) in self.data.knots.iter().rev() {
            if t.0 <= time {
                return knot.interp_mode() == InterpMode::ValueBlock;
            }
        }

        false
    }

    /// Returns true if the spline value varies over time.
    pub fn is_varying(&self) -> bool {
        if self.data.knots.len() < 2 {
            return false;
        }

        let first_val = self.data.knots.values().next().map(|k| k.value());
        self.data
            .knots
            .values()
            .any(|k| Some(k.value()) != first_val)
    }

    /// Returns true if the spline has any looping (inner or extrapolation).
    pub fn has_loops(&self) -> bool {
        self.has_inner_loops() || self.has_extrapolating_loops()
    }

    /// Returns true if inner loops are enabled.
    pub fn has_inner_loops(&self) -> bool {
        let params = &self.data.inner_loop_params;
        params.is_enabled() && self.has_knot_at(params.proto_start)
    }

    /// Returns true if extrapolation uses looping.
    pub fn has_extrapolating_loops(&self) -> bool {
        self.data.pre_extrapolation.is_looping() || self.data.post_extrapolation.is_looping()
    }

    /// Returns true if all segments use linear interpolation.
    pub fn is_linear(&self) -> bool {
        if self.data.knots.is_empty() {
            return true;
        }
        self.data
            .knots
            .values()
            .all(|k| matches!(k.interp_mode(), InterpMode::Linear | InterpMode::Held))
    }

    /// Returns true if the spline is C0 continuous (no value jumps).
    pub fn is_c0_continuous(&self) -> bool {
        !self.data.knots.values().any(|k| k.is_dual_valued())
    }

    /// Returns true if the spline is G1 continuous (tangent directions match).
    pub fn is_g1_continuous(&self) -> bool {
        self.data.knots.values().all(|k| k.is_g1_continuous())
    }

    /// Returns true if the spline is C1 continuous (tangent slopes match).
    pub fn is_c1_continuous(&self) -> bool {
        self.data.knots.values().all(|k| k.is_c1_continuous())
    }

    /// Returns true if the segment starting at the given time is flat.
    pub fn is_segment_flat(&self, start_time: TsTime) -> bool {
        let knots: Vec<_> = self.data.knots.values().collect();
        let times: Vec<_> = self.data.knots.keys().collect();

        for i in 0..knots.len().saturating_sub(1) {
            if (times[i].0 - start_time).abs() < 1e-10 {
                let v0 = knots[i].value();
                let v1 = knots[i + 1].value();
                return (v0 - v1).abs() < 1e-10;
            }
        }
        false
    }

    /// Returns true if the segment starting at the given time is monotonic.
    pub fn is_segment_monotonic(&self, start_time: TsTime) -> bool {
        let knots: Vec<_> = self.data.knots.values().collect();
        let times: Vec<_> = self.data.knots.keys().collect();

        for i in 0..knots.len().saturating_sub(1) {
            if (times[i].0 - start_time).abs() < 1e-10 {
                let t0 = times[i].0;
                let t1 = times[i + 1].0;
                let v0 = knots[i].value();
                let v1 = knots[i + 1].value();

                // Sample the segment and check monotonicity
                let num_samples = 10;
                let dt = (t1 - t0) / num_samples as f64;

                let mut prev = v0;
                let increasing = v1 > v0;

                for j in 1..=num_samples {
                    let t = t0 + j as f64 * dt;
                    let v = self.eval(t).unwrap_or(v1);

                    if increasing {
                        if v < prev - 1e-10 {
                            return false;
                        }
                    } else if v > prev + 1e-10 {
                        return false;
                    }
                    prev = v;
                }

                return true;
            }
        }
        false
    }

    /// Returns true if the knot at the given time is redundant.
    pub fn is_knot_redundant(&self, time: TsTime, default_value: Option<f64>) -> bool {
        let knots: Vec<_> = self.data.knots.values().collect();
        let times: Vec<_> = self.data.knots.keys().collect();

        if knots.len() < 2 {
            // Single knot is redundant only if it matches default
            if let Some(default) = default_value {
                if let Some(knot) = knots.first() {
                    return (knot.value() - default).abs() < 1e-10;
                }
            }
            return false;
        }

        // Find the knot at this time
        for (i, t) in times.iter().enumerate() {
            if (t.0 - time).abs() < 1e-10 {
                // Check if removing this knot would change the curve
                let knot = knots[i];

                // First and last knots are never redundant
                if i == 0 || i == knots.len() - 1 {
                    return false;
                }

                // If interpolation is held, knot is redundant if values match
                let prev = knots[i - 1];
                if prev.interp_mode() == InterpMode::Held {
                    return (prev.value() - knot.value()).abs() < 1e-10;
                }

                // For linear, check if point is on line
                if prev.interp_mode() == InterpMode::Linear {
                    let next = knots[i + 1];
                    let t0 = times[i - 1].0;
                    let t1 = times[i].0;
                    let t2 = times[i + 1].0;

                    let expected =
                        prev.value() + (next.value() - prev.value()) * (t1 - t0) / (t2 - t0);
                    return (expected - knot.value()).abs() < 1e-10;
                }

                return false;
            }
        }

        false
    }

    /// Computes the range of values in the spline.
    pub fn value_range(&self) -> Option<(f64, f64)> {
        if self.data.knots.is_empty() {
            return None;
        }

        let mut min_val = f64::MAX;
        let mut max_val = f64::MIN;

        for knot in self.data.knots.values() {
            let v = knot.value();
            min_val = min_val.min(v);
            max_val = max_val.max(v);

            if knot.is_dual_valued() {
                let pv = knot.pre_value();
                min_val = min_val.min(pv);
                max_val = max_val.max(pv);
            }
        }

        Some((min_val, max_val))
    }

    /// Computes the value range over a specific time span.
    pub fn value_range_in_interval(&self, time_span: &Interval) -> Option<(f64, f64)> {
        if self.data.knots.is_empty() || time_span.is_empty() {
            return None;
        }

        let mut min_val = f64::MAX;
        let mut max_val = f64::MIN;

        // Sample the spline within the interval
        let num_samples = 100;
        let min_t = time_span.get_min();
        let max_t = time_span.get_max();
        let dt = (max_t - min_t) / num_samples as f64;

        for i in 0..=num_samples {
            let t = min_t + i as f64 * dt;
            if let Some(v) = self.eval(t) {
                min_val = min_val.min(v);
                max_val = max_val.max(v);
            }
        }

        if min_val <= max_val {
            Some((min_val, max_val))
        } else {
            None
        }
    }

    // =========================================================================
    // Anti-regression
    // =========================================================================

    /// Returns the current anti-regression authoring mode.
    ///
    /// Consults the RAII stack (AntiRegressionAuthoringSelector / EditBehaviorBlock).
    /// Falls back to DEFAULT_ANTI_REGRESSION_MODE if no selector is active.
    pub fn anti_regression_authoring_mode() -> AntiRegressionMode {
        use super::raii;
        if let Some(mode) = raii::get_anti_regression_mode() {
            mode
        } else {
            DEFAULT_ANTI_REGRESSION_MODE
        }
    }

    /// Returns true if any segment has regressive tangents.
    pub fn has_regressive_tangents(&self) -> bool {
        if self.data.curve_type != CurveType::Bezier {
            return false;
        }

        let knots: Vec<_> = self.data.knots.values().collect();
        if knots.len() < 2 {
            return false;
        }

        let mode = Self::anti_regression_authoring_mode();

        for i in 0..knots.len() - 1 {
            let k0 = knots[i];
            let k1 = knots[i + 1];
            let segment_width = k1.time() - k0.time();

            if segment_width > 0.0
                && RegressionPreventerBatch::is_segment_regressive(
                    k0.post_tan_width(),
                    k1.pre_tan_width(),
                    segment_width,
                    mode,
                )
            {
                return true;
            }
        }

        false
    }

    /// Adjusts any regressive tangents.
    ///
    /// Returns true if any changes were made.
    pub fn adjust_regressive_tangents(&mut self) -> bool {
        if self.data.curve_type != CurveType::Bezier {
            return false;
        }

        let knot_count = self.data.knots.len();
        if knot_count < 2 {
            return false;
        }

        let mode = Self::anti_regression_authoring_mode();
        let mut changed = false;

        // Collect knot times and data
        let times: Vec<TsTime> = self.data.knots.keys().map(|k| k.0).collect();

        for i in 0..knot_count - 1 {
            let t0 = times[i];
            let t1 = times[i + 1];
            let segment_width = t1 - t0;

            if segment_width <= 0.0 {
                continue;
            }

            // Get current widths
            let post_width = self
                .data
                .knots
                .get(&OrderedTime(t0))
                .map(|k| k.post_tan_width())
                .unwrap_or(0.0);
            let pre_width = self
                .data
                .knots
                .get(&OrderedTime(t1))
                .map(|k| k.pre_tan_width())
                .unwrap_or(0.0);

            if RegressionPreventerBatch::is_segment_regressive(
                post_width,
                pre_width,
                segment_width,
                mode,
            ) {
                // Adjust widths
                let (new_post, new_pre) = RegressionPreventerBatch::adjust_widths(
                    post_width,
                    pre_width,
                    segment_width,
                    mode,
                );

                // Apply adjustments
                let data = self.make_unique();
                if let Some(k0) = data.knots.get_mut(&OrderedTime(t0)) {
                    k0.set_post_tan_width(new_post);
                }
                if let Some(k1) = data.knots.get_mut(&OrderedTime(t1)) {
                    k1.set_pre_tan_width(new_pre);
                }

                changed = true;
            }
        }

        changed
    }

    // =========================================================================
    // Breakdown
    // =========================================================================

    /// Checks if a breakdown can be performed at the given time.
    pub fn can_breakdown(&self, time: TsTime) -> Result<(), String> {
        if self.has_knot_at(time) {
            return Err("A knot already exists at this time".to_string());
        }

        // Check if time is in a looped region
        if let Some(first) = self.first_time() {
            if let Some(last) = self.last_time() {
                if (time < first || time > last) && self.has_extrapolating_loops() {
                    return Err("Cannot breakdown in extrapolation loop region".to_string());
                }

                if self.has_inner_loops() {
                    let params = &self.data.inner_loop_params;
                    if time >= params.proto_start && time < params.proto_end {
                        // Time is in prototype region, which is OK
                    } else if time < first || time > last {
                        return Err("Cannot breakdown in looped region".to_string());
                    }
                }
            }
        }

        Ok(())
    }

    /// Performs a breakdown at the given time.
    ///
    /// Inserts a knot with minimal disruption to the curve shape.
    /// Returns true if successful.
    pub fn breakdown(&mut self, time: TsTime) -> bool {
        if self.can_breakdown(time).is_err() {
            return false;
        }

        if self.data.knots.is_empty() {
            return false;
        }

        // Find surrounding knots
        let mut before_time: Option<TsTime> = None;
        let mut after_time: Option<TsTime> = None;

        for t in self.data.knots.keys() {
            if t.0 < time {
                before_time = Some(t.0);
            } else {
                after_time = Some(t.0);
                break;
            }
        }

        // Handle extrapolation regions
        if before_time.is_none() {
            // Before first knot - create knot with extrapolated value
            let value = self.eval(time).unwrap_or(0.0);
            let mut knot = Knot::at_time(time, value);
            if let Some(first) = self.data.knots.values().next() {
                knot.set_interp_mode(first.interp_mode());
            }
            self.set_knot(knot);
            return true;
        }

        if after_time.is_none() {
            // After last knot - create knot with extrapolated value
            let value = self.eval(time).unwrap_or(0.0);
            let knot = Knot::at_time(time, value);
            self.set_knot(knot);
            return true;
        }

        // Between knots - create knot that preserves curve shape
        let t0 = before_time.expect("value expected");
        let t1 = after_time.expect("value expected");

        let k0 = self.data.knots.get(&OrderedTime(t0)).cloned();
        let k1 = self.data.knots.get(&OrderedTime(t1)).cloned();

        if k0.is_none() || k1.is_none() {
            return false;
        }

        let k0 = k0.expect("value expected");
        let k1 = k1.expect("value expected");

        // Evaluate at breakdown time
        let value = self.eval(time).unwrap_or(0.0);

        // Create the new knot
        let mut new_knot = Knot::at_time(time, value);
        new_knot.set_interp_mode(k0.interp_mode());

        // For curve interpolation, compute appropriate tangents
        if k0.interp_mode() == InterpMode::Curve {
            // Use De Casteljau subdivision to get tangent slopes
            let u = (time - t0) / (t1 - t0);

            let s0 = k0.post_tangent().slope;
            let s1 = k1.pre_tangent().slope;

            // Simple linear interpolation of slopes
            let new_slope = s0 * (1.0 - u) + s1 * u;

            new_knot.set_pre_tan_slope(new_slope);
            new_knot.set_post_tan_slope(new_slope);

            // Set tangent widths proportionally
            let _seg_width = t1 - t0;
            let pre_width = (time - t0) / 3.0;
            let post_width = (t1 - time) / 3.0;

            new_knot.set_pre_tan_width(pre_width);
            new_knot.set_post_tan_width(post_width);

            // Update original knots' tangent widths
            let data = self.make_unique();
            if let Some(k) = data.knots.get_mut(&OrderedTime(t0)) {
                k.set_post_tan_width(pre_width);
            }
            if let Some(k) = data.knots.get_mut(&OrderedTime(t1)) {
                k.set_pre_tan_width(post_width);
            }
        }

        self.set_knot(new_knot);
        true
    }

    // =========================================================================
    // Loop baking
    // =========================================================================

    /// Bakes inner loops into explicit knots.
    ///
    /// Returns true if successful.
    pub fn bake_inner_loops(&mut self) -> bool {
        if !self.has_inner_loops() {
            return true;
        }

        let baked = self.knots_with_inner_loops_baked();
        if baked.is_empty() {
            return false;
        }

        // Replace knots
        let data = self.make_unique();
        data.knots.clear();
        for knot in baked {
            data.knots.insert(OrderedTime(knot.time()), knot);
        }

        // Clear loop params
        data.inner_loop_params = LoopParams::default();

        true
    }

    /// Returns knots with inner loops baked.
    pub fn knots_with_inner_loops_baked(&self) -> Vec<Knot> {
        if !self.has_inner_loops() {
            return self.knots_vec();
        }

        let params = &self.data.inner_loop_params;
        let proto_start = params.proto_start;
        let proto_end = params.proto_end;
        let proto_span = proto_end - proto_start;

        if proto_span <= 0.0 {
            return self.knots_vec();
        }

        let mut result: Vec<Knot> = Vec::new();

        // Collect prototype knots
        let proto_knots: Vec<Knot> = self
            .data
            .knots
            .iter()
            .filter(|(t, _)| t.0 >= proto_start && t.0 <= proto_end)
            .map(|(_, k)| k.clone())
            .collect();

        // Generate pre-loops
        for i in (1..=params.num_pre_loops).rev() {
            let offset = -i as f64 * proto_span;
            let value_offset = -i as f64 * params.value_offset;

            for knot in &proto_knots {
                let mut new_knot = knot.clone();
                new_knot.set_time(knot.time() + offset);
                new_knot.set_value(knot.value() + value_offset);
                if knot.is_dual_valued() {
                    new_knot.set_pre_value(knot.pre_value() + value_offset);
                }
                result.push(new_knot);
            }
        }

        // Add prototype knots
        result.extend(proto_knots.clone());

        // Generate post-loops
        for i in 1..=params.num_post_loops {
            let offset = i as f64 * proto_span;
            let value_offset = i as f64 * params.value_offset;

            for knot in &proto_knots {
                let mut new_knot = knot.clone();
                new_knot.set_time(knot.time() + offset);
                new_knot.set_value(knot.value() + value_offset);
                if knot.is_dual_valued() {
                    new_knot.set_pre_value(knot.pre_value() + value_offset);
                }
                result.push(new_knot);
            }
        }

        // Sort by time
        result.sort_by(|a, b| a.time().partial_cmp(&b.time()).expect("value expected"));

        result
    }

    /// Returns knots with all loops (inner and extrapolation) baked.
    pub fn knots_with_loops_baked(&self, interval: &Interval) -> Vec<Knot> {
        if !self.has_loops() || self.data.knots.is_empty() {
            return self.knots_vec();
        }

        // Start with inner-loop-baked knots
        let mut result = self.knots_with_inner_loops_baked();

        // Handle extrapolation loops
        if self.has_extrapolating_loops() {
            if !interval.is_finite() {
                // Cannot bake infinite extrapolation loops
                return result;
            }

            let min_t = interval.get_min();
            let max_t = interval.get_max();

            // Pre-extrapolation loop
            if self.data.pre_extrapolation.is_looping() {
                if let Some(first) = self.first_time() {
                    if let Some(last) = self.last_time() {
                        let span = last - first;
                        if span > 0.0 {
                            let mut t = first - span;
                            while t >= min_t {
                                for knot in self.data.knots.values() {
                                    let offset = t - first;
                                    let mut new_knot = knot.clone();
                                    new_knot.set_time(knot.time() + offset);
                                    result.push(new_knot);
                                }
                                t -= span;
                            }
                        }
                    }
                }
            }

            // Post-extrapolation loop
            if self.data.post_extrapolation.is_looping() {
                if let Some(first) = self.first_time() {
                    if let Some(last) = self.last_time() {
                        let span = last - first;
                        if span > 0.0 {
                            let mut t = last;
                            while t <= max_t {
                                for knot in self.data.knots.values() {
                                    let offset = t - first;
                                    let mut new_knot = knot.clone();
                                    new_knot.set_time(knot.time() + offset);
                                    result.push(new_knot);
                                }
                                t += span;
                            }
                        }
                    }
                }
            }

            // Sort by time
            result.sort_by(|a, b| a.time().partial_cmp(&b.time()).expect("value expected"));
        }

        result
    }

    // =========================================================================
    // Offset and scale (for layer offsets)
    // =========================================================================

    /// Applies time offset and scale to the spline.
    ///
    /// Used for layer offset application.
    pub fn apply_offset_and_scale(&mut self, offset: TsTime, scale: f64) {
        if scale <= 0.0 {
            return;
        }

        let data = self.make_unique();

        // Scale extrapolation slopes
        if data.pre_extrapolation.mode == ExtrapMode::Sloped {
            data.pre_extrapolation.slope /= scale;
        }
        if data.post_extrapolation.mode == ExtrapMode::Sloped {
            data.post_extrapolation.slope /= scale;
        }

        // Process loop params
        if data.inner_loop_params.proto_end > data.inner_loop_params.proto_start {
            data.inner_loop_params.proto_start =
                data.inner_loop_params.proto_start * scale + offset;
            data.inner_loop_params.proto_end = data.inner_loop_params.proto_end * scale + offset;
        }

        // Transform knots
        let times: Vec<TsTime> = data.knots.keys().map(|k| k.0).collect();
        let knots: Vec<Knot> = data.knots.values().cloned().collect();
        data.knots.clear();

        for (old_time, mut knot) in times.into_iter().zip(knots) {
            let new_time = old_time * scale + offset;
            knot.set_time(new_time);

            // Scale tangent widths
            knot.set_pre_tan_width(knot.pre_tan_width() * scale);
            knot.set_post_tan_width(knot.post_tan_width() * scale);

            // Scale slopes inversely
            let (pre_slope, post_slope) = (knot.pre_tangent().slope, knot.post_tangent().slope);
            knot.set_pre_tan_slope(pre_slope / scale);
            knot.set_post_tan_slope(post_slope / scale);

            data.knots.insert(OrderedTime(new_time), knot);
        }
    }

    // =========================================================================
    // Tangent updates
    // =========================================================================

    /// Updates all tangents based on tangent algorithms.
    fn update_all_tangents(&mut self) -> bool {
        let knot_count = self.knot_count();
        let mut changed = false;

        for i in 0..knot_count {
            if self.update_knot_tangents_at_index(i) {
                changed = true;
            }
        }

        if self.adjust_regressive_tangents() {
            changed = true;
        }

        changed
    }

    /// Updates tangents for a single knot and its neighbors.
    fn update_knot_tangents(&mut self, idx: usize) -> bool {
        let knot_count = self.knot_count();
        let first = if idx > 0 { idx - 1 } else { idx };
        let last = if idx < knot_count - 1 { idx + 1 } else { idx };

        let mut changed = false;

        for i in first..=last {
            if self.update_knot_tangents_at_index(i) {
                changed = true;
            }
        }

        // Process segments for regression prevention
        if self.data.curve_type == CurveType::Bezier {
            for i in first..last {
                if self.process_segment_regression(i) {
                    changed = true;
                }
            }
        }

        changed
    }

    /// Updates tangents at a specific index using auto-tangent algorithms.
    ///
    /// For knots with `auto_tangents` enabled and an automatic tangent
    /// algorithm, computes the slope from neighboring knots:
    /// - Interior: slope = (y[i+1] - y[i-1]) / (t[i+1] - t[i-1])
    /// - First:    slope = (y[1] - y[0]) / (t[1] - t[0])
    /// - Last:     slope = (y[n-1] - y[n-2]) / (t[n-1] - t[n-2])
    fn update_knot_tangents_at_index(&mut self, idx: usize) -> bool {
        let knot_count = self.data.knots.len();
        if knot_count == 0 || idx >= knot_count {
            return false;
        }

        // Gather time/value pairs and knot properties at idx
        let times: Vec<TsTime> = self.data.knots.keys().map(|k| k.0).collect();
        let values: Vec<f64> = self.data.knots.values().map(|k| k.value()).collect();

        let knot = self.data.knots.values().nth(idx).unwrap();
        if !knot.auto_tangents() {
            return false;
        }

        let pre_algo = knot.pre_tan_algorithm();
        let post_algo = knot.post_tan_algorithm();

        // Only process if at least one side has an automatic algorithm
        if !pre_algo.is_automatic() && !post_algo.is_automatic() {
            return false;
        }

        // Compute slope based on neighbors
        let slope = if knot_count == 1 {
            // Single knot: flat tangent
            0.0
        } else if idx == 0 {
            // First knot: one-sided slope forward
            let dt = times[1] - times[0];
            if dt.abs() < f64::EPSILON {
                0.0
            } else {
                (values[1] - values[0]) / dt
            }
        } else if idx == knot_count - 1 {
            // Last knot: one-sided slope backward
            let dt = times[idx] - times[idx - 1];
            if dt.abs() < f64::EPSILON {
                0.0
            } else {
                (values[idx] - values[idx - 1]) / dt
            }
        } else {
            // Interior knot: central difference
            let dt = times[idx + 1] - times[idx - 1];
            if dt.abs() < f64::EPSILON {
                0.0
            } else {
                (values[idx + 1] - values[idx - 1]) / dt
            }
        };

        // Apply computed slope to the knot tangents
        let time_key = OrderedTime(times[idx]);
        let data = self.make_unique();
        if let Some(knot) = data.knots.get_mut(&time_key) {
            let mut changed = false;

            if pre_algo.is_automatic() {
                let old_slope = knot.pre_tan_slope();
                if (old_slope - slope).abs() > f64::EPSILON {
                    knot.pre_tangent_mut().slope = slope;
                    changed = true;
                }
            }

            if post_algo.is_automatic() {
                let old_slope = knot.post_tan_slope();
                if (old_slope - slope).abs() > f64::EPSILON {
                    knot.post_tangent_mut().slope = slope;
                    changed = true;
                }
            }

            changed
        } else {
            false
        }
    }

    /// Processes a segment for regression.
    fn process_segment_regression(&mut self, idx: usize) -> bool {
        let times: Vec<TsTime> = self.data.knots.keys().map(|k| k.0).collect();

        if idx + 1 >= times.len() {
            return false;
        }

        let t0 = times[idx];
        let t1 = times[idx + 1];
        let segment_width = t1 - t0;

        if segment_width <= 0.0 {
            return false;
        }

        let mode = Self::anti_regression_authoring_mode();

        let post_width = self
            .data
            .knots
            .get(&OrderedTime(t0))
            .map(|k| k.post_tan_width())
            .unwrap_or(0.0);
        let pre_width = self
            .data
            .knots
            .get(&OrderedTime(t1))
            .map(|k| k.pre_tan_width())
            .unwrap_or(0.0);

        if RegressionPreventerBatch::is_segment_regressive(
            post_width,
            pre_width,
            segment_width,
            mode,
        ) {
            let (new_post, new_pre) =
                RegressionPreventerBatch::adjust_widths(post_width, pre_width, segment_width, mode);

            let data = self.make_unique();
            if let Some(k0) = data.knots.get_mut(&OrderedTime(t0)) {
                k0.set_post_tan_width(new_post);
            }
            if let Some(k1) = data.knots.get_mut(&OrderedTime(t1)) {
                k1.set_pre_tan_width(new_pre);
            }

            return true;
        }

        false
    }

    // =========================================================================
    // Internal helpers
    // =========================================================================

    fn interpolate(&self, k0: &Knot, k1: &Knot, time: TsTime) -> f64 {
        let t0 = k0.time();
        let t1 = k1.time();
        let v0 = k0.value();
        let v1 = k1.value();

        if (t1 - t0).abs() < 1e-10 {
            return v0;
        }

        let t = (time - t0) / (t1 - t0);

        match k0.interp_mode() {
            InterpMode::ValueBlock => v0,
            InterpMode::Held => v0,
            InterpMode::Linear => v0 + t * (v1 - v0),
            InterpMode::Curve => {
                // Cubic Bezier interpolation
                let s0 = k0.post_tangent().slope;
                let s1 = k1.pre_tangent().slope;
                let dt = t1 - t0;

                // Control points for cubic bezier
                let p0 = v0;
                let p1 = v0 + s0 * dt / 3.0;
                let p2 = v1 - s1 * dt / 3.0;
                let p3 = v1;

                // De Casteljau
                let u = 1.0 - t;
                u * u * u * p0 + 3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t * p3
            }
        }
    }

    fn extrapolate_pre(&self, first: &Knot, time: TsTime) -> f64 {
        let extrap = &self.data.pre_extrapolation;
        match extrap.mode {
            ExtrapMode::Held => first.value(),
            ExtrapMode::Linear => {
                let slope = first.pre_tangent().slope;
                first.value() + slope * (time - first.time())
            }
            ExtrapMode::Sloped => first.value() + extrap.slope * (time - first.time()),
            ExtrapMode::LoopRepeat | ExtrapMode::LoopReset | ExtrapMode::LoopOscillate => {
                // Handle loop extrapolation
                if let (Some(first_t), Some(last_t)) = (self.first_time(), self.last_time()) {
                    let span = last_t - first_t;
                    if span > 0.0 {
                        let offset = ((first_t - time) / span).ceil() * span;
                        return self.eval(time + offset).unwrap_or(first.value());
                    }
                }
                first.value()
            }
            _ => first.value(),
        }
    }

    fn extrapolate_post(&self, last: &Knot, time: TsTime) -> f64 {
        let extrap = &self.data.post_extrapolation;
        match extrap.mode {
            ExtrapMode::Held => last.value(),
            ExtrapMode::Linear => {
                let slope = last.post_tangent().slope;
                last.value() + slope * (time - last.time())
            }
            ExtrapMode::Sloped => last.value() + extrap.slope * (time - last.time()),
            ExtrapMode::LoopRepeat | ExtrapMode::LoopReset | ExtrapMode::LoopOscillate => {
                // Handle loop extrapolation
                if let (Some(first_t), Some(last_t)) = (self.first_time(), self.last_time()) {
                    let span = last_t - first_t;
                    if span > 0.0 {
                        let offset = ((time - last_t) / span).ceil() * span;
                        return self.eval(time - offset).unwrap_or(last.value());
                    }
                }
                last.value()
            }
            _ => last.value(),
        }
    }

    /// Gets mutable access to data, cloning if necessary (COW).
    fn make_unique(&mut self) -> &mut SplineInnerData {
        Arc::make_mut(&mut self.data)
    }

    /// Provides access to internal data for external helpers.
    pub fn get_data(&self) -> SplineData<'_> {
        SplineData { spline: self }
    }
}

// ============================================================================
// PartialEq implementation
// ============================================================================

impl PartialEq for Spline {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
            || (self.data.knots.len() == other.data.knots.len()
                && self.data.value_type == other.data.value_type
                && self.data.curve_type == other.data.curve_type
                && self.data.pre_extrapolation == other.data.pre_extrapolation
                && self.data.post_extrapolation == other.data.post_extrapolation
                && self.data.inner_loop_params == other.data.inner_loop_params
                && self
                    .data
                    .knots
                    .iter()
                    .zip(other.data.knots.iter())
                    .all(|((t1, k1), (t2, k2))| t1 == t2 && k1 == k2))
    }
}

impl Eq for Spline {}

impl std::hash::Hash for Spline {
    /// Hashes by data pointer (matches C++ TfHashAppend).
    /// Identical but independent splines hash unequal.
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::ptr::hash(Arc::as_ptr(&self.data), state);
    }
}

// ============================================================================
// Display implementation
// ============================================================================

impl std::fmt::Display for Spline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Spline:")?;
        writeln!(f, "  value type {:?}", self.data.value_type)?;
        writeln!(f, "  time valued {}", self.data.time_valued)?;
        writeln!(f, "  curve type {:?}", self.data.curve_type)?;
        writeln!(f, "  pre extrap {:?}", self.data.pre_extrapolation.mode)?;
        writeln!(f, "  post extrap {:?}", self.data.post_extrapolation.mode)?;

        if self.has_inner_loops() {
            let lp = &self.data.inner_loop_params;
            writeln!(f, "Loop:")?;
            writeln!(
                f,
                "  start {}, end {}, numPreLoops {}, numPostLoops {}, valueOffset {}",
                lp.proto_start, lp.proto_end, lp.num_pre_loops, lp.num_post_loops, lp.value_offset
            )?;
        }

        for knot in self.data.knots.values() {
            writeln!(f, "{:?}", knot)?;
        }

        Ok(())
    }
}

// ============================================================================
// Swap function
// ============================================================================

/// Swaps two splines.
pub fn swap(lhs: &mut Spline, rhs: &mut Spline) {
    std::mem::swap(lhs, rhs);
}

// ============================================================================
// SplineOffsetAccess - for layer offsets
// ============================================================================

/// Provides access to apply offset and scale to splines.
pub struct SplineOffsetAccess;

impl SplineOffsetAccess {
    /// Applies time offset and scale to a spline.
    pub fn apply_offset_and_scale(spline: &mut Spline, offset: TsTime, scale: f64) {
        spline.apply_offset_and_scale(offset, scale);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::types::TangentAlgorithm;
    use super::*;

    #[test]
    fn test_spline_empty() {
        let spline = Spline::new();
        assert!(spline.is_empty());
        assert_eq!(spline.knot_count(), 0);
        assert!(spline.time_range().is_none());
        assert!(spline.eval(0.0).is_none());
    }

    #[test]
    fn test_spline_single_knot() {
        let mut spline = Spline::new();
        spline.set_knot(Knot::at_time(5.0, 10.0));

        assert!(!spline.is_empty());
        assert_eq!(spline.knot_count(), 1);
        assert!(spline.has_knot_at(5.0));
        assert_eq!(spline.eval(5.0), Some(10.0));
    }

    #[test]
    fn test_spline_linear_interp() {
        let mut spline = Spline::new();

        let mut k0 = Knot::at_time(0.0, 0.0);
        k0.set_interp_mode(InterpMode::Linear);
        spline.set_knot(k0);

        let k1 = Knot::at_time(10.0, 100.0);
        spline.set_knot(k1);

        assert_eq!(spline.eval(0.0), Some(0.0));
        assert_eq!(spline.eval(10.0), Some(100.0));
        assert_eq!(spline.eval(5.0), Some(50.0));
    }

    #[test]
    fn test_spline_held_interp() {
        let mut spline = Spline::new();

        let mut k0 = Knot::at_time(0.0, 10.0);
        k0.set_interp_mode(InterpMode::Held);
        spline.set_knot(k0);

        spline.set_knot(Knot::at_time(10.0, 20.0));

        assert_eq!(spline.eval(5.0), Some(10.0));
    }

    #[test]
    fn test_spline_knots_in_interval() {
        let mut spline = Spline::new();
        spline.set_knot(Knot::at_time(0.0, 0.0));
        spline.set_knot(Knot::at_time(5.0, 10.0));
        spline.set_knot(Knot::at_time(10.0, 20.0));
        spline.set_knot(Knot::at_time(15.0, 30.0));

        let interval = Interval::new(7.0, 12.0, true, true);
        let knots = spline.knots_in_interval(&interval);
        assert_eq!(knots.len(), 3);
        let times: Vec<_> = knots.iter().map(|k| k.time()).collect();
        assert_eq!(times, [5.0, 10.0, 15.0]);
    }

    #[test]
    fn test_spline_time_range() {
        let mut spline = Spline::new();
        spline.set_knot(Knot::at_time(5.0, 0.0));
        spline.set_knot(Knot::at_time(15.0, 0.0));

        let range = spline.time_range().expect("value expected");
        assert_eq!(range.get_min(), 5.0);
        assert_eq!(range.get_max(), 15.0);
    }

    #[test]
    fn test_spline_remove_knot() {
        let mut spline = Spline::new();
        spline.set_knot(Knot::at_time(5.0, 10.0));

        assert!(spline.has_knot_at(5.0));
        assert!(spline.remove_knot(5.0));
        assert!(!spline.has_knot_at(5.0));
    }

    #[test]
    fn test_spline_copy_on_write() {
        let mut spline1 = Spline::new();
        spline1.set_knot(Knot::at_time(0.0, 0.0));

        let spline2 = spline1.clone();
        assert!(Arc::ptr_eq(&spline1.data, &spline2.data));

        // Modify spline1
        spline1.set_knot(Knot::at_time(10.0, 10.0));
        assert!(!Arc::ptr_eq(&spline1.data, &spline2.data));
        assert_eq!(spline1.knot_count(), 2);
        assert_eq!(spline2.knot_count(), 1);
    }

    #[test]
    fn test_spline_extrapolation() {
        let mut spline = Spline::new();

        let mut k = Knot::at_time(0.0, 10.0);
        k.set_interp_mode(InterpMode::Linear);
        spline.set_knot(k);

        spline.set_knot(Knot::at_time(10.0, 20.0));

        // Default held extrapolation
        assert_eq!(spline.eval(-5.0), Some(10.0));
        assert_eq!(spline.eval(15.0), Some(20.0));
    }

    #[test]
    fn test_spline_diff() {
        let mut s1 = Spline::new();
        s1.set_knot(Knot::at_time(0.0, 0.0));
        s1.set_knot(Knot::at_time(10.0, 100.0));

        let s2 = s1.clone();
        let diff = s1.diff(&s2);
        assert!(diff.is_empty());

        // Different spline
        let mut s3 = Spline::new();
        s3.set_knot(Knot::at_time(0.0, 0.0));
        s3.set_knot(Knot::at_time(10.0, 50.0)); // Different value

        let diff = s1.diff(&s3);
        assert!(!diff.is_empty());
    }

    #[test]
    fn test_spline_breakdown() {
        let mut spline = Spline::new();

        let mut k0 = Knot::at_time(0.0, 0.0);
        k0.set_interp_mode(InterpMode::Linear);
        spline.set_knot(k0);

        spline.set_knot(Knot::at_time(10.0, 100.0));

        // Breakdown at midpoint
        assert!(spline.breakdown(5.0));
        assert_eq!(spline.knot_count(), 3);
        assert!(spline.has_knot_at(5.0));

        // Value should be preserved
        assert!((spline.eval(5.0).expect("value expected") - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_spline_sample() {
        let mut spline = Spline::new();

        let mut k0 = Knot::at_time(0.0, 0.0);
        k0.set_interp_mode(InterpMode::Linear);
        spline.set_knot(k0);

        spline.set_knot(Knot::at_time(10.0, 100.0));

        let interval = Interval::new(0.0, 10.0, true, true);
        let samples = spline.sample(&interval, 1.0, 1.0, 1.0);

        assert!(samples.is_some());
        let samples = samples.expect("value expected");
        assert!(!samples.polylines.is_empty());
    }

    #[test]
    fn test_spline_value_range() {
        let mut spline = Spline::new();
        spline.set_knot(Knot::at_time(0.0, 10.0));
        spline.set_knot(Knot::at_time(5.0, 50.0));
        spline.set_knot(Knot::at_time(10.0, 30.0));

        let range = spline.value_range();
        assert!(range.is_some());
        let (min, max) = range.expect("value expected");
        assert_eq!(min, 10.0);
        assert_eq!(max, 50.0);
    }

    #[test]
    fn test_spline_has_value_blocks() {
        let mut spline = Spline::new();

        let mut k = Knot::at_time(0.0, 0.0);
        k.set_interp_mode(InterpMode::ValueBlock);
        spline.set_knot(k);

        assert!(spline.has_value_blocks());
    }

    #[test]
    fn test_spline_apply_offset_and_scale() {
        let mut spline = Spline::new();
        spline.set_knot(Knot::at_time(1.0, 10.0));

        spline.apply_offset_and_scale(5.0, 2.0);

        // time = 1.0 * 2.0 + 5.0 = 7.0
        assert!(spline.has_knot_at(7.0));
        assert!(!spline.has_knot_at(1.0));
    }

    #[test]
    fn test_spline_inner_loops() {
        let mut spline = Spline::new();
        spline.set_knot(Knot::at_time(0.0, 0.0));
        spline.set_knot(Knot::at_time(10.0, 100.0));

        let params = LoopParams {
            proto_start: 0.0,
            proto_end: 10.0,
            num_pre_loops: 2,
            num_post_loops: 2,
            value_offset: 100.0,
        };
        spline.set_inner_loop_params(params);

        assert!(spline.has_inner_loops());

        let baked = spline.knots_with_inner_loops_baked();
        // 2 pre-loops + 1 prototype + 2 post-loops = 5 iterations * 2 knots = 10 knots
        assert_eq!(baked.len(), 10);
    }

    #[test]
    fn test_spline_is_linear() {
        let mut spline = Spline::new();

        let mut k0 = Knot::at_time(0.0, 0.0);
        k0.set_interp_mode(InterpMode::Linear);
        spline.set_knot(k0);

        let mut k1 = Knot::at_time(10.0, 100.0);
        k1.set_interp_mode(InterpMode::Linear);
        spline.set_knot(k1);

        assert!(spline.is_linear());
    }

    #[test]
    fn test_spline_continuity() {
        let mut spline = Spline::new();
        spline.set_knot(Knot::at_time(0.0, 0.0));
        spline.set_knot(Knot::at_time(10.0, 100.0));

        assert!(spline.is_c0_continuous());
        assert!(spline.is_g1_continuous());
        assert!(spline.is_c1_continuous());
    }

    #[test]
    fn test_spline_equality() {
        let mut s1 = Spline::new();
        s1.set_knot(Knot::at_time(0.0, 0.0));

        let s2 = s1.clone();
        assert_eq!(s1, s2);

        let mut s3 = Spline::new();
        s3.set_knot(Knot::at_time(0.0, 1.0)); // Different value

        assert_ne!(s1, s3);
    }

    #[test]
    fn test_auto_tangent_interior_knot() {
        // Three knots: (0,0), (5,10), (10,20) => linear, slope=2
        // Interior knot at idx=1 should get slope = (20-0)/(10-0) = 2.0
        let mut spline = Spline::new();

        let mut k0 = Knot::at_time(0.0, 0.0);
        k0.set_interp_mode(InterpMode::Curve);
        k0.set_pre_tan_algorithm(TangentAlgorithm::AutoEase);
        k0.set_post_tan_algorithm(TangentAlgorithm::AutoEase);
        spline.set_knot(k0);

        let mut k1 = Knot::at_time(5.0, 10.0);
        k1.set_interp_mode(InterpMode::Curve);
        k1.set_pre_tan_algorithm(TangentAlgorithm::AutoEase);
        k1.set_post_tan_algorithm(TangentAlgorithm::AutoEase);
        spline.set_knot(k1);

        let mut k2 = Knot::at_time(10.0, 20.0);
        k2.set_interp_mode(InterpMode::Curve);
        k2.set_pre_tan_algorithm(TangentAlgorithm::AutoEase);
        k2.set_post_tan_algorithm(TangentAlgorithm::AutoEase);
        spline.set_knot(k2);

        // Interior knot at index 1: central difference = (20-0)/(10-0) = 2.0
        let middle_knot = spline.knots().nth(1).unwrap();
        assert!(
            (middle_knot.pre_tan_slope() - 2.0).abs() < 1e-10,
            "pre slope: {}",
            middle_knot.pre_tan_slope()
        );
        assert!(
            (middle_knot.post_tan_slope() - 2.0).abs() < 1e-10,
            "post slope: {}",
            middle_knot.post_tan_slope()
        );
    }

    #[test]
    fn test_auto_tangent_edge_knots() {
        // Two knots: (0,0), (10,30) => slope = 3.0
        let mut spline = Spline::new();

        let mut k0 = Knot::at_time(0.0, 0.0);
        k0.set_interp_mode(InterpMode::Curve);
        k0.set_pre_tan_algorithm(TangentAlgorithm::AutoEase);
        k0.set_post_tan_algorithm(TangentAlgorithm::AutoEase);
        spline.set_knot(k0);

        let mut k1 = Knot::at_time(10.0, 30.0);
        k1.set_interp_mode(InterpMode::Curve);
        k1.set_pre_tan_algorithm(TangentAlgorithm::AutoEase);
        k1.set_post_tan_algorithm(TangentAlgorithm::AutoEase);
        spline.set_knot(k1);

        // First knot: one-sided forward = (30-0)/(10-0) = 3.0
        let first = spline.knots().next().unwrap();
        assert!(
            (first.post_tan_slope() - 3.0).abs() < 1e-10,
            "first post slope: {}",
            first.post_tan_slope()
        );

        // Last knot: one-sided backward = (30-0)/(10-0) = 3.0
        let last = spline.knots().last().unwrap();
        assert!(
            (last.pre_tan_slope() - 3.0).abs() < 1e-10,
            "last pre slope: {}",
            last.pre_tan_slope()
        );
    }

    #[test]
    fn test_auto_tangent_no_algo_unchanged() {
        // Knot with TangentAlgorithm::None should NOT get updated
        let mut spline = Spline::new();

        let mut k0 = Knot::at_time(0.0, 0.0);
        k0.set_interp_mode(InterpMode::Curve);
        // Default algorithm is None => not automatic
        spline.set_knot(k0);

        spline.set_knot(Knot::at_time(10.0, 100.0));

        // Tangent slope stays at default (0.0) because algorithm is None
        let first = spline.knots().next().unwrap();
        assert!(
            first.post_tan_slope().abs() < 1e-10,
            "slope should stay 0: {}",
            first.post_tan_slope()
        );
    }
}
