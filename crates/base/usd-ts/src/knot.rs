//! TsKnot - Knot belonging to a spline.
//!
//! Port of pxr/base/ts/knot.h

use super::knot_data::{KnotData, KnotValueType, TypedKnotData};
use super::types::{CurveType, InterpMode, TangentAlgorithm, TsTime};
use usd_vt::Dictionary;

/// Tangent for a knot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tangent {
    /// Tangent slope (value units per time unit).
    pub slope: f64,
    /// Tangent length/width (time units).
    pub width: f64,
}

impl Default for Tangent {
    fn default() -> Self {
        Self {
            slope: 0.0,
            width: 1.0,
        }
    }
}

impl Tangent {
    /// Creates a new tangent with given slope and width.
    pub fn new(slope: f64, width: f64) -> Self {
        Self { slope, width }
    }

    /// Creates a zero tangent.
    pub fn zero() -> Self {
        Self {
            slope: 0.0,
            width: 0.0,
        }
    }
}

/// A knot belonging to a spline.
///
/// Knots are the control points that define a spline curve. Each knot
/// has a time, value, and optional tangent information.
#[derive(Debug, Clone)]
pub struct Knot {
    /// Knot time.
    time: TsTime,
    /// Knot value (stored as f64, converted on access for other types).
    value: f64,
    /// Pre-value for dual-valued knots.
    pre_value: f64,
    /// Whether this knot is dual-valued.
    dual_valued: bool,
    /// Value type.
    value_type: KnotValueType,
    /// Interpolation mode for the segment following this knot.
    interp_mode: InterpMode,
    /// Curve type (deprecated but maintained for compatibility).
    curve_type: CurveType,
    /// Pre-tangent (incoming).
    pre_tangent: Tangent,
    /// Post-tangent (outgoing).
    post_tangent: Tangent,
    /// Pre-tangent algorithm.
    pre_tan_algorithm: TangentAlgorithm,
    /// Post-tangent algorithm.
    post_tan_algorithm: TangentAlgorithm,
    /// Whether tangents are in auto mode.
    auto_tangents: bool,
    /// Custom data attached to this knot.
    custom_data: Option<Dictionary>,
}

impl Default for Knot {
    fn default() -> Self {
        Self {
            time: 0.0,
            value: 0.0,
            pre_value: 0.0,
            dual_valued: false,
            value_type: KnotValueType::Double,
            interp_mode: InterpMode::Curve,
            curve_type: CurveType::Bezier,
            pre_tangent: Tangent::default(),
            post_tangent: Tangent::default(),
            pre_tan_algorithm: TangentAlgorithm::None,
            post_tan_algorithm: TangentAlgorithm::None,
            auto_tangents: true,
            custom_data: None,
        }
    }
}

impl Knot {
    /// Creates a new double-typed knot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a knot with a specified value type.
    pub fn with_value_type(value_type: KnotValueType) -> Self {
        Self {
            value_type,
            ..Default::default()
        }
    }

    /// Creates a knot with a specified value type and curve type.
    #[deprecated(note = "Curve type for knots is deprecated; knots work in any spline type")]
    pub fn with_value_type_and_curve(value_type: KnotValueType, curve_type: CurveType) -> Self {
        Self {
            value_type,
            curve_type,
            ..Default::default()
        }
    }

    /// Creates a knot at a specific time with a value.
    pub fn at_time(time: TsTime, value: f64) -> Self {
        Self {
            time,
            value,
            pre_value: value,
            ..Default::default()
        }
    }

    // =========================================================================
    // Time
    // =========================================================================

    /// Returns the knot time.
    #[inline]
    pub fn time(&self) -> TsTime {
        self.time
    }

    /// Sets the knot time.
    pub fn set_time(&mut self, time: TsTime) -> bool {
        if !time.is_finite() {
            return false;
        }
        self.time = time;
        true
    }

    // =========================================================================
    // Interpolation mode
    // =========================================================================

    /// Returns the interpolation mode.
    #[inline]
    pub fn interp_mode(&self) -> InterpMode {
        self.interp_mode
    }

    /// Sets the interpolation mode.
    pub fn set_interp_mode(&mut self, mode: InterpMode) {
        self.interp_mode = mode;
    }

    // =========================================================================
    // Value type
    // =========================================================================

    /// Returns the value type.
    #[inline]
    pub fn value_type(&self) -> KnotValueType {
        self.value_type
    }

    /// Returns true if holding the specified type.
    pub fn is_holding<T: 'static>(&self) -> bool {
        match self.value_type {
            KnotValueType::Double => std::any::TypeId::of::<T>() == std::any::TypeId::of::<f64>(),
            KnotValueType::Float => std::any::TypeId::of::<T>() == std::any::TypeId::of::<f32>(),
            KnotValueType::Half => false, // Half type not directly supported
        }
    }

    // =========================================================================
    // Value
    // =========================================================================

    /// Returns the value as f64.
    #[inline]
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Returns the value as f32.
    #[inline]
    pub fn value_f32(&self) -> f32 {
        self.value as f32
    }

    /// Sets the value.
    pub fn set_value(&mut self, value: f64) {
        if !value.is_finite() {
            return;
        }
        self.value = value;
        if !self.dual_valued {
            self.pre_value = value;
        }
    }

    /// Sets the value from f32.
    pub fn set_value_f32(&mut self, value: f32) {
        self.set_value(value as f64);
        self.value_type = KnotValueType::Float;
    }

    // =========================================================================
    // Dual values
    // =========================================================================

    /// Returns true if this knot is dual-valued.
    #[inline]
    pub fn is_dual_valued(&self) -> bool {
        self.dual_valued
    }

    /// Returns the pre-value (left-side value at discontinuity).
    /// If not dual-valued, returns the regular value.
    #[inline]
    pub fn pre_value(&self) -> f64 {
        if self.dual_valued {
            self.pre_value
        } else {
            self.value
        }
    }

    /// Returns pre-value as f32.
    #[inline]
    pub fn pre_value_f32(&self) -> f32 {
        self.pre_value() as f32
    }

    /// Sets the pre-value, making this knot dual-valued.
    pub fn set_pre_value(&mut self, value: f64) -> bool {
        if !value.is_finite() {
            return false;
        }
        self.dual_valued = true;
        self.pre_value = value;
        true
    }

    /// Sets pre-value from f32.
    pub fn set_pre_value_f32(&mut self, value: f32) -> bool {
        self.set_pre_value(value as f64)
    }

    /// Clears the pre-value, making this knot single-valued.
    pub fn clear_pre_value(&mut self) -> bool {
        self.dual_valued = false;
        self.pre_value = self.value;
        true
    }

    // =========================================================================
    // Curve type (deprecated)
    // =========================================================================

    /// Returns the curve type.
    #[deprecated(note = "Curve type for knots is deprecated")]
    pub fn curve_type(&self) -> CurveType {
        self.curve_type
    }

    /// Sets the curve type.
    #[deprecated(note = "Curve type for knots is deprecated")]
    #[allow(deprecated)]
    pub fn set_curve_type(&mut self, curve_type: CurveType) -> bool {
        self.curve_type = curve_type;
        true
    }

    // =========================================================================
    // Pre-tangent
    // =========================================================================

    /// Returns the pre-tangent (incoming).
    #[inline]
    pub fn pre_tangent(&self) -> &Tangent {
        &self.pre_tangent
    }

    /// Returns a mutable reference to the pre-tangent.
    #[inline]
    pub fn pre_tangent_mut(&mut self) -> &mut Tangent {
        &mut self.pre_tangent
    }

    /// Sets the pre-tangent.
    pub fn set_pre_tangent(&mut self, tangent: Tangent) {
        self.pre_tangent = tangent;
        self.auto_tangents = false;
    }

    /// Sets the pre-tangent width.
    pub fn set_pre_tan_width(&mut self, width: TsTime) -> bool {
        if width < 0.0 || !width.is_finite() {
            return false;
        }
        self.pre_tangent.width = width;
        self.auto_tangents = false;
        true
    }

    /// Returns the pre-tangent width.
    #[inline]
    pub fn pre_tan_width(&self) -> TsTime {
        self.pre_tangent.width
    }

    /// Sets the pre-tangent slope.
    pub fn set_pre_tan_slope(&mut self, slope: f64) -> bool {
        if !slope.is_finite() {
            return false;
        }
        self.pre_tangent.slope = slope;
        self.auto_tangents = false;
        true
    }

    /// Returns the pre-tangent slope.
    #[inline]
    pub fn pre_tan_slope(&self) -> f64 {
        self.pre_tangent.slope
    }

    /// Sets the pre-tangent algorithm.
    pub fn set_pre_tan_algorithm(&mut self, algorithm: TangentAlgorithm) -> bool {
        self.pre_tan_algorithm = algorithm;
        true
    }

    /// Returns the pre-tangent algorithm.
    #[inline]
    pub fn pre_tan_algorithm(&self) -> TangentAlgorithm {
        self.pre_tan_algorithm
    }

    // =========================================================================
    // Post-tangent
    // =========================================================================

    /// Returns the post-tangent (outgoing).
    #[inline]
    pub fn post_tangent(&self) -> &Tangent {
        &self.post_tangent
    }

    /// Returns a mutable reference to the post-tangent.
    #[inline]
    pub fn post_tangent_mut(&mut self) -> &mut Tangent {
        &mut self.post_tangent
    }

    /// Sets the post-tangent.
    pub fn set_post_tangent(&mut self, tangent: Tangent) {
        self.post_tangent = tangent;
        self.auto_tangents = false;
    }

    /// Sets the post-tangent width.
    pub fn set_post_tan_width(&mut self, width: TsTime) -> bool {
        if width < 0.0 || !width.is_finite() {
            return false;
        }
        self.post_tangent.width = width;
        self.auto_tangents = false;
        true
    }

    /// Returns the post-tangent width.
    #[inline]
    pub fn post_tan_width(&self) -> TsTime {
        self.post_tangent.width
    }

    /// Sets the post-tangent slope.
    pub fn set_post_tan_slope(&mut self, slope: f64) -> bool {
        if !slope.is_finite() {
            return false;
        }
        self.post_tangent.slope = slope;
        self.auto_tangents = false;
        true
    }

    /// Returns the post-tangent slope.
    #[inline]
    pub fn post_tan_slope(&self) -> f64 {
        self.post_tangent.slope
    }

    /// Sets the post-tangent algorithm.
    pub fn set_post_tan_algorithm(&mut self, algorithm: TangentAlgorithm) -> bool {
        self.post_tan_algorithm = algorithm;
        true
    }

    /// Returns the post-tangent algorithm.
    #[inline]
    pub fn post_tan_algorithm(&self) -> TangentAlgorithm {
        self.post_tan_algorithm
    }

    // =========================================================================
    // Tangent utilities
    // =========================================================================

    /// Sets both tangents to the same value.
    pub fn set_tangents(&mut self, slope: f64, width: f64) {
        let tangent = Tangent::new(slope, width);
        self.pre_tangent = tangent;
        self.post_tangent = tangent;
        self.auto_tangents = false;
    }

    /// Returns whether tangents are in auto mode.
    #[inline]
    pub fn auto_tangents(&self) -> bool {
        self.auto_tangents
    }

    /// Sets auto tangent mode.
    pub fn set_auto_tangents(&mut self, auto_mode: bool) {
        self.auto_tangents = auto_mode;
    }

    /// Updates tangent values algorithmically based on neighbor knots.
    ///
    /// Uses the tangent algorithms set on this knot to compute updated
    /// tangent values. Pass None for prev/next if at spline endpoints.
    pub fn update_tangents(
        &mut self,
        prev_knot: Option<&Knot>,
        next_knot: Option<&Knot>,
        curve_type: CurveType,
    ) -> bool {
        let mut updated = false;

        // Validate neighbor times
        if let Some(prev) = prev_knot {
            if prev.time >= self.time {
                return false;
            }
        }
        if let Some(next) = next_knot {
            if next.time <= self.time {
                return false;
            }
        }

        // Process pre-tangent algorithm
        if self.pre_tan_algorithm == TangentAlgorithm::AutoEase {
            if let Some(slope) = Self::compute_auto_ease_slope(prev_knot, self, next_knot, true) {
                self.pre_tangent.slope = slope;
                updated = true;
            }
        }

        // Process post-tangent algorithm
        if self.post_tan_algorithm == TangentAlgorithm::AutoEase {
            if let Some(slope) = Self::compute_auto_ease_slope(prev_knot, self, next_knot, false) {
                self.post_tangent.slope = slope;
                updated = true;
            }
        }

        // For Hermite curves, widths are always 1/3 of segment
        if curve_type == CurveType::Hermite {
            if let Some(prev) = prev_knot {
                let segment_width = self.time - prev.time;
                self.pre_tangent.width = segment_width / 3.0;
                updated = true;
            }
            if let Some(next) = next_knot {
                let segment_width = next.time - self.time;
                self.post_tangent.width = segment_width / 3.0;
                updated = true;
            }
        }

        updated
    }

    /// Computes Auto Ease slope (Maya/animX algorithm).
    fn compute_auto_ease_slope(
        prev: Option<&Knot>,
        current: &Knot,
        next: Option<&Knot>,
        is_pre: bool,
    ) -> Option<f64> {
        // Compute slopes to neighbors
        let slope_to_prev = prev.map(|p| {
            let dt = current.time - p.time;
            if dt > 1e-10 {
                (current.pre_value() - p.value) / dt
            } else {
                0.0
            }
        });

        let slope_to_next = next.map(|n| {
            let dt = n.time - current.time;
            if dt > 1e-10 {
                (n.pre_value() - current.value) / dt
            } else {
                0.0
            }
        });

        // Auto Ease: average of slopes to neighbors
        match (slope_to_prev, slope_to_next) {
            (Some(sp), Some(sn)) => {
                // If slopes have different signs, use 0 (flat)
                if sp * sn < 0.0 {
                    Some(0.0)
                } else {
                    Some((sp + sn) / 2.0)
                }
            }
            (Some(sp), None) => Some(if is_pre { sp } else { 0.0 }),
            (None, Some(sn)) => Some(if is_pre { 0.0 } else { sn }),
            (None, None) => Some(0.0),
        }
    }

    // =========================================================================
    // Continuity queries
    // =========================================================================

    /// Returns true if C0 continuous (values match).
    /// Not yet fully implemented.
    pub fn is_c0_continuous(&self) -> bool {
        !self.dual_valued
    }

    /// Returns true if G1 continuous (tangent directions match).
    /// Not yet fully implemented.
    pub fn is_g1_continuous(&self) -> bool {
        if self.dual_valued {
            return false;
        }
        // Tangent slopes must have same sign (or be zero)
        self.pre_tangent.slope * self.post_tangent.slope >= 0.0
    }

    /// Returns true if C1 continuous (tangent slopes match).
    /// Not yet fully implemented.
    pub fn is_c1_continuous(&self) -> bool {
        if self.dual_valued {
            return false;
        }
        (self.pre_tangent.slope - self.post_tangent.slope).abs() < 1e-10
    }

    // =========================================================================
    // Custom data
    // =========================================================================

    /// Returns the custom data, if any.
    pub fn custom_data(&self) -> Option<&Dictionary> {
        self.custom_data.as_ref()
    }

    /// Sets custom data.
    pub fn set_custom_data(&mut self, data: Dictionary) {
        self.custom_data = Some(data);
    }

    /// Sets a custom data value by key path.
    pub fn set_custom_data_by_key(&mut self, key_path: &str, value: usd_vt::Value) -> bool {
        let dict = self.custom_data.get_or_insert_with(Dictionary::new);
        dict.insert(key_path.to_string(), value);
        true
    }

    /// Gets a custom data value by key path.
    pub fn custom_data_by_key(&self, key_path: &str) -> Option<&usd_vt::Value> {
        self.custom_data.as_ref()?.get(key_path)
    }

    /// Clears custom data.
    pub fn clear_custom_data(&mut self) {
        self.custom_data = None;
    }

    /// Returns true if this knot has custom data.
    pub fn has_custom_data(&self) -> bool {
        self.custom_data.is_some()
    }

    // =========================================================================
    // Conversion
    // =========================================================================

    /// Converts this knot to underlying TypedKnotData.
    pub fn to_typed_knot_data(&self) -> TypedKnotData<f64> {
        let mut data = TypedKnotData::<f64>::new();
        data.base.time = self.time;
        data.value = self.value;
        data.pre_value = self.pre_value;
        data.base.dual_valued = self.dual_valued;
        data.pre_tan_slope = self.pre_tangent.slope;
        data.post_tan_slope = self.post_tangent.slope;
        data.base.pre_tan_width = self.pre_tangent.width;
        data.base.post_tan_width = self.post_tangent.width;
        data.base.next_interp = self.interp_mode;
        data.base.curve_type = self.curve_type;
        data.base.pre_tan_algorithm = self.pre_tan_algorithm;
        data.base.post_tan_algorithm = self.post_tan_algorithm;
        data
    }

    /// Converts this knot to base KnotData (without value).
    pub fn to_knot_data(&self) -> KnotData {
        let mut data = KnotData::new();
        data.time = self.time;
        data.pre_tan_width = self.pre_tangent.width;
        data.post_tan_width = self.post_tangent.width;
        data.next_interp = self.interp_mode;
        data.curve_type = self.curve_type;
        data.dual_valued = self.dual_valued;
        data.pre_tan_algorithm = self.pre_tan_algorithm;
        data.post_tan_algorithm = self.post_tan_algorithm;
        data
    }

    /// Creates a knot from TypedKnotData.
    pub fn from_typed_knot_data(data: &TypedKnotData<f64>) -> Self {
        Self {
            time: data.base.time,
            value: data.value,
            pre_value: data.pre_value,
            dual_valued: data.base.dual_valued,
            value_type: KnotValueType::Double,
            interp_mode: data.base.next_interp,
            curve_type: data.base.curve_type,
            pre_tangent: Tangent::new(data.pre_tan_slope, data.base.pre_tan_width),
            post_tangent: Tangent::new(data.post_tan_slope, data.base.post_tan_width),
            pre_tan_algorithm: data.base.pre_tan_algorithm,
            post_tan_algorithm: data.base.post_tan_algorithm,
            auto_tangents: false,
            custom_data: None,
        }
    }
}

impl PartialEq for Knot {
    /// Matches C++ TsKnot::operator== which compares knot data AND custom_data.
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
            && self.value == other.value
            && self.pre_value == other.pre_value
            && self.dual_valued == other.dual_valued
            && self.value_type == other.value_type
            && self.interp_mode == other.interp_mode
            && self.pre_tangent == other.pre_tangent
            && self.post_tangent == other.post_tangent
            && self.pre_tan_algorithm == other.pre_tan_algorithm
            && self.post_tan_algorithm == other.post_tan_algorithm
            && self.custom_data == other.custom_data
    }
}

/// Typed knot for compile-time type safety.
#[derive(Debug, Clone)]
pub struct TypedKnot<T> {
    /// Inner knot data.
    inner: Knot,
    /// Phantom for type.
    _marker: std::marker::PhantomData<T>,
}

impl<T> TypedKnot<T> {
    /// Creates a new typed knot.
    pub fn new() -> Self
    where
        T: Default,
    {
        Self {
            inner: Knot::new(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Returns a reference to the inner untyped knot.
    pub fn inner(&self) -> &Knot {
        &self.inner
    }

    /// Returns a mutable reference to the inner untyped knot.
    pub fn inner_mut(&mut self) -> &mut Knot {
        &mut self.inner
    }

    /// Unwraps the typed knot into an untyped knot.
    pub fn into_inner(self) -> Knot {
        self.inner
    }
}

impl<T: Default> Default for TypedKnot<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl TypedKnot<f64> {
    /// Creates a knot at a specific time with a value.
    pub fn at_time(time: TsTime, value: f64) -> Self {
        Self {
            inner: Knot::at_time(time, value),
            _marker: std::marker::PhantomData,
        }
    }

    /// Returns the value.
    pub fn value(&self) -> f64 {
        self.inner.value()
    }

    /// Sets the value.
    pub fn set_value(&mut self, value: f64) {
        self.inner.set_value(value);
    }
}

impl TypedKnot<f32> {
    /// Creates a knot at a specific time with a value.
    pub fn at_time(time: TsTime, value: f32) -> Self {
        Self {
            inner: Knot {
                value_type: KnotValueType::Float,
                ..Knot::at_time(time, value as f64)
            },
            _marker: std::marker::PhantomData,
        }
    }

    /// Returns the value.
    pub fn value(&self) -> f32 {
        self.inner.value_f32()
    }

    /// Sets the value.
    pub fn set_value(&mut self, value: f32) {
        self.inner.set_value_f32(value);
    }
}

/// Type alias for double-typed knots. Matches C++ `TsDoubleKnot`.
pub type DoubleKnot = TypedKnot<f64>;

/// Type alias for float-typed knots. Matches C++ `TsFloatKnot`.
pub type FloatKnot = TypedKnot<f32>;

/// Type alias for half-typed knots. Matches C++ `TsHalfKnot`.
pub type HalfKnot = TypedKnot<usd_gf::Half>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knot_default() {
        let knot = Knot::new();
        assert_eq!(knot.time(), 0.0);
        assert_eq!(knot.value(), 0.0);
        assert_eq!(knot.value_type(), KnotValueType::Double);
        assert!(knot.auto_tangents());
        assert!(!knot.is_dual_valued());
    }

    #[test]
    fn test_knot_at_time() {
        let knot = Knot::at_time(10.0, 5.0);
        assert_eq!(knot.time(), 10.0);
        assert_eq!(knot.value(), 5.0);
    }

    #[test]
    fn test_knot_tangents() {
        let mut knot = Knot::new();
        assert!(knot.auto_tangents());

        knot.set_tangents(1.0, 2.0);
        assert!(!knot.auto_tangents());
        assert_eq!(knot.pre_tangent().slope, 1.0);
        assert_eq!(knot.post_tangent().width, 2.0);
    }

    #[test]
    fn test_knot_dual_valued() {
        let mut knot = Knot::at_time(1.0, 10.0);
        assert!(!knot.is_dual_valued());
        assert_eq!(knot.pre_value(), 10.0);

        knot.set_pre_value(5.0);
        assert!(knot.is_dual_valued());
        assert_eq!(knot.pre_value(), 5.0);
        assert_eq!(knot.value(), 10.0);

        knot.clear_pre_value();
        assert!(!knot.is_dual_valued());
        assert_eq!(knot.pre_value(), 10.0);
    }

    #[test]
    fn test_knot_tangent_algorithms() {
        let mut knot = Knot::new();
        assert_eq!(knot.pre_tan_algorithm(), TangentAlgorithm::None);
        assert_eq!(knot.post_tan_algorithm(), TangentAlgorithm::None);

        knot.set_pre_tan_algorithm(TangentAlgorithm::AutoEase);
        knot.set_post_tan_algorithm(TangentAlgorithm::Custom);
        assert_eq!(knot.pre_tan_algorithm(), TangentAlgorithm::AutoEase);
        assert_eq!(knot.post_tan_algorithm(), TangentAlgorithm::Custom);
    }

    #[test]
    fn test_knot_continuity() {
        let mut knot = Knot::new();
        knot.set_tangents(1.0, 1.0);
        assert!(knot.is_c0_continuous());
        assert!(knot.is_g1_continuous());
        assert!(knot.is_c1_continuous());

        // Different slopes break C1
        knot.set_pre_tan_slope(1.0);
        knot.set_post_tan_slope(2.0);
        assert!(knot.is_c0_continuous());
        assert!(knot.is_g1_continuous()); // Same direction
        assert!(!knot.is_c1_continuous());

        // Opposite signs break G1
        knot.set_pre_tan_slope(-1.0);
        knot.set_post_tan_slope(1.0);
        assert!(!knot.is_g1_continuous());
    }

    #[test]
    fn test_typed_knot() {
        let knot = TypedKnot::<f64>::at_time(5.0, 10.0);
        assert_eq!(knot.value(), 10.0);

        let knot_f32 = TypedKnot::<f32>::at_time(5.0, 10.0);
        assert_eq!(knot_f32.value(), 10.0);
    }

    #[test]
    fn test_knot_data_conversion() {
        let mut knot = Knot::at_time(5.0, 10.0);
        knot.set_tangents(2.0, 1.5);
        knot.set_pre_value(8.0);
        knot.set_pre_tan_algorithm(TangentAlgorithm::AutoEase);

        let data = knot.to_typed_knot_data();
        assert_eq!(data.base.time, 5.0);
        assert_eq!(data.value, 10.0);
        assert_eq!(data.pre_tan_slope, 2.0);
        assert!(data.base.dual_valued);
        assert_eq!(data.pre_value, 8.0);
        assert_eq!(data.base.pre_tan_algorithm, TangentAlgorithm::AutoEase);

        let restored = Knot::from_typed_knot_data(&data);
        assert_eq!(restored.time(), 5.0);
        assert_eq!(restored.value(), 10.0);
        assert!(restored.is_dual_valued());
        assert_eq!(restored.pre_value(), 8.0);
    }

    #[test]
    fn test_update_tangents_auto_ease() {
        let prev = Knot::at_time(0.0, 0.0);
        let next = Knot::at_time(2.0, 2.0);
        let mut current = Knot::at_time(1.0, 1.0);
        current.set_pre_tan_algorithm(TangentAlgorithm::AutoEase);
        current.set_post_tan_algorithm(TangentAlgorithm::AutoEase);

        let updated = current.update_tangents(Some(&prev), Some(&next), CurveType::Bezier);
        assert!(updated);

        // Linear progression -> slope should be 1.0
        assert!((current.pre_tan_slope() - 1.0).abs() < 1e-10);
        assert!((current.post_tan_slope() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_update_tangents_hermite() {
        let prev = Knot::at_time(0.0, 0.0);
        let next = Knot::at_time(3.0, 3.0);
        let mut current = Knot::at_time(1.0, 1.0);

        current.update_tangents(Some(&prev), Some(&next), CurveType::Hermite);

        // Hermite: widths are 1/3 of segment
        assert!((current.pre_tan_width() - 1.0 / 3.0).abs() < 1e-10);
        assert!((current.post_tan_width() - 2.0 / 3.0).abs() < 1e-10);
    }
}
