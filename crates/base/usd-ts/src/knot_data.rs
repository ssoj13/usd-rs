//! Knot data structures for splines.
//!
//! This module provides low-level data structures for storing spline knot
//! information. Knots are the control points of a spline.
//!
//! # Overview
//!
//! The knot data system uses generics to support different value types
//! (f64, f32, half) while sharing common time-based fields.
//!
//! - [`KnotData`]: Base knot data with time, tangent widths, and flags
//! - [`TypedKnotData<T>`]: Type-specific knot data with values and slopes
//!
//! # Examples
//!
//! ```
//! use usd_ts::{KnotData, TypedKnotData, InterpMode, CurveType};
//!
//! // Create knot data at time 1.0
//! let mut knot = TypedKnotData::<f64>::new();
//! knot.base.time = 1.0;
//! knot.value = 10.0;
//! knot.base.next_interp = InterpMode::Curve;
//! ```

use std::fmt;

use super::types::{CurveType, InterpMode, TangentAlgorithm, TsTime};

/// Base knot data shared across all value types.
///
/// Contains time-based fields and flags that don't depend on
/// the specific value type of the spline.
///
/// # Examples
///
/// ```
/// use usd_ts::{KnotData, InterpMode, CurveType, TangentAlgorithm};
///
/// let mut data = KnotData::new();
/// data.time = 1.0;
/// data.pre_tan_width = 0.333;
/// data.post_tan_width = 0.333;
/// data.next_interp = InterpMode::Linear;
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnotData {
    /// Knot time.
    pub time: TsTime,

    /// Time width of the pre-tangent. Always non-negative.
    /// Ignored for Hermite knots.
    pub pre_tan_width: TsTime,

    /// Time width of the post-tangent. Always non-negative.
    /// Ignored for Hermite knots.
    pub post_tan_width: TsTime,

    /// Interpolation mode for the segment following this knot.
    pub next_interp: InterpMode,

    /// The spline curve type this knot belongs to or is intended for.
    /// Deprecated: knots now work in splines of any curve type.
    pub curve_type: CurveType,

    /// Whether this knot is dual-valued (value discontinuity at the knot).
    pub dual_valued: bool,

    /// Pre-tangent algorithm.
    pub pre_tan_algorithm: TangentAlgorithm,

    /// Post-tangent algorithm.
    pub post_tan_algorithm: TangentAlgorithm,
}

impl Default for KnotData {
    fn default() -> Self {
        Self::new()
    }
}

impl KnotData {
    /// Creates new knot data with default values.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            time: 0.0,
            pre_tan_width: 0.0,
            post_tan_width: 0.0,
            next_interp: InterpMode::default(),
            curve_type: CurveType::default(),
            dual_valued: false,
            pre_tan_algorithm: TangentAlgorithm::default(),
            post_tan_algorithm: TangentAlgorithm::default(),
        }
    }

    /// Returns the pre-tangent width.
    #[inline]
    #[must_use]
    pub fn get_pre_tan_width(&self) -> TsTime {
        self.pre_tan_width
    }

    /// Returns the post-tangent width.
    #[inline]
    #[must_use]
    pub fn get_post_tan_width(&self) -> TsTime {
        self.post_tan_width
    }

    /// Sets the pre-tangent width.
    #[inline]
    pub fn set_pre_tan_width(&mut self, width: TsTime) {
        self.pre_tan_width = width;
    }

    /// Sets the post-tangent width.
    #[inline]
    pub fn set_post_tan_width(&mut self, width: TsTime) {
        self.post_tan_width = width;
    }
}

impl fmt::Display for KnotData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "KnotData(t={}, interp={}, dual={})",
            self.time, self.next_interp, self.dual_valued
        )
    }
}

/// Type-specific knot data with values and tangent slopes.
///
/// This struct extends [`KnotData`] with value-type-specific fields.
/// Tangents are expressed as width and slope.
///
/// # Type Parameters
///
/// - `T`: The value type (typically f64, f32, or half)
///
/// # Examples
///
/// ```
/// use usd_ts::TypedKnotData;
///
/// let mut knot = TypedKnotData::<f64>::new();
/// knot.base.time = 1.0;
/// knot.value = 10.0;
/// knot.post_tan_slope = 2.5; // rising tangent
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypedKnotData<T> {
    /// Base knot data (time, widths, flags).
    pub base: KnotData,

    /// Value at this knot.
    pub value: T,

    /// If dual-valued, the pre-value at this knot.
    pub pre_value: T,

    /// Slope of the pre-tangent (rise over run, value height / time width).
    pub pre_tan_slope: T,

    /// Slope of the post-tangent (rise over run, value height / time width).
    pub post_tan_slope: T,
}

impl<T: Default> Default for TypedKnotData<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Default> TypedKnotData<T> {
    /// Creates new typed knot data with default values.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: KnotData::new(),
            value: T::default(),
            pre_value: T::default(),
            pre_tan_slope: T::default(),
            post_tan_slope: T::default(),
        }
    }
}

// Mutable field access for TypedKnotData<f64>
impl TypedKnotData<f64> {
    /// Returns the knot time.
    #[inline]
    pub fn time(&self) -> TsTime {
        self.base.time
    }

    /// Sets the knot time.
    #[inline]
    pub fn set_time(&mut self, t: TsTime) {
        self.base.time = t;
    }

    /// Returns the interpolation mode.
    #[inline]
    pub fn next_interp(&self) -> InterpMode {
        self.base.next_interp
    }

    /// Returns whether dual-valued.
    #[inline]
    pub fn is_dual_valued(&self) -> bool {
        self.base.dual_valued
    }

    /// Returns the pre-tangent width.
    #[inline]
    pub fn pre_tan_width(&self) -> TsTime {
        self.base.pre_tan_width
    }

    /// Returns the post-tangent width.
    #[inline]
    pub fn post_tan_width(&self) -> TsTime {
        self.base.post_tan_width
    }

    /// Returns the pre-value (or value if not dual-valued).
    #[inline]
    pub fn pre_value(&self) -> f64 {
        if self.base.dual_valued {
            self.pre_value
        } else {
            self.value
        }
    }

    /// Returns the pre-tangent height.
    #[inline]
    pub fn pre_tan_height(&self) -> f64 {
        -self.pre_tan_slope * self.base.pre_tan_width
    }

    /// Returns the post-tangent height.
    #[inline]
    pub fn post_tan_height(&self) -> f64 {
        self.post_tan_slope * self.base.post_tan_width
    }
}

impl<T: Copy> TypedKnotData<T> {
    /// Returns the pre-value (or value if not dual-valued).
    #[inline]
    #[must_use]
    pub fn get_pre_value(&self) -> T {
        if self.base.dual_valued {
            self.pre_value
        } else {
            self.value
        }
    }

    /// Returns the pre-tangent slope.
    #[inline]
    #[must_use]
    pub fn get_pre_tan_slope(&self) -> T {
        self.pre_tan_slope
    }

    /// Returns the post-tangent slope.
    #[inline]
    #[must_use]
    pub fn get_post_tan_slope(&self) -> T {
        self.post_tan_slope
    }
}

impl<T: Copy + std::ops::Mul<TsTime, Output = T> + std::ops::Neg<Output = T>> TypedKnotData<T> {
    /// Returns the pre-tangent height (negative width * slope).
    #[inline]
    #[must_use]
    pub fn get_pre_tan_height(&self) -> T {
        -self.pre_tan_slope * self.base.pre_tan_width
    }

    /// Returns the post-tangent height (width * slope).
    #[inline]
    #[must_use]
    pub fn get_post_tan_height(&self) -> T {
        self.post_tan_slope * self.base.post_tan_width
    }
}

impl<T: fmt::Display> fmt::Display for TypedKnotData<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TypedKnotData(t={}, v={}, interp={})",
            self.base.time, self.value, self.base.next_interp
        )
    }
}

/// Enumeration of supported knot value types.
///
/// Used for runtime type identification when working with
/// type-erased knot collections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KnotValueType {
    /// 64-bit floating point (double precision).
    Double,
    /// 32-bit floating point (single precision).
    Float,
    /// 16-bit floating point (half precision).
    Half,
}

impl KnotValueType {
    /// Returns the size in bytes of values of this type.
    #[inline]
    #[must_use]
    pub fn size_of(&self) -> usize {
        match self {
            Self::Double => std::mem::size_of::<f64>(),
            Self::Float => std::mem::size_of::<f32>(),
            Self::Half => 2, // GfHalf is 2 bytes
        }
    }
}

impl fmt::Display for KnotValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Double => write!(f, "double"),
            Self::Float => write!(f, "float"),
            Self::Half => write!(f, "half"),
        }
    }
}

/// Type-erased knot data that can hold any supported value type.
///
/// This enum provides a way to work with knots of different value types
/// in a unified manner.
///
/// # Examples
///
/// ```
/// use usd_ts::{AnyKnotData, KnotValueType};
///
/// let knot = AnyKnotData::new_double();
/// assert_eq!(knot.value_type(), KnotValueType::Double);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub enum AnyKnotData {
    /// Double-precision knot data.
    Double(TypedKnotData<f64>),
    /// Single-precision knot data.
    Float(TypedKnotData<f32>),
    // Note: Half would need GfHalf type support
}

impl AnyKnotData {
    /// Creates new double-precision knot data.
    #[inline]
    #[must_use]
    pub fn new_double() -> Self {
        Self::Double(TypedKnotData::new())
    }

    /// Creates new single-precision knot data.
    #[inline]
    #[must_use]
    pub fn new_float() -> Self {
        Self::Float(TypedKnotData::new())
    }

    /// Returns the value type of this knot data.
    #[inline]
    #[must_use]
    pub fn value_type(&self) -> KnotValueType {
        match self {
            Self::Double(_) => KnotValueType::Double,
            Self::Float(_) => KnotValueType::Float,
        }
    }

    /// Returns a reference to the base knot data.
    #[inline]
    #[must_use]
    pub fn base(&self) -> &KnotData {
        match self {
            Self::Double(d) => &d.base,
            Self::Float(f) => &f.base,
        }
    }

    /// Returns a mutable reference to the base knot data.
    #[inline]
    #[must_use]
    pub fn base_mut(&mut self) -> &mut KnotData {
        match self {
            Self::Double(d) => &mut d.base,
            Self::Float(f) => &mut f.base,
        }
    }

    /// Returns the time of this knot.
    #[inline]
    #[must_use]
    pub fn time(&self) -> TsTime {
        self.base().time
    }

    /// Sets the time of this knot.
    #[inline]
    pub fn set_time(&mut self, time: TsTime) {
        self.base_mut().time = time;
    }

    /// Returns the interpolation mode for the segment following this knot.
    #[inline]
    #[must_use]
    pub fn next_interp(&self) -> InterpMode {
        self.base().next_interp
    }

    /// Sets the interpolation mode for the segment following this knot.
    #[inline]
    pub fn set_next_interp(&mut self, mode: InterpMode) {
        self.base_mut().next_interp = mode;
    }

    /// Returns true if this knot is dual-valued.
    #[inline]
    #[must_use]
    pub fn is_dual_valued(&self) -> bool {
        self.base().dual_valued
    }

    /// Returns the value as f64 (converting if necessary).
    #[must_use]
    pub fn value_as_f64(&self) -> f64 {
        match self {
            Self::Double(d) => d.value,
            Self::Float(f) => f64::from(f.value),
        }
    }

    /// Sets the value from f64 (converting if necessary).
    pub fn set_value_f64(&mut self, value: f64) {
        match self {
            Self::Double(d) => d.value = value,
            Self::Float(f) => f.value = value as f32,
        }
    }

    /// Returns the pre-value as f64 (converting if necessary).
    #[must_use]
    pub fn pre_value_as_f64(&self) -> f64 {
        match self {
            Self::Double(d) => d.get_pre_value(),
            Self::Float(f) => f64::from(f.get_pre_value()),
        }
    }

    /// Returns the pre-tangent slope as f64.
    #[must_use]
    pub fn pre_tan_slope_as_f64(&self) -> f64 {
        match self {
            Self::Double(d) => d.pre_tan_slope,
            Self::Float(f) => f64::from(f.pre_tan_slope),
        }
    }

    /// Sets the pre-tangent slope from f64.
    pub fn set_pre_tan_slope_f64(&mut self, slope: f64) {
        match self {
            Self::Double(d) => d.pre_tan_slope = slope,
            Self::Float(f) => f.pre_tan_slope = slope as f32,
        }
    }

    /// Returns the post-tangent slope as f64.
    #[must_use]
    pub fn post_tan_slope_as_f64(&self) -> f64 {
        match self {
            Self::Double(d) => d.post_tan_slope,
            Self::Float(f) => f64::from(f.post_tan_slope),
        }
    }

    /// Sets the post-tangent slope from f64.
    pub fn set_post_tan_slope_f64(&mut self, slope: f64) {
        match self {
            Self::Double(d) => d.post_tan_slope = slope,
            Self::Float(f) => f.post_tan_slope = slope as f32,
        }
    }

    /// Returns a reference to double data if this is double type.
    #[inline]
    #[must_use]
    pub fn as_double(&self) -> Option<&TypedKnotData<f64>> {
        match self {
            Self::Double(d) => Some(d),
            _ => None,
        }
    }

    /// Returns a mutable reference to double data if this is double type.
    #[inline]
    #[must_use]
    pub fn as_double_mut(&mut self) -> Option<&mut TypedKnotData<f64>> {
        match self {
            Self::Double(d) => Some(d),
            _ => None,
        }
    }

    /// Returns a reference to float data if this is float type.
    #[inline]
    #[must_use]
    pub fn as_float(&self) -> Option<&TypedKnotData<f32>> {
        match self {
            Self::Float(f) => Some(f),
            _ => None,
        }
    }

    /// Returns a mutable reference to float data if this is float type.
    #[inline]
    #[must_use]
    pub fn as_float_mut(&mut self) -> Option<&mut TypedKnotData<f32>> {
        match self {
            Self::Float(f) => Some(f),
            _ => None,
        }
    }
}

impl Default for AnyKnotData {
    fn default() -> Self {
        Self::new_double()
    }
}

impl fmt::Display for AnyKnotData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Double(d) => write!(f, "{}", d),
            Self::Float(fl) => write!(f, "{}", fl),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knot_data_default() {
        let data = KnotData::new();
        assert_eq!(data.time, 0.0);
        assert_eq!(data.pre_tan_width, 0.0);
        assert_eq!(data.post_tan_width, 0.0);
        assert_eq!(data.next_interp, InterpMode::Held);
        assert!(!data.dual_valued);
    }

    #[test]
    fn test_knot_data_setters() {
        let mut data = KnotData::new();
        data.set_pre_tan_width(0.5);
        data.set_post_tan_width(0.75);
        assert_eq!(data.get_pre_tan_width(), 0.5);
        assert_eq!(data.get_post_tan_width(), 0.75);
    }

    #[test]
    fn test_typed_knot_data_default() {
        let data = TypedKnotData::<f64>::new();
        assert_eq!(data.value, 0.0);
        assert_eq!(data.pre_value, 0.0);
        assert_eq!(data.pre_tan_slope, 0.0);
        assert_eq!(data.post_tan_slope, 0.0);
    }

    #[test]
    fn test_typed_knot_data_pre_value() {
        let mut data = TypedKnotData::<f64>::new();
        data.value = 10.0;
        data.pre_value = 5.0;

        // Not dual-valued: returns value
        assert_eq!(data.get_pre_value(), 10.0);

        // Dual-valued: returns pre_value
        data.base.dual_valued = true;
        assert_eq!(data.get_pre_value(), 5.0);
    }

    #[test]
    fn test_typed_knot_data_tangent_heights() {
        let mut data = TypedKnotData::<f64>::new();
        data.base.pre_tan_width = 2.0;
        data.base.post_tan_width = 3.0;
        data.pre_tan_slope = 1.5;
        data.post_tan_slope = 2.0;

        // Pre-tangent height = -width * slope = -2.0 * 1.5 = -3.0
        assert!((data.get_pre_tan_height() - (-3.0)).abs() < 1e-10);

        // Post-tangent height = width * slope = 3.0 * 2.0 = 6.0
        assert!((data.get_post_tan_height() - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_knot_value_type() {
        assert_eq!(KnotValueType::Double.size_of(), 8);
        assert_eq!(KnotValueType::Float.size_of(), 4);
        assert_eq!(KnotValueType::Half.size_of(), 2);

        assert_eq!(format!("{}", KnotValueType::Double), "double");
        assert_eq!(format!("{}", KnotValueType::Float), "float");
    }

    #[test]
    fn test_any_knot_data_double() {
        let mut knot = AnyKnotData::new_double();
        assert_eq!(knot.value_type(), KnotValueType::Double);

        knot.set_time(1.5);
        assert_eq!(knot.time(), 1.5);

        knot.set_value_f64(42.0);
        assert_eq!(knot.value_as_f64(), 42.0);

        knot.set_next_interp(InterpMode::Curve);
        assert_eq!(knot.next_interp(), InterpMode::Curve);

        assert!(knot.as_double().is_some());
        assert!(knot.as_float().is_none());
    }

    #[test]
    fn test_any_knot_data_float() {
        let mut knot = AnyKnotData::new_float();
        assert_eq!(knot.value_type(), KnotValueType::Float);

        knot.set_value_f64(3.14);
        // Float precision loss expected
        assert!((knot.value_as_f64() - 3.14).abs() < 0.001);

        assert!(knot.as_float().is_some());
        assert!(knot.as_double().is_none());
    }

    #[test]
    fn test_any_knot_data_slopes() {
        let mut knot = AnyKnotData::new_double();
        knot.set_pre_tan_slope_f64(1.5);
        knot.set_post_tan_slope_f64(2.5);

        assert_eq!(knot.pre_tan_slope_as_f64(), 1.5);
        assert_eq!(knot.post_tan_slope_as_f64(), 2.5);
    }

    #[test]
    fn test_display() {
        let data = KnotData::new();
        let s = format!("{}", data);
        assert!(s.contains("KnotData"));

        let typed = TypedKnotData::<f64>::new();
        let s = format!("{}", typed);
        assert!(s.contains("TypedKnotData"));

        let any = AnyKnotData::new_double();
        let s = format!("{}", any);
        assert!(s.contains("TypedKnotData"));
    }

    #[test]
    fn test_equality() {
        let d1 = KnotData::new();
        let d2 = KnotData::new();
        assert_eq!(d1, d2);

        let mut d3 = KnotData::new();
        d3.time = 1.0;
        assert_ne!(d1, d3);
    }

    #[test]
    fn test_typed_equality() {
        let t1 = TypedKnotData::<f64>::new();
        let t2 = TypedKnotData::<f64>::new();
        assert_eq!(t1, t2);

        let mut t3 = TypedKnotData::<f64>::new();
        t3.value = 5.0;
        assert_ne!(t1, t3);
    }
}
