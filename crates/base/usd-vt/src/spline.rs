//! Spline types for animation curves.
//!
//! Port of pxr/base/ts/splineData.h/cpp and pxr/base/ts/eval.cpp
//!
//! Provides spline curve types for smooth animation interpolation with
//! tangent controls, extrapolation modes, and looping support.

use crate::Value;
use ordered_float::OrderedFloat;
use std::fmt;
use std::hash::{Hash, Hasher};

// Epsilon for parameter calculations in Bezier curves
const PARAMETER_EPSILON: f64 = 1.0e-10;

// ============================================================================
// Spline Curve Type
// ============================================================================

/// Spline curve type (bezier or hermite).
///
/// Matches C++ `TsSpline::Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SplineCurveType {
    /// Bezier interpolation (cubic Bezier curves).
    #[default]
    Bezier,
    /// Hermite interpolation (cubic Hermite splines).
    Hermite,
}

impl fmt::Display for SplineCurveType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SplineCurveType::Bezier => write!(f, "bezier"),
            SplineCurveType::Hermite => write!(f, "hermite"),
        }
    }
}

// ============================================================================
// Spline Extrapolation
// ============================================================================

/// Spline extrapolation behavior.
///
/// Matches C++ `TsSpline::Extrapolation`.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SplineExtrapolation {
    /// No value in this extrapolation region.
    ValueBlock,
    /// No extrapolation (constant value at boundaries).
    None,
    /// Hold the last value (constant extrapolation).
    #[default]
    Held,
    /// Linear extrapolation.
    Linear,
    /// Sloped extrapolation with a specified slope.
    Sloped(f64),
    /// Loop with repeat mode.
    LoopRepeat,
    /// Loop with reset mode.
    LoopReset,
    /// Loop with oscillate mode.
    LoopOscillate,
}

impl SplineExtrapolation {
    /// Returns true if this extrapolation mode involves looping.
    pub fn is_looping(&self) -> bool {
        matches!(
            self,
            SplineExtrapolation::LoopRepeat
                | SplineExtrapolation::LoopReset
                | SplineExtrapolation::LoopOscillate
        )
    }
}

impl fmt::Display for SplineExtrapolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SplineExtrapolation::ValueBlock => write!(f, "value_block"),
            SplineExtrapolation::None => write!(f, "none"),
            SplineExtrapolation::Held => write!(f, "held"),
            SplineExtrapolation::Linear => write!(f, "linear"),
            SplineExtrapolation::Sloped(slope) => write!(f, "sloped({})", slope),
            SplineExtrapolation::LoopRepeat => write!(f, "loop repeat"),
            SplineExtrapolation::LoopReset => write!(f, "loop reset"),
            SplineExtrapolation::LoopOscillate => write!(f, "loop oscillate"),
        }
    }
}

impl Hash for SplineExtrapolation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        if let SplineExtrapolation::Sloped(slope) = self {
            OrderedFloat(*slope).hash(state);
        }
    }
}

impl Eq for SplineExtrapolation {}

// ============================================================================
// Spline Loop Parameters
// ============================================================================

/// Parameters for spline looping behavior.
///
/// Matches C++ `TsSpline::LoopParams`.
#[derive(Debug, Clone, PartialEq)]
pub struct SplineLoopParams {
    /// Start of the prototype interval.
    pub proto_start: f64,
    /// End of the prototype interval.
    pub proto_end: f64,
    /// Number of loops before the prototype.
    pub num_pre_loops: i64,
    /// Number of loops after the prototype.
    pub num_post_loops: i64,
    /// Value offset per loop.
    pub value_offset: f64,
}

impl Default for SplineLoopParams {
    fn default() -> Self {
        Self {
            proto_start: 0.0,
            proto_end: 0.0,
            num_pre_loops: 0,
            num_post_loops: 0,
            value_offset: 0.0,
        }
    }
}

impl SplineLoopParams {
    /// Returns true if inner loops are valid and enabled.
    pub fn is_valid(&self) -> bool {
        self.proto_end > self.proto_start && (self.num_pre_loops > 0 || self.num_post_loops > 0)
    }

    /// Returns the prototype interval size.
    pub fn proto_span(&self) -> f64 {
        self.proto_end - self.proto_start
    }

    /// Returns the full looped interval (including pre and post loops).
    pub fn looped_interval(&self) -> (f64, f64) {
        let span = self.proto_span();
        let start = self.proto_start - (self.num_pre_loops as f64) * span;
        let end = self.proto_end + (self.num_post_loops as f64) * span;
        (start, end)
    }
}

impl Hash for SplineLoopParams {
    fn hash<H: Hasher>(&self, state: &mut H) {
        OrderedFloat(self.proto_start).hash(state);
        OrderedFloat(self.proto_end).hash(state);
        self.num_pre_loops.hash(state);
        self.num_post_loops.hash(state);
        OrderedFloat(self.value_offset).hash(state);
    }
}

impl Eq for SplineLoopParams {}

// ============================================================================
// Spline Tangent Algorithm
// ============================================================================

/// Spline tangent algorithm.
///
/// Matches C++ `TsSpline::TangentAlgorithm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SplineTangentAlgorithm {
    /// Custom tangent (explicitly specified).
    Custom,
    /// Auto-ease tangent (automatically computed).
    #[default]
    AutoEase,
}

// ============================================================================
// Spline Tangent
// ============================================================================

/// Spline tangent specification.
///
/// Matches C++ `TsSpline::Tangent`.
#[derive(Debug, Clone, PartialEq)]
pub struct SplineTangent {
    /// Tangent width (time delta).
    pub width: Option<f64>,
    /// Tangent slope.
    pub slope: f64,
    /// Tangent algorithm (custom or auto-ease).
    pub algorithm: Option<SplineTangentAlgorithm>,
}

impl Default for SplineTangent {
    fn default() -> Self {
        Self {
            width: None,
            slope: 0.0,
            algorithm: Some(SplineTangentAlgorithm::AutoEase),
        }
    }
}

impl SplineTangent {
    /// Returns the tangent height (value delta) for Bezier curves.
    pub fn height(&self, default_width: f64) -> f64 {
        let width = self.width.unwrap_or(default_width);
        self.slope * width
    }
}

impl Hash for SplineTangent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.width.map(OrderedFloat).hash(state);
        OrderedFloat(self.slope).hash(state);
        self.algorithm.hash(state);
    }
}

impl Eq for SplineTangent {}

// ============================================================================
// Spline Interpolation Mode
// ============================================================================

/// Spline interpolation mode for post-tangent.
///
/// Matches C++ `TsSpline::InterpMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SplineInterpMode {
    /// No value in this segment.
    ValueBlock,
    /// No interpolation (for parser compatibility).
    None,
    /// Linear interpolation.
    Linear,
    /// Curve interpolation (smooth).
    #[default]
    Curve,
    /// Held interpolation (step).
    Held,
}

impl fmt::Display for SplineInterpMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SplineInterpMode::ValueBlock => write!(f, "value_block"),
            SplineInterpMode::None => write!(f, "none"),
            SplineInterpMode::Linear => write!(f, "linear"),
            SplineInterpMode::Curve => write!(f, "curve"),
            SplineInterpMode::Held => write!(f, "held"),
        }
    }
}

// ============================================================================
// Spline Knot
// ============================================================================

/// A spline knot (keyframe).
///
/// Matches C++ `TsSpline::Knot`.
#[derive(Debug, Clone, PartialEq)]
pub struct SplineKnot {
    /// Time of the knot.
    pub time: f64,
    /// Value at the knot.
    pub value: f64,
    /// Pre-value for dual-valued knots (discontinuities).
    pub pre_value: Option<f64>,
    /// Pre-tangent.
    pub pre_tangent: Option<SplineTangent>,
    /// Post interpolation mode.
    pub post_interp: Option<SplineInterpMode>,
    /// Post-tangent.
    pub post_tangent: Option<SplineTangent>,
    /// Additional knot metadata (custom data).
    pub custom_data: Option<Value>,
}

impl SplineKnot {
    /// Creates a new spline knot.
    pub fn new(time: f64, value: f64) -> Self {
        Self {
            time,
            value,
            pre_value: None,
            pre_tangent: None,
            post_interp: None,
            post_tangent: None,
            custom_data: None,
        }
    }

    /// Creates a dual-valued knot (discontinuity).
    pub fn new_dual_valued(time: f64, pre_value: f64, value: f64) -> Self {
        Self {
            time,
            value,
            pre_value: Some(pre_value),
            pre_tangent: None,
            post_interp: None,
            post_tangent: None,
            custom_data: None,
        }
    }

    /// Returns the pre-value, or the regular value if not dual-valued.
    pub fn get_pre_value(&self) -> f64 {
        self.pre_value.unwrap_or(self.value)
    }

    /// Returns the post-tangent width, defaulting to 1/3 of segment width for Bezier.
    pub fn get_post_tan_width(&self, next_time: f64) -> f64 {
        self.post_tangent
            .as_ref()
            .and_then(|t| t.width)
            .unwrap_or_else(|| {
                // Default: 1/3 of segment width for Bezier
                (next_time - self.time) / 3.0
            })
    }

    /// Returns the pre-tangent width, defaulting to 1/3 of segment width for Bezier.
    pub fn get_pre_tan_width(&self, prev_time: f64) -> f64 {
        self.pre_tangent
            .as_ref()
            .and_then(|t| t.width)
            .unwrap_or_else(|| {
                // Default: 1/3 of segment width for Bezier
                (self.time - prev_time) / 3.0
            })
    }

    /// Returns the post-tangent slope.
    pub fn get_post_tan_slope(&self) -> f64 {
        self.post_tangent.as_ref().map(|t| t.slope).unwrap_or(0.0)
    }

    /// Returns the pre-tangent slope.
    pub fn get_pre_tan_slope(&self) -> f64 {
        self.pre_tangent.as_ref().map(|t| t.slope).unwrap_or(0.0)
    }

    /// Returns the interpolation mode of the segment following this knot.
    pub fn get_next_interpolation(&self) -> SplineInterpMode {
        self.post_interp.unwrap_or(SplineInterpMode::Curve)
    }

    /// Sets the interpolation mode of the segment following this knot.
    pub fn set_next_interpolation(&mut self, mode: SplineInterpMode) {
        self.post_interp = Some(mode);
    }
}

impl Hash for SplineKnot {
    fn hash<H: Hasher>(&self, state: &mut H) {
        OrderedFloat(self.time).hash(state);
        OrderedFloat(self.value).hash(state);
        self.pre_value.map(OrderedFloat).hash(state);
        self.pre_tangent.hash(state);
        self.post_interp.hash(state);
        self.post_tangent.hash(state);
        // Skip custom_data for hashing - metadata only
    }
}

impl Eq for SplineKnot {}

// ============================================================================
// Cubic Bezier Math Helpers
// ============================================================================

/// Coefficients for a cubic function (for Bezier curves).
struct Cubic {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
}

impl Cubic {
    /// Compute cubic coefficients from Bezier control points.
    /// The segment starts at p0, start tangent endpoint is p1,
    /// end tangent endpoint is p2, segment ends at p3.
    fn from_points(p0: f64, p1: f64, p2: f64, p3: f64) -> Self {
        Self {
            a: -p0 + 3.0 * p1 - 3.0 * p2 + p3,
            b: 3.0 * p0 - 6.0 * p1 + 3.0 * p2,
            c: -3.0 * p0 + 3.0 * p1,
            d: p0,
        }
    }

    /// Evaluate the cubic at parameter t.
    fn eval(&self, t: f64) -> f64 {
        t * (t * (t * self.a + self.b) + self.c) + self.d
    }

    /// Get the derivative (quadratic).
    #[allow(dead_code)] // C++ parity - used for slope calculations
    fn derivative(&self) -> Quadratic {
        Quadratic {
            a: 3.0 * self.a,
            b: 2.0 * self.b,
            c: self.c,
        }
    }
}

/// Coefficients for a quadratic function.
struct Quadratic {
    a: f64,
    b: f64,
    c: f64,
}

impl Quadratic {
    /// Evaluate the quadratic at parameter t.
    #[allow(dead_code)] // C++ parity - used for derivative evaluation
    fn eval(&self, t: f64) -> f64 {
        t * (t * self.a + self.b) + self.c
    }
}

/// Find the unique zero of a monotonic cubic function in [0, 1] using Cardano's algorithm.
fn find_monotonic_zero_cubic(cubic: &Cubic) -> f64 {
    const EPSILON: f64 = 1.0e-10;

    // Check for coefficients near zero
    let a_zero = cubic.a.abs() < EPSILON;
    let b_zero = cubic.b.abs() < EPSILON;
    let _c_zero = cubic.c.abs() < EPSILON;

    // Check for linearity
    if a_zero && b_zero {
        return -cubic.d / cubic.c;
    }

    // Check for quadraticity
    if a_zero {
        return find_monotonic_zero_quadratic(&Quadratic {
            a: cubic.b,
            b: cubic.c,
            c: cubic.d,
        });
    }

    // Full cubic solution using Cardano's method
    // Scale to force t^3 coefficient to 1
    let b = cubic.b / cubic.a;
    let c = cubic.c / cubic.a;
    let d = cubic.d / cubic.a;

    let p = (3.0 * c - b * b) / 3.0;
    let p3 = p / 3.0;
    let p33 = p3 * p3 * p3;
    let q = (2.0 * b * b * b - 9.0 * b * c + 27.0 * d) / 27.0;
    let q2 = q / 2.0;
    let discrim = q2 * q2 + p33;
    let b3 = b / 3.0;

    if discrim < 0.0 {
        // Three real roots
        let r = (-p33).sqrt();
        let t_val = -q / (2.0 * r);
        let phi = t_val.clamp(-1.0, 1.0).acos();
        let t1 = 2.0 * r.cbrt();
        let root1 = t1 * (phi / 3.0).cos() - b3;
        let root2 = t1 * ((phi + 2.0 * std::f64::consts::PI) / 3.0).cos() - b3;
        let root3 = t1 * ((phi + 4.0 * std::f64::consts::PI) / 3.0).cos() - b3;
        filter_zeros_3(root1, root2, root3)
    } else if discrim == 0.0 {
        // Two real roots
        let u1 = -q2.cbrt();
        let root1 = 2.0 * u1 - b3;
        let root2 = -u1 - b3;
        filter_zeros_2(root1, root2)
    } else {
        // One real root
        let sd = discrim.sqrt();
        let u1 = (sd - q2).cbrt();
        let v1 = (sd + q2).cbrt();
        u1 - v1 - b3
    }
}

/// Find the unique zero of a monotonic quadratic function in [0, 1].
fn find_monotonic_zero_quadratic(quad: &Quadratic) -> f64 {
    let discrim = (quad.b * quad.b - 4.0 * quad.a * quad.c).sqrt();
    let root0 = (-quad.b - discrim) / (2.0 * quad.a);
    let root1 = (-quad.b + discrim) / (2.0 * quad.a);
    filter_zeros_2(root0, root1)
}

/// Filter zeros to find the one closest to 0.5 in [0, 1].
fn filter_zeros_2(z0: f64, z1: f64) -> f64 {
    let mut result = z0;
    let mut min_error = (z0 - 0.5).abs();
    let error = (z1 - 0.5).abs();

    if error < min_error {
        result = z1;
        min_error = error;
    }

    if min_error > 0.5 + PARAMETER_EPSILON {
        // Warning would be emitted in C++ version
    }

    result
}

/// Filter zeros (three roots case).
fn filter_zeros_3(z0: f64, z1: f64, z2: f64) -> f64 {
    let mut result = z0;
    let mut min_error = (z0 - 0.5).abs();

    for z in [z1, z2] {
        let error = (z - 0.5).abs();
        if error < min_error {
            result = z;
            min_error = error;
        }
    }

    if min_error > 0.5 + PARAMETER_EPSILON {
        // Warning would be emitted in C++ version
    }

    result
}

// ============================================================================
// Loop Resolver
// ============================================================================

/// Resolves loop time shifts before evaluation.
///
/// Matches C++ `_LoopResolver` class from eval.cpp.
/// Handles both inner loops and extrapolating loops, computing final eval time
/// and value offset ONCE before the main evaluation.
struct LoopResolver {
    /// The resolved evaluation time (after all loop shifts).
    eval_time: f64,
    /// Value offset to add after evaluation.
    value_offset: f64,
    /// Whether to negate the result (for oscillating derivatives).
    negate: bool,
}

impl LoopResolver {
    /// Creates a new loop resolver and computes all loop shifts.
    ///
    /// Matches C++ `_LoopResolver` constructor.
    fn new(spline: &SplineValue, time: f64) -> Self {
        let mut resolver = Self {
            eval_time: time,
            value_offset: 0.0,
            negate: false,
        };

        if spline.knots.is_empty() {
            return resolver;
        }

        let first_time = spline.knots[0].time;
        let last_time = spline.knots[spline.knots.len() - 1].time;
        let have_multiple_knots = spline.knots.len() > 1;

        // Check for inner loops
        let have_inner_loops = spline
            .loop_params
            .as_ref()
            .map(|lp| lp.is_valid())
            .unwrap_or(false);

        // Check for extrapolating loops
        let have_pre_extrap_loops = have_multiple_knots && spline.pre_extrap.is_looping();
        let have_post_extrap_loops = have_multiple_knots && spline.post_extrap.is_looping();

        // Nothing to do if no loops
        if !have_inner_loops && !have_pre_extrap_loops && !have_post_extrap_loops {
            return resolver;
        }

        // Resolve extrapolating loops first (if any), then inner loops.
        // This reverses the procedure of knot copying.
        if have_pre_extrap_loops || have_post_extrap_loops {
            resolver.resolve_extrap(spline, first_time, last_time);
        }

        if have_inner_loops {
            resolver.resolve_inner(spline);
        }

        resolver
    }

    /// Resolves extrapolating loops.
    fn resolve_extrap(&mut self, spline: &SplineValue, first_time: f64, last_time: f64) {
        let span = last_time - first_time;
        if span <= 0.0 {
            return;
        }

        // Pre-extrapolation loop
        if self.eval_time < first_time && spline.pre_extrap.is_looping() {
            let offset = first_time - self.eval_time;
            let num_iters = (offset / span).ceil() as i64;

            // Hop forward into the non-extrapolating region
            self.eval_time += num_iters as f64 * span;

            // Handle repeat mode value offset
            if matches!(spline.pre_extrap, SplineExtrapolation::LoopRepeat) {
                let value_diff = spline.knots[spline.knots.len() - 1].value - spline.knots[0].value;
                self.value_offset -= num_iters as f64 * value_diff;
            }

            // Handle oscillate mode
            if matches!(spline.pre_extrap, SplineExtrapolation::LoopOscillate) && num_iters % 2 != 0
            {
                self.eval_time = first_time + (span - (self.eval_time - first_time));
                self.negate = true;
            }
        }

        // Post-extrapolation loop
        if self.eval_time > last_time && spline.post_extrap.is_looping() {
            let offset = self.eval_time - last_time;
            let num_iters = (offset / span).ceil() as i64;

            // Hop backward into the non-extrapolating region
            self.eval_time -= num_iters as f64 * span;

            // Handle repeat mode value offset
            if matches!(spline.post_extrap, SplineExtrapolation::LoopRepeat) {
                let value_diff = spline.knots[spline.knots.len() - 1].value - spline.knots[0].value;
                self.value_offset += num_iters as f64 * value_diff;
            }

            // Handle oscillate mode
            if matches!(spline.post_extrap, SplineExtrapolation::LoopOscillate)
                && num_iters % 2 != 0
            {
                self.eval_time = first_time + (span - (self.eval_time - first_time));
                self.negate = true;
            }
        }
    }

    /// Resolves inner loops.
    fn resolve_inner(&mut self, spline: &SplineValue) {
        let lp = match &spline.loop_params {
            Some(lp) if lp.is_valid() => lp,
            _ => return,
        };

        let (looped_start, looped_end) = lp.looped_interval();
        let proto_span = lp.proto_span();

        // Check if we're in the looped region but not in prototype
        if self.eval_time < looped_start || self.eval_time > looped_end {
            return;
        }

        // Already in prototype region - no shift needed
        if self.eval_time >= lp.proto_start && self.eval_time <= lp.proto_end {
            return;
        }

        // Handle pre-echo
        if self.eval_time < lp.proto_start {
            let loop_offset = lp.proto_start - self.eval_time;
            let iter_num = (loop_offset / proto_span).ceil() as i64;

            // Hop forward to the prototype region
            self.eval_time += iter_num as f64 * proto_span;

            // Adjust for value offset
            self.value_offset -= iter_num as f64 * lp.value_offset;
        }
        // Handle post-echo
        else {
            let loop_offset = self.eval_time - lp.proto_end;
            let iter_num = (loop_offset / proto_span).floor() as i64 + 1;

            // Hop backward to the prototype region
            self.eval_time -= iter_num as f64 * proto_span;

            // Adjust for value offset
            self.value_offset += iter_num as f64 * lp.value_offset;
        }
    }
}

// ============================================================================
// Spline Value
// ============================================================================

/// A complete spline value for animation curves.
///
/// Matches C++ `TsSpline`.
///
/// Splines provide smooth interpolation for animation with tangent controls,
/// extrapolation modes, and looping support.
///
/// # Examples
///
/// ```rust
/// use usd_vt::spline::{SplineValue, SplineKnot, SplineCurveType, SplineExtrapolation};
///
/// let mut spline = SplineValue::new(SplineCurveType::Bezier);
/// spline.set_pre_extrapolation(SplineExtrapolation::Held);
/// spline.set_post_extrapolation(SplineExtrapolation::Linear);
/// spline.add_knot(SplineKnot::new(0.0, 0.0));
/// spline.add_knot(SplineKnot::new(1.0, 1.0));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SplineValue {
    /// Curve type (bezier or hermite).
    pub curve_type: SplineCurveType,
    /// Pre-extrapolation mode.
    pub pre_extrap: SplineExtrapolation,
    /// Post-extrapolation mode.
    pub post_extrap: SplineExtrapolation,
    /// Loop parameters.
    pub loop_params: Option<SplineLoopParams>,
    /// Spline knots (sorted by time).
    pub knots: Vec<SplineKnot>,
}

impl SplineValue {
    /// Creates a new spline with the given curve type.
    pub fn new(curve_type: SplineCurveType) -> Self {
        Self {
            curve_type,
            pre_extrap: SplineExtrapolation::default(),
            post_extrap: SplineExtrapolation::default(),
            loop_params: None,
            knots: Vec::new(),
        }
    }

    /// Returns the curve type.
    pub fn curve_type(&self) -> SplineCurveType {
        self.curve_type
    }

    /// Sets the curve type.
    pub fn set_curve_type(&mut self, curve_type: SplineCurveType) {
        self.curve_type = curve_type;
    }

    /// Returns the pre-extrapolation mode.
    pub fn pre_extrapolation(&self) -> &SplineExtrapolation {
        &self.pre_extrap
    }

    /// Sets the pre-extrapolation mode.
    pub fn set_pre_extrapolation(&mut self, extrap: SplineExtrapolation) {
        self.pre_extrap = extrap;
    }

    /// Returns the post-extrapolation mode.
    pub fn post_extrapolation(&self) -> &SplineExtrapolation {
        &self.post_extrap
    }

    /// Sets the post-extrapolation mode.
    pub fn set_post_extrapolation(&mut self, extrap: SplineExtrapolation) {
        self.post_extrap = extrap;
    }

    /// Returns the loop parameters.
    pub fn loop_params(&self) -> Option<&SplineLoopParams> {
        self.loop_params.as_ref()
    }

    /// Sets the loop parameters.
    pub fn set_loop_params(&mut self, params: Option<SplineLoopParams>) {
        self.loop_params = params;
    }

    /// Sets the curve type (for parser compatibility).
    pub fn set_curve_type_value(&mut self, curve_type: SplineCurveType) {
        self.curve_type = curve_type;
    }

    /// Sets the pre-extrapolation (for parser compatibility).
    pub fn set_pre_extrap(&mut self, extrap: SplineExtrapolation) {
        self.pre_extrap = extrap;
    }

    /// Sets the post-extrapolation (for parser compatibility).
    pub fn set_post_extrap(&mut self, extrap: SplineExtrapolation) {
        self.post_extrap = extrap;
    }

    /// Adds a knot (for parser compatibility).
    pub fn add_knot_parsed(&mut self, knot: SplineKnot) {
        self.add_knot(knot);
    }

    /// Returns true if inner loops are valid and enabled.
    pub fn has_inner_loops(&self) -> bool {
        self.loop_params
            .as_ref()
            .map(|lp| lp.is_valid())
            .unwrap_or(false)
            && !self.knots.is_empty()
    }

    /// Returns the number of knots.
    pub fn num_knots(&self) -> usize {
        self.knots.len()
    }

    /// Returns true if the spline has no knots.
    pub fn is_empty(&self) -> bool {
        self.knots.is_empty()
    }

    /// Returns all knots.
    pub fn knots(&self) -> &[SplineKnot] {
        &self.knots
    }

    /// Returns a mutable reference to all knots.
    pub fn knots_mut(&mut self) -> &mut Vec<SplineKnot> {
        &mut self.knots
    }

    /// Returns the knot at the given time, if present.
    pub fn get_knot(&self, time: f64) -> Option<SplineKnot> {
        self.knots
            .iter()
            .find(|k| (k.time - time).abs() < f64::EPSILON)
            .cloned()
    }

    /// Adds a knot to the spline.
    ///
    /// The knot is inserted in sorted order by time.
    pub fn add_knot(&mut self, knot: SplineKnot) {
        // Find insertion point to maintain sorted order
        let pos = self
            .knots
            .binary_search_by(|k| {
                k.time
                    .partial_cmp(&knot.time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or_else(|pos| pos);
        self.knots.insert(pos, knot);
    }

    /// Inserts or replaces a knot at the given time.
    pub fn set_knot(&mut self, knot: SplineKnot) {
        if let Some(existing) = self
            .knots
            .iter_mut()
            .find(|existing| (existing.time - knot.time).abs() < f64::EPSILON)
        {
            *existing = knot;
            return;
        }
        self.add_knot(knot);
    }

    /// Removes the knot at the given index.
    pub fn remove_knot(&mut self, index: usize) -> Option<SplineKnot> {
        if index < self.knots.len() {
            Some(self.knots.remove(index))
        } else {
            None
        }
    }

    /// Clears all knots.
    pub fn clear_knots(&mut self) {
        self.knots.clear();
    }

    /// Evaluates the spline at the given time.
    ///
    /// Returns the interpolated value at the specified time.
    /// For times outside the knot range, extrapolation is used.
    ///
    /// Matches C++ `Ts_Eval()`.
    pub fn evaluate(&self, time: f64) -> Option<f64> {
        if self.knots.is_empty() {
            return None;
        }

        // Resolve all loops (inner and extrapolating) ONCE before evaluation.
        // This matches C++ _LoopResolver pattern.
        let resolver = LoopResolver::new(self, time);

        // Perform the main evaluation at the resolved time
        let result = self.eval_main(resolver.eval_time)?;

        // Apply value offset and negation
        let final_result =
            (result + resolver.value_offset) * if resolver.negate { -1.0 } else { 1.0 };

        Some(final_result)
    }

    /// Evaluates the spline immediately before the given time.
    ///
    /// Matches the left-hand-limit behavior of C++ `TsSpline::EvalPreValue()`.
    pub fn evaluate_pre(&self, time: f64) -> Option<f64> {
        if self.knots.is_empty() {
            return None;
        }

        if let Some(idx) = self
            .knots
            .iter()
            .position(|k| (k.time - time).abs() < f64::EPSILON)
        {
            if idx == 0 {
                return self.eval_pre_extrap(time, self.knots[0].time);
            }

            let prev = &self.knots[idx - 1];
            return match prev.get_next_interpolation() {
                SplineInterpMode::ValueBlock => None,
                SplineInterpMode::Held => Some(prev.value),
                SplineInterpMode::None | SplineInterpMode::Linear | SplineInterpMode::Curve => {
                    Some(self.knots[idx].get_pre_value())
                }
            };
        }

        self.evaluate(time)
    }

    /// Main evaluation function after loop resolution.
    ///
    /// Matches C++ `_EvalMain()`.
    fn eval_main(&self, time: f64) -> Option<f64> {
        if self.knots.is_empty() {
            return None;
        }

        let first_time = self.knots[0].time;
        let last_time = self.knots[self.knots.len() - 1].time;

        // Pre-extrapolation (non-looping - loops already resolved)
        if time < first_time {
            return self.eval_pre_extrap(time, first_time);
        }

        // Post-extrapolation (non-looping - loops already resolved)
        if time > last_time {
            return self.eval_post_extrap(time, last_time);
        }

        // Time exactly matches a knot
        for knot in &self.knots {
            if (knot.time - time).abs() < f64::EPSILON {
                return match knot.get_next_interpolation() {
                    SplineInterpMode::ValueBlock => None,
                    _ => Some(knot.value),
                };
            }
        }

        // Find the knot interval containing time.
        // Treat intervals as half-open [k0, k1) so exact upper-knot queries
        // use the knot's own value instead of the previous segment's mode.
        for i in 0..self.knots.len() - 1 {
            let k0 = &self.knots[i];
            let k1 = &self.knots[i + 1];

            if time > k0.time && time < k1.time {
                return self.interpolate(k0, k1, time);
            }
        }

        None
    }

    /// Evaluates pre-extrapolation (non-looping modes only).
    fn eval_pre_extrap(&self, time: f64, first_time: f64) -> Option<f64> {
        let first_knot = &self.knots[0];

        match &self.pre_extrap {
            SplineExtrapolation::ValueBlock => None,
            SplineExtrapolation::None | SplineExtrapolation::Held => {
                Some(first_knot.get_pre_value())
            }
            SplineExtrapolation::Linear => {
                if self.knots.len() < 2 {
                    Some(first_knot.get_pre_value())
                } else {
                    let second_knot = &self.knots[1];
                    let slope = if second_knot.time != first_knot.time {
                        (second_knot.value - first_knot.value)
                            / (second_knot.time - first_knot.time)
                    } else {
                        0.0
                    };
                    Some(first_knot.get_pre_value() + slope * (time - first_time))
                }
            }
            SplineExtrapolation::Sloped(slope) => {
                Some(first_knot.get_pre_value() + slope * (time - first_time))
            }
            // Loop modes should have been resolved by LoopResolver
            SplineExtrapolation::LoopRepeat
            | SplineExtrapolation::LoopReset
            | SplineExtrapolation::LoopOscillate => {
                // Fallback to held if loop resolution failed
                Some(first_knot.get_pre_value())
            }
        }
    }

    /// Evaluates post-extrapolation (non-looping modes only).
    fn eval_post_extrap(&self, time: f64, last_time: f64) -> Option<f64> {
        let last_knot = &self.knots[self.knots.len() - 1];

        match &self.post_extrap {
            SplineExtrapolation::ValueBlock => None,
            SplineExtrapolation::None | SplineExtrapolation::Held => Some(last_knot.value),
            SplineExtrapolation::Linear => {
                if self.knots.len() < 2 {
                    Some(last_knot.value)
                } else {
                    let second_last_knot = &self.knots[self.knots.len() - 2];
                    let slope = if last_knot.time != second_last_knot.time {
                        (last_knot.value - second_last_knot.value)
                            / (last_knot.time - second_last_knot.time)
                    } else {
                        0.0
                    };
                    Some(last_knot.value + slope * (time - last_time))
                }
            }
            SplineExtrapolation::Sloped(slope) => {
                Some(last_knot.value + slope * (time - last_time))
            }
            // Loop modes should have been resolved by LoopResolver
            SplineExtrapolation::LoopRepeat
            | SplineExtrapolation::LoopReset
            | SplineExtrapolation::LoopOscillate => {
                // Fallback to held if loop resolution failed
                Some(last_knot.value)
            }
        }
    }

    /// Interpolates between two knots.
    fn interpolate(&self, k0: &SplineKnot, k1: &SplineKnot, time: f64) -> Option<f64> {
        // Check interpolation mode
        let interp_mode = k0.get_next_interpolation();
        match interp_mode {
            SplineInterpMode::ValueBlock => None,
            SplineInterpMode::None | SplineInterpMode::Held => Some(k0.value),
            SplineInterpMode::Linear => {
                let t = (time - k0.time) / (k1.time - k0.time);
                Some(k0.value + (k1.get_pre_value() - k0.value) * t)
            }
            SplineInterpMode::Curve => match self.curve_type {
                SplineCurveType::Bezier => Some(self.interpolate_bezier(k0, k1, time)),
                SplineCurveType::Hermite => Some(self.interpolate_hermite(k0, k1, time)),
            },
        }
    }

    /// Interpolates using Bezier curves with full tangent support.
    fn interpolate_bezier(&self, k0: &SplineKnot, k1: &SplineKnot, time: f64) -> f64 {
        // Get tangent widths (default to 1/3 of segment width)
        let _segment_width = k1.time - k0.time;
        let post_width = k0.get_post_tan_width(k1.time);
        let pre_width = k1.get_pre_tan_width(k0.time);

        // Find Bezier parameter t using Cardano's algorithm
        let time_cubic = Cubic::from_points(
            k0.time - time,
            k0.time + post_width - time,
            k1.time - pre_width - time,
            k1.time - time,
        );

        let mut t = find_monotonic_zero_cubic(&time_cubic);
        if t < 0.0 {
            t = 0.0;
        } else if t > 1.0 {
            t = 1.0;
        }

        // Get tangent slopes and compute heights
        let post_slope = k0.get_post_tan_slope();
        let pre_slope = k1.get_pre_tan_slope();
        let post_height = post_slope * post_width;
        let pre_height = pre_slope * pre_width;

        // Compute value cubic from Bezier control points
        let value_cubic = Cubic::from_points(
            k0.value,
            k0.value + post_height,
            k1.get_pre_value() + pre_height,
            k1.get_pre_value(),
        );

        value_cubic.eval(t)
    }

    /// Interpolates using Hermite splines with full tangent support.
    fn interpolate_hermite(&self, k0: &SplineKnot, k1: &SplineKnot, time: f64) -> f64 {
        let t0 = k0.time;
        let v0 = k0.value;
        let m0 = k0.get_post_tan_slope();

        let t1 = k1.time;
        let v1 = k1.get_pre_value();
        let m1 = k1.get_pre_tan_slope();

        if t1 <= t0 {
            return v0;
        }

        let dt = t1 - t0;
        let dv = v1 - v0;

        // Convert time into [0..1] range and adjust slopes
        let u = (time - t0) / dt;
        let um0 = m0 * dt;
        let um1 = m1 * dt;

        // Calculate Hermite coefficients
        let a = -2.0 * dv + um0 + um1;
        let b = 3.0 * dv - 2.0 * um0 - um1;
        let c = um0;
        let d = v0;

        // Evaluate Hermite polynomial: v(u) = a*u^3 + b*u^2 + c*u + d
        u * (u * (u * a + b) + c) + d
    }
}

impl Hash for SplineValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.curve_type.hash(state);
        self.pre_extrap.hash(state);
        self.post_extrap.hash(state);
        self.loop_params.hash(state);
        self.knots.hash(state);
    }
}

impl Eq for SplineValue {}

impl Default for SplineValue {
    fn default() -> Self {
        Self::new(SplineCurveType::Bezier)
    }
}

impl fmt::Display for SplineValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Spline({}, {} knots)", self.curve_type, self.knots.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spline_creation() {
        let spline = SplineValue::new(SplineCurveType::Bezier);
        assert_eq!(spline.curve_type(), SplineCurveType::Bezier);
        assert!(spline.is_empty());
    }

    #[test]
    fn test_spline_add_knots() {
        let mut spline = SplineValue::new(SplineCurveType::Bezier);
        spline.add_knot(SplineKnot::new(1.0, 10.0));
        spline.add_knot(SplineKnot::new(0.0, 0.0));
        spline.add_knot(SplineKnot::new(2.0, 20.0));

        assert_eq!(spline.num_knots(), 3);
        assert_eq!(spline.knots()[0].time, 0.0);
        assert_eq!(spline.knots()[1].time, 1.0);
        assert_eq!(spline.knots()[2].time, 2.0);
    }

    #[test]
    fn test_spline_evaluate_bezier() {
        let mut spline = SplineValue::new(SplineCurveType::Bezier);
        spline.add_knot(SplineKnot::new(0.0, 0.0));
        spline.add_knot(SplineKnot::new(1.0, 1.0));

        assert_eq!(spline.evaluate(0.0), Some(0.0));
        assert_eq!(spline.evaluate(1.0), Some(1.0));
        // Midpoint should be close to 0.5 (may vary due to Bezier curve)
        let mid = spline.evaluate(0.5).unwrap();
        assert!(mid > 0.0 && mid < 1.0);
    }

    #[test]
    fn test_spline_evaluate_hermite() {
        let mut spline = SplineValue::new(SplineCurveType::Hermite);
        spline.add_knot(SplineKnot::new(0.0, 0.0));
        spline.add_knot(SplineKnot::new(1.0, 1.0));

        assert_eq!(spline.evaluate(0.0), Some(0.0));
        assert_eq!(spline.evaluate(1.0), Some(1.0));
    }

    #[test]
    fn test_spline_extrapolation() {
        let mut spline = SplineValue::new(SplineCurveType::Bezier);
        spline.set_pre_extrapolation(SplineExtrapolation::Held);
        spline.set_post_extrapolation(SplineExtrapolation::Linear);
        spline.add_knot(SplineKnot::new(0.0, 0.0));
        spline.add_knot(SplineKnot::new(1.0, 1.0));

        // Pre-extrapolation (held)
        assert_eq!(spline.evaluate(-1.0), Some(0.0));

        // Post-extrapolation (linear)
        let result = spline.evaluate(2.0);
        assert!(result.is_some());
        assert!((result.unwrap() - 2.0).abs() < 0.1);
    }

    #[test]
    fn test_spline_inner_loops() {
        let mut spline = SplineValue::new(SplineCurveType::Bezier);
        spline.add_knot(SplineKnot::new(0.0, 0.0));
        spline.add_knot(SplineKnot::new(1.0, 1.0));
        spline.set_loop_params(Some(SplineLoopParams {
            proto_start: 0.0,
            proto_end: 1.0,
            num_pre_loops: 1,
            num_post_loops: 1,
            value_offset: 0.0,
        }));

        assert!(spline.has_inner_loops());
        // Evaluate in pre-loop region
        let result = spline.evaluate(-0.5);
        assert!(result.is_some());
    }

    #[test]
    fn test_spline_loop_extrapolation() {
        let mut spline = SplineValue::new(SplineCurveType::Bezier);
        spline.set_post_extrapolation(SplineExtrapolation::LoopRepeat);
        spline.add_knot(SplineKnot::new(0.0, 0.0));
        spline.add_knot(SplineKnot::new(1.0, 1.0));

        // Evaluate beyond end - should loop
        let result = spline.evaluate(2.5);
        assert!(result.is_some());
    }

    #[test]
    fn test_spline_value_block_extrapolation() {
        let mut spline = SplineValue::new(SplineCurveType::Bezier);
        spline.set_pre_extrapolation(SplineExtrapolation::ValueBlock);
        spline.set_post_extrapolation(SplineExtrapolation::ValueBlock);
        spline.add_knot(SplineKnot::new(0.0, 0.0));
        spline.add_knot(SplineKnot::new(1.0, 1.0));

        assert_eq!(spline.evaluate(-1.0), None);
        assert_eq!(spline.evaluate(2.0), None);
    }

    #[test]
    fn test_spline_value_block_segment() {
        let mut spline = SplineValue::new(SplineCurveType::Bezier);
        let mut k0 = SplineKnot::new(0.0, 0.0);
        k0.set_next_interpolation(SplineInterpMode::ValueBlock);
        spline.set_knot(k0);
        spline.set_knot(SplineKnot::new(1.0, 1.0));

        assert_eq!(spline.evaluate(0.0), None);
        assert_eq!(spline.evaluate(0.5), None);
        assert_eq!(spline.evaluate(1.0), Some(1.0));
    }

    #[test]
    fn test_spline_get_and_set_knot() {
        let mut spline = SplineValue::new(SplineCurveType::Bezier);
        spline.set_knot(SplineKnot::new(1.0, 1.0));

        let mut knot = spline.get_knot(1.0).expect("knot at 1.0");
        knot.value = 3.0;
        knot.set_next_interpolation(SplineInterpMode::Held);
        spline.set_knot(knot);

        let got = spline.get_knot(1.0).expect("updated knot");
        assert_eq!(got.value, 3.0);
        assert_eq!(got.get_next_interpolation(), SplineInterpMode::Held);
    }
}
