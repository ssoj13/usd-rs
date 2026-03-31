//! Timecode Spline types.
//!
//! This module provides fundamental types for the spline animation system.
//!
//! # Spline System Overview
//!
//! The TS (Timecode Spline) module implements spline-based animation curves
//! for USD. Key concepts:
//!
//! - **Time**: Values vary over time, using `TsTime` (f64)
//! - **Interpolation**: How values change between knots
//! - **Extrapolation**: How values extend beyond defined knots
//! - **Tangents**: Control curve shape at knots
//!
//! # Examples
//!
//! ```
//! use usd_ts::{InterpMode, ExtrapMode, CurveType};
//!
//! let interp = InterpMode::Curve;
//! let extrap = ExtrapMode::Held;
//! let curve = CurveType::Bezier;
//! ```

use std::fmt;

use usd_gf::Interval;

/// Time value type for splines.
///
/// Times in the spline system are encoded as f64 for precision.
pub type TsTime = f64;

/// Interpolation mode for a spline segment (region between two knots).
///
/// Determines how values are calculated between two consecutive knots.
///
/// # Examples
///
/// ```
/// use usd_ts::InterpMode;
///
/// let mode = InterpMode::Linear;
/// assert!(mode.has_value());
/// assert!(!mode.is_curve());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum InterpMode {
    /// No value in this segment (blocked).
    ValueBlock = 0,
    /// Constant value in this segment (step function).
    #[default]
    Held = 1,
    /// Linear interpolation between knots.
    Linear = 2,
    /// Bezier or Hermite curve interpolation (depends on curve type).
    Curve = 3,
}

impl InterpMode {
    /// Returns true if this mode produces a value.
    #[inline]
    #[must_use]
    pub fn has_value(&self) -> bool {
        !matches!(self, Self::ValueBlock)
    }

    /// Returns true if this mode uses curve interpolation.
    #[inline]
    #[must_use]
    pub fn is_curve(&self) -> bool {
        matches!(self, Self::Curve)
    }

    /// Returns true if this mode uses linear interpolation.
    #[inline]
    #[must_use]
    pub fn is_linear(&self) -> bool {
        matches!(self, Self::Linear)
    }

    /// Returns true if value is held constant (step function).
    #[inline]
    #[must_use]
    pub fn is_held(&self) -> bool {
        matches!(self, Self::Held)
    }
}

impl fmt::Display for InterpMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValueBlock => write!(f, "ValueBlock"),
            Self::Held => write!(f, "Held"),
            Self::Linear => write!(f, "Linear"),
            Self::Curve => write!(f, "Curve"),
        }
    }
}

/// Type of curve interpolation for spline segments.
///
/// When [`InterpMode::Curve`] is used, this determines the curve type.
///
/// # Examples
///
/// ```
/// use usd_ts::CurveType;
///
/// let curve = CurveType::Bezier;
/// assert!(curve.has_free_tangent_widths());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CurveType {
    /// Bezier curve with free tangent widths.
    #[default]
    Bezier = 0,
    /// Hermite curve (like Bezier but with fixed tangent widths).
    Hermite = 1,
}

impl CurveType {
    /// Returns true if tangent widths can be freely adjusted.
    #[inline]
    #[must_use]
    pub fn has_free_tangent_widths(&self) -> bool {
        matches!(self, Self::Bezier)
    }
}

impl fmt::Display for CurveType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bezier => write!(f, "Bezier"),
            Self::Hermite => write!(f, "Hermite"),
        }
    }
}

/// Extrapolation mode for regions beyond spline knots.
///
/// Controls how values are computed before the first knot and
/// after the last knot.
///
/// # Examples
///
/// ```
/// use usd_ts::ExtrapMode;
///
/// let mode = ExtrapMode::Linear;
/// assert!(mode.has_value());
/// assert!(!mode.is_looping());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExtrapMode {
    /// No value in this region (blocked).
    ValueBlock = 0,
    /// Constant value held from edge knot.
    #[default]
    Held = 1,
    /// Linear extrapolation based on edge knots.
    Linear = 2,
    /// Linear extrapolation with specified slope.
    Sloped = 3,
    /// Knot curve repeated, offset so ends meet.
    LoopRepeat = 4,
    /// Curve repeated exactly, discontinuous joins.
    LoopReset = 5,
    /// Like Reset, but every other copy reversed.
    LoopOscillate = 6,
}

impl ExtrapMode {
    /// Returns true if this mode produces a value.
    #[inline]
    #[must_use]
    pub fn has_value(&self) -> bool {
        !matches!(self, Self::ValueBlock)
    }

    /// Returns true if this is a looping extrapolation mode.
    #[inline]
    #[must_use]
    pub fn is_looping(&self) -> bool {
        matches!(
            self,
            Self::LoopRepeat | Self::LoopReset | Self::LoopOscillate
        )
    }

    /// Returns true if this mode uses linear extrapolation.
    #[inline]
    #[must_use]
    pub fn is_linear(&self) -> bool {
        matches!(self, Self::Linear | Self::Sloped)
    }
}

impl fmt::Display for ExtrapMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValueBlock => write!(f, "ValueBlock"),
            Self::Held => write!(f, "Held"),
            Self::Linear => write!(f, "Linear"),
            Self::Sloped => write!(f, "Sloped"),
            Self::LoopRepeat => write!(f, "LoopRepeat"),
            Self::LoopReset => write!(f, "LoopReset"),
            Self::LoopOscillate => write!(f, "LoopOscillate"),
        }
    }
}

/// Source for a sampled spline region.
///
/// When a spline is sampled for display, this indicates which
/// region of the spline each sample comes from.
///
/// # Examples
///
/// ```
/// use usd_ts::SplineSampleSource;
///
/// let source = SplineSampleSource::KnotInterp;
/// assert!(source.is_normal_interpolation());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SplineSampleSource {
    /// Extrapolation before the first knot.
    PreExtrap = 0,
    /// Looped extrapolation before the first knot.
    PreExtrapLoop = 1,
    /// Echoed copy of an inner loop prototype (before).
    InnerLoopPreEcho = 2,
    /// The inner loop prototype region.
    InnerLoopProto = 3,
    /// Echoed copy of an inner loop prototype (after).
    InnerLoopPostEcho = 4,
    /// Normal knot interpolation.
    #[default]
    KnotInterp = 5,
    /// Extrapolation after the last knot.
    PostExtrap = 6,
    /// Looped extrapolation after the last knot.
    PostExtrapLoop = 7,
}

impl SplineSampleSource {
    /// Returns true if this is normal knot interpolation.
    #[inline]
    #[must_use]
    pub fn is_normal_interpolation(&self) -> bool {
        matches!(self, Self::KnotInterp)
    }

    /// Returns true if this is an extrapolation region.
    #[inline]
    #[must_use]
    pub fn is_extrapolation(&self) -> bool {
        matches!(
            self,
            Self::PreExtrap | Self::PreExtrapLoop | Self::PostExtrap | Self::PostExtrapLoop
        )
    }

    /// Returns true if this is part of an inner loop.
    #[inline]
    #[must_use]
    pub fn is_inner_loop(&self) -> bool {
        matches!(
            self,
            Self::InnerLoopPreEcho | Self::InnerLoopProto | Self::InnerLoopPostEcho
        )
    }
}

impl fmt::Display for SplineSampleSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreExtrap => write!(f, "PreExtrap"),
            Self::PreExtrapLoop => write!(f, "PreExtrapLoop"),
            Self::InnerLoopPreEcho => write!(f, "InnerLoopPreEcho"),
            Self::InnerLoopProto => write!(f, "InnerLoopProto"),
            Self::InnerLoopPostEcho => write!(f, "InnerLoopPostEcho"),
            Self::KnotInterp => write!(f, "KnotInterp"),
            Self::PostExtrap => write!(f, "PostExtrap"),
            Self::PostExtrapLoop => write!(f, "PostExtrapLoop"),
        }
    }
}

/// Automatic tangent calculation algorithms.
///
/// Determines how tangents are computed at knots.
///
/// # Examples
///
/// ```
/// use usd_ts::TangentAlgorithm;
///
/// let algo = TangentAlgorithm::AutoEase;
/// assert!(algo.is_automatic());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TangentAlgorithm {
    /// Tangents are not automatically calculated; provided values are used.
    #[default]
    None = 0,
    /// Algorithm determined by custom data keys.
    Custom = 1,
    /// "Auto Ease" algorithm from Maya/animX.
    /// Computes a slope between slopes to neighboring knots.
    AutoEase = 2,
}

impl TangentAlgorithm {
    /// Returns true if tangents are computed automatically.
    #[inline]
    #[must_use]
    pub fn is_automatic(&self) -> bool {
        !matches!(self, Self::None)
    }
}

impl fmt::Display for TangentAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Custom => write!(f, "Custom"),
            Self::AutoEase => write!(f, "AutoEase"),
        }
    }
}

/// Modes for enforcing non-regression in splines.
///
/// Regression occurs when a spline "goes backwards" in time.
/// These modes control how regression is prevented.
///
/// # Examples
///
/// ```
/// use usd_ts::AntiRegressionMode;
///
/// let mode = AntiRegressionMode::KeepRatio;
/// assert!(mode.prevents_regression());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AntiRegressionMode {
    /// Do not enforce. Runtime evaluation uses KeepRatio if regression occurs.
    #[default]
    None = 0,
    /// Prevent tangents from crossing neighboring knots.
    /// Slightly over-conservative but guarantees non-regression.
    Contain = 1,
    /// Shorten both tangents while preserving their ratio.
    KeepRatio = 2,
    /// Leave start tangent alone, shorten end tangent only.
    /// Matches Maya behavior.
    KeepStart = 3,
}

impl AntiRegressionMode {
    /// Returns true if this mode actively prevents regression.
    #[inline]
    #[must_use]
    pub fn prevents_regression(&self) -> bool {
        !matches!(self, Self::None)
    }
}

impl fmt::Display for AntiRegressionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Contain => write!(f, "Contain"),
            Self::KeepRatio => write!(f, "KeepRatio"),
            Self::KeepStart => write!(f, "KeepStart"),
        }
    }
}

/// Inner-loop parameters for a spline.
///
/// At most one inner-loop region can be specified per spline.
/// Only whole numbers of pre- and post-iterations are supported.
///
/// # Examples
///
/// ```
/// use usd_ts::LoopParams;
///
/// let params = LoopParams::new(0.0, 10.0)
///     .with_pre_loops(2)
///     .with_post_loops(3)
///     .with_value_offset(100.0);
///
/// assert!(params.is_enabled());
/// assert_eq!(params.num_pre_loops, 2);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LoopParams {
    /// Start time of the prototype region.
    pub proto_start: TsTime,
    /// End time of the prototype region.
    pub proto_end: TsTime,
    /// Number of loop iterations before the prototype.
    pub num_pre_loops: i32,
    /// Number of loop iterations after the prototype.
    pub num_post_loops: i32,
    /// Value difference between consecutive iterations.
    pub value_offset: f64,
}

impl LoopParams {
    /// Creates new loop parameters with the given prototype range.
    #[inline]
    #[must_use]
    pub fn new(proto_start: TsTime, proto_end: TsTime) -> Self {
        Self {
            proto_start,
            proto_end,
            num_pre_loops: 0,
            num_post_loops: 0,
            value_offset: 0.0,
        }
    }

    /// Sets the number of pre-loops.
    #[inline]
    #[must_use]
    pub fn with_pre_loops(mut self, count: i32) -> Self {
        self.num_pre_loops = count;
        self
    }

    /// Sets the number of post-loops.
    #[inline]
    #[must_use]
    pub fn with_post_loops(mut self, count: i32) -> Self {
        self.num_post_loops = count;
        self
    }

    /// Sets the value offset between iterations.
    #[inline]
    #[must_use]
    pub fn with_value_offset(mut self, offset: f64) -> Self {
        self.value_offset = offset;
        self
    }

    /// Returns true if inner looping is enabled.
    ///
    /// Looping is disabled when proto_end <= proto_start.
    #[inline]
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.proto_end > self.proto_start
    }

    /// Returns the prototype region as an interval.
    #[must_use]
    pub fn prototype_interval(&self) -> Interval {
        if self.is_enabled() {
            Interval::new(self.proto_start, self.proto_end, true, false)
        } else {
            Interval::new_empty()
        }
    }

    /// Returns the total looped interval (prototype + all echoes).
    #[must_use]
    pub fn looped_interval(&self) -> Interval {
        if !self.is_enabled() {
            return Interval::new_empty();
        }

        let proto_len = self.proto_end - self.proto_start;
        let pre = self.num_pre_loops.max(0) as f64;
        let post = self.num_post_loops.max(0) as f64;

        let start = self.proto_start - pre * proto_len;
        let end = self.proto_end + post * proto_len;

        Interval::new(start, end, true, false)
    }
}

/// Extrapolation parameters for a spline endpoint.
///
/// # Examples
///
/// ```
/// use usd_ts::{Extrapolation, ExtrapMode};
///
/// let extrap = Extrapolation::new(ExtrapMode::Linear);
/// assert!(extrap.mode.has_value());
///
/// let sloped = Extrapolation::sloped(2.5);
/// assert_eq!(sloped.slope, 2.5);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Extrapolation {
    /// The extrapolation mode.
    pub mode: ExtrapMode,
    /// Slope for sloped extrapolation (only used when mode is Sloped).
    pub slope: f64,
}

impl Extrapolation {
    /// Creates extrapolation with the given mode.
    #[inline]
    #[must_use]
    pub fn new(mode: ExtrapMode) -> Self {
        Self { mode, slope: 0.0 }
    }

    /// Creates sloped extrapolation with the given slope.
    #[inline]
    #[must_use]
    pub fn sloped(slope: f64) -> Self {
        Self {
            mode: ExtrapMode::Sloped,
            slope,
        }
    }

    /// Creates held (constant) extrapolation.
    #[inline]
    #[must_use]
    pub fn held() -> Self {
        Self::new(ExtrapMode::Held)
    }

    /// Creates linear extrapolation.
    #[inline]
    #[must_use]
    pub fn linear() -> Self {
        Self::new(ExtrapMode::Linear)
    }

    /// Returns true if this is a looping extrapolation mode.
    #[inline]
    #[must_use]
    pub fn is_looping(&self) -> bool {
        self.mode.is_looping()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interp_mode() {
        assert!(InterpMode::Held.has_value());
        assert!(InterpMode::Linear.has_value());
        assert!(InterpMode::Curve.has_value());
        assert!(!InterpMode::ValueBlock.has_value());

        assert!(InterpMode::Curve.is_curve());
        assert!(InterpMode::Linear.is_linear());
        assert!(InterpMode::Held.is_held());
    }

    #[test]
    fn test_curve_type() {
        assert!(CurveType::Bezier.has_free_tangent_widths());
        assert!(!CurveType::Hermite.has_free_tangent_widths());
    }

    #[test]
    fn test_extrap_mode() {
        assert!(ExtrapMode::Held.has_value());
        assert!(!ExtrapMode::ValueBlock.has_value());

        assert!(ExtrapMode::LoopRepeat.is_looping());
        assert!(ExtrapMode::LoopReset.is_looping());
        assert!(ExtrapMode::LoopOscillate.is_looping());
        assert!(!ExtrapMode::Held.is_looping());

        assert!(ExtrapMode::Linear.is_linear());
        assert!(ExtrapMode::Sloped.is_linear());
    }

    #[test]
    fn test_sample_source() {
        assert!(SplineSampleSource::KnotInterp.is_normal_interpolation());
        assert!(SplineSampleSource::PreExtrap.is_extrapolation());
        assert!(SplineSampleSource::PostExtrap.is_extrapolation());
        assert!(SplineSampleSource::InnerLoopProto.is_inner_loop());
    }

    #[test]
    fn test_tangent_algorithm() {
        assert!(!TangentAlgorithm::None.is_automatic());
        assert!(TangentAlgorithm::Custom.is_automatic());
        assert!(TangentAlgorithm::AutoEase.is_automatic());
    }

    #[test]
    fn test_anti_regression_mode() {
        assert!(!AntiRegressionMode::None.prevents_regression());
        assert!(AntiRegressionMode::Contain.prevents_regression());
        assert!(AntiRegressionMode::KeepRatio.prevents_regression());
        assert!(AntiRegressionMode::KeepStart.prevents_regression());
    }

    #[test]
    fn test_loop_params() {
        let params = LoopParams::new(0.0, 10.0)
            .with_pre_loops(2)
            .with_post_loops(3)
            .with_value_offset(100.0);

        assert!(params.is_enabled());
        assert_eq!(params.proto_start, 0.0);
        assert_eq!(params.proto_end, 10.0);
        assert_eq!(params.num_pre_loops, 2);
        assert_eq!(params.num_post_loops, 3);
        assert_eq!(params.value_offset, 100.0);
    }

    #[test]
    fn test_loop_params_disabled() {
        let params = LoopParams::new(10.0, 0.0);
        assert!(!params.is_enabled());
        assert!(params.prototype_interval().is_empty());
    }

    #[test]
    fn test_loop_params_intervals() {
        let params = LoopParams::new(10.0, 20.0)
            .with_pre_loops(1)
            .with_post_loops(2);

        let proto = params.prototype_interval();
        assert_eq!(proto.get_min(), 10.0);
        assert_eq!(proto.get_max(), 20.0);

        let looped = params.looped_interval();
        assert_eq!(looped.get_min(), 0.0); // 10 - 1*10
        assert_eq!(looped.get_max(), 40.0); // 20 + 2*10
    }

    #[test]
    fn test_extrapolation() {
        let held = Extrapolation::held();
        assert_eq!(held.mode, ExtrapMode::Held);

        let linear = Extrapolation::linear();
        assert_eq!(linear.mode, ExtrapMode::Linear);

        let sloped = Extrapolation::sloped(2.5);
        assert_eq!(sloped.mode, ExtrapMode::Sloped);
        assert_eq!(sloped.slope, 2.5);

        let looping = Extrapolation::new(ExtrapMode::LoopRepeat);
        assert!(looping.is_looping());
    }

    #[test]
    fn test_defaults() {
        assert_eq!(InterpMode::default(), InterpMode::Held);
        assert_eq!(CurveType::default(), CurveType::Bezier);
        assert_eq!(ExtrapMode::default(), ExtrapMode::Held);
        assert_eq!(
            SplineSampleSource::default(),
            SplineSampleSource::KnotInterp
        );
        assert_eq!(TangentAlgorithm::default(), TangentAlgorithm::None);
        assert_eq!(AntiRegressionMode::default(), AntiRegressionMode::None);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", InterpMode::Curve), "Curve");
        assert_eq!(format!("{}", CurveType::Bezier), "Bezier");
        assert_eq!(format!("{}", ExtrapMode::LoopRepeat), "LoopRepeat");
        assert_eq!(format!("{}", SplineSampleSource::KnotInterp), "KnotInterp");
        assert_eq!(format!("{}", TangentAlgorithm::AutoEase), "AutoEase");
        assert_eq!(format!("{}", AntiRegressionMode::KeepRatio), "KeepRatio");
    }
}
