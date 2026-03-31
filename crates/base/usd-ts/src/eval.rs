//! Spline evaluation and breakdown.
//!
//! This module provides types and functions for evaluating splines and
//! performing breakdown (splitting segments at specific times).
//!
//! # Overview
//!
//! - [`EvalAspect`]: What to evaluate (value, held value, derivative)
//! - [`EvalLocation`]: Where to evaluate (pre, at time, post)
//! - [`SplineSamples`]: Sampled spline as polylines
//! - [`SplineSamplesWithSources`]: Sampled spline with source information
//! - [`eval`]: Main evaluation function
//! - [`breakdown`]: Split a segment at a specific time
//!
//! # Bezier/Hermite Math
//!
//! Bezier curves are cubic polynomials parameterized by t in [0,1].
//! Hermite curves use endpoint values and slopes.
//! Both use Cardano's algorithm for root finding.

use std::f64::consts::PI;
use std::fmt;

use usd_gf::{Interval, Vec2d, Vec2f};

use super::knot_data::TypedKnotData;
use super::regression_preventer::RegressionPreventerBatch;
use super::spline_data::SplineData;
use super::types::{
    AntiRegressionMode, CurveType, ExtrapMode, Extrapolation, InterpMode, SplineSampleSource,
    TsTime,
};

/// Epsilon for Bezier parameter calculations (unitless, [0..1] range).
const PARAMETER_EPSILON: f64 = 1.0e-10;

//=============================================================================
// BEZIER MATH
//=============================================================================

/// Quadratic polynomial coefficients: f(t) = at^2 + bt + c.
#[derive(Clone, Copy, Debug, Default)]
struct Quadratic {
    a: f64,
    b: f64,
    c: f64,
}

impl Quadratic {
    /// Evaluate at parameter t.
    #[inline]
    fn eval(&self, t: f64) -> f64 {
        t * (t * self.a + self.b) + self.c
    }
}

/// Cubic polynomial coefficients: f(t) = at^3 + bt^2 + ct + d.
#[derive(Clone, Copy, Debug, Default)]
struct Cubic {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
}

impl Cubic {
    /// Construct from Bezier control points p0, p1, p2, p3.
    fn from_points(p0: f64, p1: f64, p2: f64, p3: f64) -> Self {
        Self {
            a: -p0 + 3.0 * p1 - 3.0 * p2 + p3,
            b: 3.0 * p0 - 6.0 * p1 + 3.0 * p2,
            c: -3.0 * p0 + 3.0 * p1,
            d: p0,
        }
    }

    /// Evaluate at parameter t.
    #[inline]
    fn eval(&self, t: f64) -> f64 {
        t * (t * (t * self.a + self.b) + self.c) + self.d
    }

    /// Get derivative (power rule: 3at^2 + 2bt + c).
    fn derivative(&self) -> Quadratic {
        Quadratic {
            a: 3.0 * self.a,
            b: 2.0 * self.b,
            c: self.c,
        }
    }
}

/// Filter zeros looking for answer in [0,1], choosing closest to 0.5.
fn filter_zeros_3(z0: f64, z1: f64, z2: f64) -> f64 {
    let mut result = z0;
    let mut min_error = (z0 - 0.5).abs();

    let error = (z1 - 0.5).abs();
    if error < min_error {
        // Check for multiple zeros (possibly regressive spline) per C++ reference
        // if min_error < 0.5 { TF_WARN("Possibly regressive spline") }
        result = z1;
        min_error = error;
    }

    let error = (z2 - 0.5).abs();
    if error < min_error {
        // if min_error < 0.5 { TF_WARN("Possibly regressive spline") }
        result = z2;
        min_error = error;
    }

    // Warn if no zero found in [0..1] per C++ reference
    if min_error > 0.5 + PARAMETER_EPSILON {
        // TF_WARN("No zero found in [0..1]")
    }

    result
}

/// Filter zeros looking for answer in [0,1], choosing closest to 0.5.
fn filter_zeros_2(z0: f64, z1: f64) -> f64 {
    let mut result = z0;
    let mut min_error = (z0 - 0.5).abs();
    let error1 = (z1 - 0.5).abs();

    if error1 < min_error {
        // Check for multiple zeros (possibly regressive spline) per C++ reference
        // if min_error < 0.5 { TF_WARN("Possibly regressive spline") }
        result = z1;
        min_error = error1;
    }

    // Warn if no zero found in [0..1] per C++ reference
    if min_error > 0.5 + PARAMETER_EPSILON {
        // TF_WARN("No zero found in [0..1]")
    }

    result
}

/// Find monotonic zero of quadratic using quadratic formula.
fn find_monotonic_zero_quadratic(quad: &Quadratic) -> f64 {
    let discrim = (quad.b.powi(2) - 4.0 * quad.a * quad.c).sqrt();
    let root0 = (-quad.b - discrim) / (2.0 * quad.a);
    let root1 = (-quad.b + discrim) / (2.0 * quad.a);
    filter_zeros_2(root0, root1)
}

/// Find monotonic zero using Cardano's algorithm for cubic t^3 + bt^2 + ct + d = 0.
fn find_monotonic_zero_scaled(b: f64, c: f64, d: f64) -> f64 {
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
        let t = -q / (2.0 * r);
        let phi = t.clamp(-1.0, 1.0).acos();
        let t1 = 2.0 * r.cbrt();
        let root1 = t1 * (phi / 3.0).cos() - b3;
        let root2 = t1 * ((phi + 2.0 * PI) / 3.0).cos() - b3;
        let root3 = t1 * ((phi + 4.0 * PI) / 3.0).cos() - b3;
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

/// Find monotonic zero of cubic polynomial.
fn find_monotonic_zero_cubic(cubic: &Cubic) -> f64 {
    const EPSILON: f64 = 1e-10;

    let a_zero = cubic.a.abs() < EPSILON;
    let b_zero = cubic.b.abs() < EPSILON;
    let c_zero = cubic.c.abs() < EPSILON;

    // Constant function check
    if a_zero && b_zero && c_zero {
        return 0.0;
    }

    // Linear case
    if a_zero && b_zero {
        return -cubic.d / cubic.c;
    }

    // Quadratic case
    if a_zero {
        return find_monotonic_zero_quadratic(&Quadratic {
            a: cubic.b,
            b: cubic.c,
            c: cubic.d,
        });
    }

    // Full cubic - scale to make a=1
    find_monotonic_zero_scaled(cubic.b / cubic.a, cubic.c / cubic.a, cubic.d / cubic.a)
}

/// Find Bezier parameter t at which time curve reaches given time.
fn find_bezier_parameter(
    begin: &TypedKnotData<f64>,
    end: &TypedKnotData<f64>,
    time: TsTime,
) -> f64 {
    // Build time cubic offset by eval time so we find zero
    let time_cubic = Cubic::from_points(
        begin.time() - time,
        begin.time() + begin.post_tan_width() - time,
        end.time() - end.pre_tan_width() - time,
        end.time() - time,
    );

    let mut t = find_monotonic_zero_cubic(&time_cubic);

    // Clamp with epsilon warning per C++ reference
    if t < -PARAMETER_EPSILON {
        // TF_WARN equivalent: parameter out of range
        t = 0.0;
    } else if t < 0.0 {
        t = 0.0;
    } else if t > 1.0 + PARAMETER_EPSILON {
        // TF_WARN equivalent: parameter out of range
        t = 1.0;
    } else if t > 1.0 {
        t = 1.0;
    }

    t
}

/// Evaluate Bezier curve segment.
fn eval_bezier(
    begin_in: &TypedKnotData<f64>,
    end_in: &TypedKnotData<f64>,
    time: TsTime,
    aspect: EvalAspect,
) -> f64 {
    // De-regress if needed (always uses KeepRatio strategy)
    let mut begin = *begin_in;
    let mut end = *end_in;
    RegressionPreventerBatch::process_segment(
        &mut begin.base,
        &mut end.base,
        AntiRegressionMode::KeepRatio,
    );

    let t = find_bezier_parameter(&begin, &end, time);

    // Value cubic: y = f(t)
    let value_cubic = Cubic::from_points(
        begin.value,
        begin.value + begin.post_tan_height(),
        end.pre_value() + end.pre_tan_height(),
        end.pre_value(),
    );

    if aspect == EvalAspect::Value || aspect == EvalAspect::HeldValue {
        value_cubic.eval(t)
    } else {
        // Derivative: dy/dx = (dy/dt) / (dx/dt)
        let time_cubic = Cubic::from_points(
            begin.time(),
            begin.time() + begin.post_tan_width(),
            end.time() - end.pre_tan_width(),
            end.time(),
        );

        let value_deriv = value_cubic.derivative();
        let time_deriv = time_cubic.derivative();
        value_deriv.eval(t) / time_deriv.eval(t)
    }
}

/// Evaluate Hermite curve segment.
fn eval_hermite(
    begin: &TypedKnotData<f64>,
    end: &TypedKnotData<f64>,
    time: TsTime,
    aspect: EvalAspect,
) -> f64 {
    let t0 = begin.time();
    let v0 = begin.value;
    let m0 = begin.post_tan_slope;

    let t1 = end.time();
    let v1 = end.pre_value();
    let m1 = end.pre_tan_slope;

    if t0 >= t1 {
        return v0;
    }

    let dt = t1 - t0;
    let dv = v1 - v0;

    // Convert to [0..1] range
    let u = (time - t0) / dt;
    let um0 = m0 * dt;
    let um1 = m1 * dt;

    // Hermite coefficients
    let a = -2.0 * dv + um0 + um1;
    let b = 3.0 * dv - 2.0 * um0 - um1;
    let c = um0;
    let d = v0;

    if aspect == EvalAspect::Derivative {
        // Chain rule derivative
        (u * (u * 3.0 * a + 2.0 * b) + c) / dt
    } else {
        // Value evaluation
        u * (u * (u * a + b) + c) + d
    }
}

//=============================================================================
// EVAL HELPERS
//=============================================================================

/// Get slope between two knots in linear segment.
fn get_segment_slope(begin: &TypedKnotData<f64>, end: &TypedKnotData<f64>) -> f64 {
    (end.pre_value() - begin.value) / (end.time() - begin.time())
}

/// Get slope in extrapolation region.
fn get_extrapolation_slope(
    extrap: &Extrapolation,
    have_multiple_knots: bool,
    end_knot: &TypedKnotData<f64>,
    adjacent: &TypedKnotData<f64>,
    location: EvalLocation,
) -> Option<f64> {
    match extrap.mode {
        ExtrapMode::ValueBlock => None,
        ExtrapMode::Held => Some(0.0),
        ExtrapMode::Sloped => Some(extrap.slope),
        ExtrapMode::Linear => {
            if !have_multiple_knots {
                return Some(0.0);
            }

            // Dual-valued end knot means flat slope
            if end_knot.is_dual_valued() {
                return Some(0.0);
            }

            if location == EvalLocation::Pre {
                match end_knot.next_interp() {
                    InterpMode::Held | InterpMode::ValueBlock => Some(0.0),
                    InterpMode::Linear => Some(get_segment_slope(end_knot, adjacent)),
                    InterpMode::Curve => Some(end_knot.post_tan_slope),
                }
            } else {
                match adjacent.next_interp() {
                    InterpMode::Held | InterpMode::ValueBlock => Some(0.0),
                    InterpMode::Linear => Some(get_segment_slope(adjacent, end_knot)),
                    InterpMode::Curve => Some(end_knot.pre_tan_slope),
                }
            }
        }
        _ => Some(0.0), // Loop modes resolved before reaching here
    }
}

/// Extrapolate linear from a knot.
fn extrapolate_linear(
    knot: &TypedKnotData<f64>,
    slope: f64,
    time: TsTime,
    location: EvalLocation,
) -> f64 {
    if location == EvalLocation::Pre {
        knot.pre_value() - slope * (knot.time() - time)
    } else {
        knot.value + slope * (time - knot.time())
    }
}

//=============================================================================
// LOOP RESOLUTION
//=============================================================================

/// Resolves evaluation time and value adjustments for loop regions.
#[derive(Debug)]
struct LoopResolver {
    eval_time: TsTime,
    location: EvalLocation,
    value_offset: f64,
    negate: bool,
    between_last_proto_and_end: bool,
    between_pre_unlooped_and_looped: bool,
    between_looped_and_post_unlooped: bool,
    first_time_looped: bool,
    last_time_looped: bool,
    first_inner_proto_index: usize,
    extrap_knot1: Option<TypedKnotData<f64>>,
    extrap_knot2: Option<TypedKnotData<f64>>,
}

impl LoopResolver {
    /// Create resolver for given spline data and evaluation time.
    fn new(data: &SplineData, time_in: TsTime, aspect: EvalAspect, location: EvalLocation) -> Self {
        let mut resolver = Self {
            eval_time: time_in,
            location,
            value_offset: 0.0,
            negate: false,
            between_last_proto_and_end: false,
            between_pre_unlooped_and_looped: false,
            between_looped_and_post_unlooped: false,
            first_time_looped: false,
            last_time_looped: false,
            first_inner_proto_index: 0,
            extrap_knot1: None,
            extrap_knot2: None,
        };

        let have_inner_loops = data.has_inner_loops();
        if have_inner_loops {
            resolver.first_inner_proto_index = data.first_inner_proto_index().unwrap_or(0);
        }

        let have_multiple_knots = have_inner_loops || data.times.len() > 1;
        let have_pre_extrap_loops = have_multiple_knots && data.pre_extrapolation.is_looping();
        let have_post_extrap_loops = have_multiple_knots && data.post_extrapolation.is_looping();

        if !have_inner_loops && !have_pre_extrap_loops && !have_post_extrap_loops {
            return resolver;
        }

        // Find first and last knot times - may be authored or echoed from inner loops
        let raw_first_time = data.times.first().copied().unwrap_or(0.0);
        let raw_last_time = data.times.last().copied().unwrap_or(0.0);
        let mut first_time = raw_first_time;
        let mut last_time = raw_last_time;

        // Check if looped interval extends beyond authored knots
        if have_inner_loops {
            let looped_interval = data.loop_params.looped_interval();
            if looped_interval.get_min() < raw_first_time {
                first_time = looped_interval.get_min();
                resolver.first_time_looped = true;
            }
            if looped_interval.get_max() > raw_last_time {
                last_time = looped_interval.get_max();
                resolver.last_time_looped = true;
            }
        }

        // Resolve extrapolating loops first, then inner loops
        if have_pre_extrap_loops || have_post_extrap_loops {
            resolver.resolve_extrap_full(
                data,
                have_pre_extrap_loops,
                have_post_extrap_loops,
                aspect,
                first_time,
                last_time,
            );
        }

        if have_inner_loops {
            resolver.resolve_inner(data, aspect);
        }

        resolver
    }

    /// Resolve inner loop regions.
    fn resolve_inner(&mut self, data: &SplineData, _aspect: EvalAspect) {
        let lp = &data.loop_params;
        let looped_interval = lp.looped_interval();
        let proto_interval = lp.prototype_interval();
        let proto_span = proto_interval.size();
        let times = &data.times;

        // Handle evaluation in echo regions
        if looped_interval.contains(self.eval_time) && !proto_interval.contains(self.eval_time) {
            if self.eval_time < lp.proto_start {
                // Pre-echo: hop forward to prototype
                let loop_offset = lp.proto_start - self.eval_time;
                let iter_num = (loop_offset / proto_span).ceil() as i32;
                self.eval_time += (iter_num as f64) * proto_span;
                self.value_offset -= (iter_num as f64) * lp.value_offset;
            } else {
                // Post-echo: hop backward to prototype
                let loop_offset = self.eval_time - lp.proto_end;
                let iter_num = (loop_offset / proto_span) as i32 + 1;
                self.eval_time -= (iter_num as f64) * proto_span;
                self.value_offset += (iter_num as f64) * lp.value_offset;
            }
        }

        // Look for special interpolation and extrapolation cases
        let first_time = looped_interval.get_min();
        let last_time = looped_interval.get_max();

        // Case 1: Between last prototype knot and prototype end
        if proto_interval.contains(self.eval_time) {
            // Find last prototype knot time using binary search
            let lb_idx =
                times[self.first_inner_proto_index..].partition_point(|&t| t < lp.proto_end);
            if lb_idx > 0 {
                let last_proto_time = times[self.first_inner_proto_index + lb_idx - 1];
                if self.eval_time > last_proto_time {
                    self.between_last_proto_and_end = true;
                }
            }
        }
        // Case 2: Pre-extrapolating with first knots from inner looping
        else if self.eval_time < first_time {
            if self.first_time_looped {
                // First knot is always a copy of the first prototype knot
                self.extrap_knot1 = self.copy_proto_knot_data(
                    data,
                    self.first_inner_proto_index,
                    -lp.num_pre_loops,
                );

                // Determine second knot
                if times.len() > self.first_inner_proto_index + 1
                    && proto_interval.contains(times[self.first_inner_proto_index + 1])
                {
                    // Second knot is a copy of the second prototype knot
                    self.extrap_knot2 = self.copy_proto_knot_data(
                        data,
                        self.first_inner_proto_index + 1,
                        -lp.num_pre_loops,
                    );
                } else {
                    // No knots after first prototype knot, so second is another copy of first
                    self.extrap_knot2 = self.copy_proto_knot_data(
                        data,
                        self.first_inner_proto_index,
                        -lp.num_pre_loops + 1,
                    );
                }
            }
        }
        // Case 3: Post-extrapolating with last knots from inner looping
        else if self.eval_time > last_time {
            if self.last_time_looped {
                // Last knot is always a copy of the first prototype knot
                self.extrap_knot1 = self.copy_proto_knot_data(
                    data,
                    self.first_inner_proto_index,
                    lp.num_post_loops + 1,
                );

                // Find last authored prototype knot
                let lb_idx =
                    times[self.first_inner_proto_index..].partition_point(|&t| t < lp.proto_end);
                let last_proto_idx = self.first_inner_proto_index + lb_idx.saturating_sub(1);

                // Second-to-last knot is a copy of the last prototype knot
                self.extrap_knot2 =
                    self.copy_proto_knot_data(data, last_proto_idx, lp.num_post_loops);
            }
        }
        // Case 4: Between last knot before looping region and start of looping region
        else if self.eval_time < looped_interval.get_min() {
            let lb_idx = times[..self.first_inner_proto_index]
                .partition_point(|&t| t < looped_interval.get_min());
            if lb_idx > 0 && lb_idx <= self.first_inner_proto_index {
                self.between_pre_unlooped_and_looped = true;
            }
        }
        // Case 5: Between end of looping region and first knot after looping region
        else if self.eval_time > looped_interval.get_max() {
            // Find first knot after looping region
            let lb_idx = times.partition_point(|&t| t <= looped_interval.get_max());
            if lb_idx < times.len() {
                self.between_looped_and_post_unlooped = true;
            }
        }
    }

    /// Resolve extrapolating loop regions with full time bounds.
    fn resolve_extrap_full(
        &mut self,
        data: &SplineData,
        have_pre: bool,
        have_post: bool,
        aspect: EvalAspect,
        first_time: TsTime,
        last_time: TsTime,
    ) {
        // Determine the interval that doesn't require extrapolation
        // One end is closed, the other open depending on eval location
        let in_knot_interval = if self.location == EvalLocation::Pre {
            self.eval_time > first_time && self.eval_time <= last_time
        } else {
            self.eval_time >= first_time && self.eval_time < last_time
        };

        if in_knot_interval {
            return;
        }

        // Is the extrapolation looped?
        let do_pre = have_pre && self.eval_time < last_time;
        let do_post = have_post && self.eval_time > first_time;

        if !do_pre && !do_post {
            return;
        }

        // Handle looped extrapolation
        if do_pre {
            self.do_extrap(
                data,
                &data.pre_extrapolation,
                first_time - self.eval_time,
                true,
                first_time,
                last_time,
                aspect,
            );
        } else if do_post {
            self.do_extrap(
                data,
                &data.post_extrapolation,
                self.eval_time - last_time,
                false,
                first_time,
                last_time,
                aspect,
            );
        }
    }

    /// Perform extrapolation loop resolution.
    fn do_extrap(
        &mut self,
        data: &SplineData,
        extrap: &Extrapolation,
        offset: f64,
        is_pre: bool,
        first_time: TsTime,
        last_time: TsTime,
        aspect: EvalAspect,
    ) {
        let proto_span = last_time - first_time;
        if proto_span <= 0.0 {
            return;
        }

        let num_iters_frac = offset / proto_span;
        let num_iters_trunc = num_iters_frac as i32;
        let boundary = (num_iters_trunc as f64) == num_iters_frac;

        // Determine if we're on the "short" side of a boundary
        let short_offset = boundary
            && ((is_pre && self.location != EvalLocation::Pre)
                || (!is_pre && self.location == EvalLocation::Pre));

        let num_iters = if short_offset {
            num_iters_trunc
        } else {
            num_iters_trunc + 1
        };

        let iter_hop = if is_pre { num_iters } else { -num_iters };

        // Hop to non-extrapolating region
        self.eval_time += (iter_hop as f64) * proto_span;

        match extrap.mode {
            ExtrapMode::LoopRepeat => {
                if aspect != EvalAspect::Derivative {
                    // Compute value offset for repeat mode
                    let extrap_value_offset = self.compute_extrap_value_offset(data);
                    self.value_offset -= (iter_hop as f64) * extrap_value_offset;
                }
            }
            ExtrapMode::LoopOscillate => {
                if iter_hop % 2 != 0 {
                    // Reflect in time
                    self.eval_time = first_time + (proto_span - (self.eval_time - first_time));
                    self.location = if self.location == EvalLocation::Pre {
                        EvalLocation::Post
                    } else {
                        EvalLocation::Pre
                    };
                    if aspect == EvalAspect::Derivative {
                        self.negate = true;
                    }
                }
            }
            _ => {} // Reset mode: no special handling
        }
    }

    /// Compute value offset for repeat extrapolation.
    fn compute_extrap_value_offset(&self, data: &SplineData) -> f64 {
        let lp = &data.loop_params;

        // Compute first value - may be from inner loops
        let first_value = if !self.first_time_looped {
            // Earliest knot is not from inner loops - read its value
            data.get_knot_data_as_double(0)
                .map(|kd| kd.pre_value())
                .unwrap_or(0.0)
        } else {
            // Earliest knot is from inner loops - compute its value
            data.get_knot_data_as_double(self.first_inner_proto_index)
                .map(|kd| kd.pre_value() - (lp.num_pre_loops as f64) * lp.value_offset)
                .unwrap_or(0.0)
        };

        // Compute last value - may be from inner loops
        let last_value = if !self.last_time_looped {
            // Latest knot is not from inner loops - read its value
            data.get_knot_data_as_double(data.times.len().saturating_sub(1))
                .map(|kd| kd.value)
                .unwrap_or(0.0)
        } else {
            // Latest knot is from inner loops - it's the final echo of prototype start knot
            data.get_knot_data_as_double(self.first_inner_proto_index)
                .map(|kd| kd.value + ((lp.num_post_loops + 1) as f64) * lp.value_offset)
                .unwrap_or(0.0)
        };

        last_value - first_value
    }

    /// Replace pre-extrapolation knots when first knots are from inner looping.
    fn replace_pre_extrap_knots(
        &self,
        next: &mut TypedKnotData<f64>,
        next2: &mut TypedKnotData<f64>,
    ) -> bool {
        if !self.first_time_looped {
            return false;
        }

        if let Some(ref kd1) = self.extrap_knot1 {
            *next = *kd1;
        }
        if let Some(ref kd2) = self.extrap_knot2 {
            *next2 = *kd2;
        }
        true
    }

    /// Replace post-extrapolation knots when last knots are from inner looping.
    fn replace_post_extrap_knots(
        &self,
        prev: &mut TypedKnotData<f64>,
        prev2: &mut TypedKnotData<f64>,
    ) -> bool {
        if !self.last_time_looped {
            return false;
        }

        if let Some(ref kd1) = self.extrap_knot1 {
            *prev = *kd1;
        }
        if let Some(ref kd2) = self.extrap_knot2 {
            *prev2 = *kd2;
        }
        true
    }

    /// Replace boundary knots for inner loop special cases.
    fn replace_boundary_knots(
        &self,
        data: &SplineData,
        prev: &mut TypedKnotData<f64>,
        next: &mut TypedKnotData<f64>,
    ) -> (bool, bool) {
        let mut gen_prev = false;
        let mut gen_next = false;

        if self.between_last_proto_and_end {
            if let Some(kd) = self.copy_proto_knot_data(data, self.first_inner_proto_index, 1) {
                *next = kd;
                gen_next = true;
            }
        } else if self.between_pre_unlooped_and_looped {
            let lp = &data.loop_params;
            if let Some(kd) =
                self.copy_proto_knot_data(data, self.first_inner_proto_index, -lp.num_pre_loops)
            {
                *next = kd;
                gen_next = true;
            }
        } else if self.between_looped_and_post_unlooped {
            let lp = &data.loop_params;
            if let Some(kd) =
                self.copy_proto_knot_data(data, self.first_inner_proto_index, lp.num_post_loops + 1)
            {
                *prev = kd;
                gen_prev = true;
            }
        }

        (gen_prev, gen_next)
    }

    /// Copy prototype knot data with time/value shift.
    fn copy_proto_knot_data(
        &self,
        data: &SplineData,
        index: usize,
        shift_iters: i32,
    ) -> Option<TypedKnotData<f64>> {
        let lp = &data.loop_params;
        let proto_span = lp.prototype_interval().size();

        let mut kd = data.get_knot_data_as_double(index)?;
        kd.base.time += (shift_iters as f64) * proto_span;
        kd.value += (shift_iters as f64) * lp.value_offset;
        if kd.base.dual_valued {
            kd.pre_value += (shift_iters as f64) * lp.value_offset;
        }

        Some(kd)
    }
}

//=============================================================================
// INTERPOLATION
//=============================================================================

/// Interpolate between two knots.
fn interpolate(
    begin: &TypedKnotData<f64>,
    end: &TypedKnotData<f64>,
    time: TsTime,
    aspect: EvalAspect,
    curve_type: CurveType,
) -> Option<f64> {
    // Value blocks return None
    if begin.next_interp() == InterpMode::ValueBlock {
        return None;
    }

    // Held value always returns begin value
    if aspect == EvalAspect::HeldValue {
        return Some(begin.value);
    }

    match begin.next_interp() {
        InterpMode::Curve => {
            let value = if curve_type == CurveType::Bezier {
                eval_bezier(begin, end, time, aspect)
            } else {
                eval_hermite(begin, end, time, aspect)
            };
            Some(value)
        }
        InterpMode::Held => Some(if aspect == EvalAspect::Value {
            begin.value
        } else {
            0.0
        }),
        InterpMode::Linear => {
            let slope = get_segment_slope(begin, end);
            if aspect == EvalAspect::Derivative {
                Some(slope)
            } else {
                Some(extrapolate_linear(begin, slope, time, EvalLocation::Post))
            }
        }
        InterpMode::ValueBlock => None,
    }
}

/// Main evaluation logic after loop resolution.
fn eval_main(data: &SplineData, loop_res: &LoopResolver, aspect: EvalAspect) -> Option<f64> {
    let time = loop_res.eval_time;
    let location = loop_res.location;
    let times = &data.times;

    if times.is_empty() {
        return None;
    }

    // Binary search for first knot at or after time
    let lb_idx = times.partition_point(|&t| t < time);

    let at_knot = lb_idx < times.len() && times[lb_idx] == time;
    let prev_idx = if lb_idx > 0 { Some(lb_idx - 1) } else { None };
    let knot_idx = if at_knot { Some(lb_idx) } else { None };
    let next_idx = if at_knot {
        if lb_idx + 1 < times.len() {
            Some(lb_idx + 1)
        } else {
            None
        }
    } else if lb_idx < times.len() {
        Some(lb_idx)
    } else {
        None
    };

    let before_start = lb_idx == 0 && !at_knot;
    let after_end = !loop_res.between_last_proto_and_end
        && prev_idx.is_some()
        && prev_idx.expect("value expected") == times.len() - 1
        && !at_knot;
    let at_first = knot_idx == Some(0);
    let at_last = knot_idx == Some(times.len() - 1);
    let have_multiple_knots = times.len() > 1;

    // Get knot data
    let knot_data = knot_idx.and_then(|i| data.get_knot_data_as_double(i));
    let prev_data = prev_idx.and_then(|i| data.get_knot_data_as_double(i));
    let next_data = next_idx.and_then(|i| data.get_knot_data_as_double(i));

    // Handle times at knots
    if at_knot {
        let kd = knot_data.as_ref()?;

        // Handle values
        if aspect == EvalAspect::Value || aspect == EvalAspect::HeldValue {
            if location == EvalLocation::Pre {
                if at_first {
                    if data.pre_extrapolation.mode == ExtrapMode::ValueBlock {
                        return None;
                    }
                } else if let Some(ref pd) = prev_data {
                    if pd.next_interp() == InterpMode::ValueBlock {
                        return None;
                    } else if pd.next_interp() == InterpMode::Held
                        || aspect == EvalAspect::HeldValue
                    {
                        return Some(pd.value);
                    }
                }
            } else if at_last {
                if data.post_extrapolation.mode == ExtrapMode::ValueBlock {
                    return None;
                }
            } else if kd.next_interp() == InterpMode::ValueBlock {
                return None;
            }

            return Some(if location == EvalLocation::Pre {
                kd.pre_value()
            } else {
                kd.value
            });
        }

        // Handle derivatives
        if location == EvalLocation::Pre {
            if at_first {
                return get_extrapolation_slope(
                    &data.pre_extrapolation,
                    have_multiple_knots,
                    kd,
                    next_data.as_ref().unwrap_or(kd),
                    EvalLocation::Pre,
                );
            }

            if let Some(ref pd) = prev_data {
                return match pd.next_interp() {
                    InterpMode::ValueBlock => None,
                    InterpMode::Held => Some(0.0),
                    InterpMode::Linear => Some(get_segment_slope(pd, kd)),
                    InterpMode::Curve => Some(kd.pre_tan_slope),
                };
            }
        } else {
            if at_last {
                return get_extrapolation_slope(
                    &data.post_extrapolation,
                    have_multiple_knots,
                    kd,
                    prev_data.as_ref().unwrap_or(kd),
                    EvalLocation::Post,
                );
            }

            return match kd.next_interp() {
                InterpMode::ValueBlock => None,
                InterpMode::Held => Some(0.0),
                InterpMode::Linear => {
                    if let Some(ref nd) = next_data {
                        Some(get_segment_slope(kd, nd))
                    } else {
                        Some(0.0)
                    }
                }
                InterpMode::Curve => Some(kd.post_tan_slope),
            };
        }
    }

    // Extrapolate before first knot
    if before_start {
        if data.pre_extrapolation.mode == ExtrapMode::ValueBlock {
            return None;
        }

        let mut nd = *next_data.as_ref()?;
        let mut nd2 = if next_idx.expect("value expected") + 1 < times.len() {
            data.get_knot_data_as_double(next_idx.expect("value expected") + 1)
                .unwrap_or(nd)
        } else {
            nd
        };

        // Apply inner-loop knot replacement
        loop_res.replace_pre_extrap_knots(&mut nd, &mut nd2);

        if aspect == EvalAspect::HeldValue {
            return Some(nd.pre_value());
        }

        let slope = get_extrapolation_slope(
            &data.pre_extrapolation,
            have_multiple_knots,
            &nd,
            &nd2,
            EvalLocation::Pre,
        )?;

        if aspect == EvalAspect::Derivative {
            return Some(slope);
        }

        return Some(extrapolate_linear(&nd, slope, time, EvalLocation::Pre));
    }

    // Extrapolate after last knot
    if after_end {
        if data.post_extrapolation.mode == ExtrapMode::ValueBlock {
            return None;
        }

        let mut pd = *prev_data.as_ref()?;
        let mut pd2 = if prev_idx.expect("value expected") > 0 {
            data.get_knot_data_as_double(prev_idx.expect("value expected") - 1)
                .unwrap_or(pd)
        } else {
            pd
        };

        // Apply inner-loop knot replacement
        loop_res.replace_post_extrap_knots(&mut pd, &mut pd2);

        if aspect == EvalAspect::HeldValue {
            return Some(pd.value);
        }

        let slope = get_extrapolation_slope(
            &data.post_extrapolation,
            have_multiple_knots,
            &pd,
            &pd2,
            EvalLocation::Post,
        )?;

        if aspect == EvalAspect::Derivative {
            return Some(slope);
        }

        return Some(extrapolate_linear(&pd, slope, time, EvalLocation::Post));
    }

    // Between knots - account for loop boundaries
    let mut pd = prev_data?;
    let mut nd = next_data?;
    loop_res.replace_boundary_knots(data, &mut pd, &mut nd);

    interpolate(&pd, &nd, time, aspect, data.curve_type)
}

//=============================================================================
// PUBLIC API
//=============================================================================

/// Evaluate a spline at the given time.
///
/// Returns `None` if the spline is empty or in a value-blocked region.
pub fn eval(
    data: &SplineData,
    time: TsTime,
    aspect: EvalAspect,
    location: EvalLocation,
) -> Option<f64> {
    if data.times.is_empty() {
        return None;
    }

    let loop_res = LoopResolver::new(data, time, aspect, location);
    let result = eval_main(data, &loop_res, aspect)?;

    // Apply value offset and negation from loop resolution
    let value = (result + loop_res.value_offset) * if loop_res.negate { -1.0 } else { 1.0 };
    Some(value)
}

/// Breakdown (split) a spline segment at a specific time.
///
/// Returns true if breakdown was successful, false otherwise.
/// If `test_only` is true, only tests if breakdown is possible without modifying.
pub fn breakdown(
    data: &mut SplineData,
    at_time: TsTime,
    test_only: bool,
    affected_interval: &mut Interval,
    reason: &mut String,
) -> bool {
    if data.times.is_empty() {
        *reason = "Cannot breakdown an empty spline.".to_string();
        return false;
    }

    // Check if time is at an existing knot
    if data
        .times
        .binary_search_by(|t| t.partial_cmp(&at_time).expect("value expected"))
        .is_ok()
    {
        *reason = format!("Cannot breakdown a spline at an existing knot (time={at_time})");
        return false;
    }

    // Check for inner loop regions
    if data.has_inner_loops() {
        let lp = &data.loop_params;
        let looped = lp.looped_interval();
        let proto = lp.prototype_interval();

        if looped.contains(at_time) && !proto.contains(at_time) {
            *reason = format!(
                "Cannot breakdown a spline in a region masked by inner looping (time={at_time})"
            );
            return false;
        }
    }

    // Check for extrapolating loop regions
    let have_multiple = data.times.len() > 1 || data.has_inner_loops();
    let first_time = data.times.first().copied().unwrap_or(0.0);
    let last_time = data.times.last().copied().unwrap_or(0.0);

    if have_multiple {
        if at_time < first_time && data.pre_extrapolation.is_looping() {
            *reason = format!(
                "Cannot breakdown a spline in a region generated by extrapolation looping (time={at_time})"
            );
            return false;
        }
        if at_time > last_time && data.post_extrapolation.is_looping() {
            *reason = format!(
                "Cannot breakdown a spline in a region generated by extrapolation looping (time={at_time})"
            );
            return false;
        }
    }

    if test_only {
        return true;
    }

    // Perform actual breakdown
    *affected_interval = Interval::new(at_time, at_time, true, true);

    // Find bracketing knots
    let lb_idx = data.times.partition_point(|&t| t < at_time);
    let before_start = lb_idx == 0;
    let after_end = lb_idx == data.times.len();

    if before_start {
        // Breakdown in pre-extrapolation region
        return breakdown_pre_extrap(data, at_time, affected_interval, reason);
    }

    if after_end {
        // Breakdown in post-extrapolation region
        return breakdown_post_extrap(data, at_time, affected_interval, reason);
    }

    // Breakdown between knots
    breakdown_between(data, at_time, lb_idx - 1, lb_idx, affected_interval, reason)
}

/// Breakdown in pre-extrapolation region.
fn breakdown_pre_extrap(
    data: &mut SplineData,
    at_time: TsTime,
    affected: &mut Interval,
    reason: &mut String,
) -> bool {
    let next_data = match data.get_knot_data_as_double(0) {
        Some(kd) => kd,
        None => {
            *reason = "No knots available for pre-extrapolation breakdown".to_string();
            return false;
        }
    };

    // Compute extrapolation slope
    let slope = match get_extrapolation_slope(
        &data.pre_extrapolation,
        data.times.len() > 1,
        &next_data,
        &next_data,
        EvalLocation::Pre,
    ) {
        Some(s) => s,
        None => {
            *reason = "Cannot compute pre-extrapolation slope".to_string();
            return false;
        }
    };

    // Create new knot
    let mut new_knot = TypedKnotData::<f64>::new();
    new_knot.base.time = at_time;
    new_knot.value = extrapolate_linear(&next_data, slope, at_time, EvalLocation::Pre);

    match data.pre_extrapolation.mode {
        ExtrapMode::Held => {
            new_knot.base.next_interp = InterpMode::Held;
        }
        ExtrapMode::Linear | ExtrapMode::Sloped => {
            new_knot.base.next_interp = InterpMode::Linear;
            new_knot.pre_tan_slope = slope;
            new_knot.post_tan_slope = slope;
        }
        _ => {
            *reason = "Invalid pre-extrapolation mode for breakdown".to_string();
            return false;
        }
    }

    // Set tangent widths
    new_knot.base.pre_tan_width = next_data.pre_tan_width();
    new_knot.base.post_tan_width = (next_data.time() - new_knot.time()) / 3.0;

    data.set_knot_from_double(&new_knot);
    affected.set_max(next_data.time());
    true
}

/// Breakdown in post-extrapolation region.
fn breakdown_post_extrap(
    data: &mut SplineData,
    at_time: TsTime,
    affected: &mut Interval,
    reason: &mut String,
) -> bool {
    let last_idx = data.times.len() - 1;
    let prev_data = match data.get_knot_data_as_double(last_idx) {
        Some(kd) => kd,
        None => {
            *reason = "No knots available for post-extrapolation breakdown".to_string();
            return false;
        }
    };

    // Compute extrapolation slope
    let slope = match get_extrapolation_slope(
        &data.post_extrapolation,
        data.times.len() > 1,
        &prev_data,
        &prev_data,
        EvalLocation::Post,
    ) {
        Some(s) => s,
        None => {
            *reason = "Cannot compute post-extrapolation slope".to_string();
            return false;
        }
    };

    // Create new knot
    let mut new_knot = TypedKnotData::<f64>::new();
    new_knot.base.time = at_time;
    new_knot.value = extrapolate_linear(&prev_data, slope, at_time, EvalLocation::Post);

    match data.post_extrapolation.mode {
        ExtrapMode::Held => {
            new_knot.base.next_interp = InterpMode::Held;
        }
        ExtrapMode::Linear | ExtrapMode::Sloped => {
            new_knot.base.next_interp = InterpMode::Linear;
            new_knot.pre_tan_slope = slope;
            new_knot.post_tan_slope = slope;
        }
        _ => {
            *reason = "Invalid post-extrapolation mode for breakdown".to_string();
            return false;
        }
    }

    // Set tangent widths
    new_knot.base.pre_tan_width = (new_knot.time() - prev_data.time()) / 3.0;
    new_knot.base.post_tan_width = prev_data.post_tan_width();

    data.set_knot_from_double(&new_knot);
    affected.set_min(prev_data.time());
    true
}

/// Breakdown between two existing knots.
fn breakdown_between(
    data: &mut SplineData,
    at_time: TsTime,
    prev_idx: usize,
    next_idx: usize,
    affected: &mut Interval,
    _reason: &mut String,
) -> bool {
    let mut prev_data = match data.get_knot_data_as_double(prev_idx) {
        Some(kd) => kd,
        None => return false,
    };
    let mut next_data = match data.get_knot_data_as_double(next_idx) {
        Some(kd) => kd,
        None => return false,
    };

    let mut new_knot = TypedKnotData::<f64>::new();
    new_knot.base.time = at_time;

    match prev_data.next_interp() {
        InterpMode::ValueBlock => {
            new_knot.base.next_interp = InterpMode::ValueBlock;
        }
        InterpMode::Held => {
            new_knot.base.next_interp = InterpMode::Held;
            new_knot.value = prev_data.value;
        }
        InterpMode::Linear => {
            let slope = get_segment_slope(&prev_data, &next_data);
            new_knot.base.next_interp = InterpMode::Linear;
            new_knot.value = extrapolate_linear(&prev_data, slope, at_time, EvalLocation::Post);
        }
        InterpMode::Curve => {
            new_knot.base.next_interp = InterpMode::Curve;
            if data.curve_type == CurveType::Bezier {
                breakdown_bezier(&mut prev_data, &mut new_knot, &mut next_data);
            } else {
                breakdown_hermite(&mut prev_data, &mut new_knot, &mut next_data);
            }
        }
    }

    // Update spline data
    data.set_knot_from_double(&prev_data);
    data.set_knot_from_double(&new_knot);
    data.set_knot_from_double(&next_data);

    *affected = Interval::new(prev_data.time(), next_data.time(), true, true);
    true
}

/// Breakdown Hermite segment.
fn breakdown_hermite(
    prev: &mut TypedKnotData<f64>,
    knot: &mut TypedKnotData<f64>,
    next: &mut TypedKnotData<f64>,
) {
    // Evaluate value and slope at the breakdown time
    knot.value = eval_hermite(prev, next, knot.time(), EvalAspect::Value);
    let slope = eval_hermite(prev, next, knot.time(), EvalAspect::Derivative);
    knot.pre_tan_slope = slope;
    knot.post_tan_slope = slope;
}

/// Breakdown Bezier segment using De Casteljau algorithm.
fn breakdown_bezier(
    prev: &mut TypedKnotData<f64>,
    knot: &mut TypedKnotData<f64>,
    next: &mut TypedKnotData<f64>,
) {
    // De-regress if needed
    RegressionPreventerBatch::process_segment(
        &mut prev.base,
        &mut next.base,
        AntiRegressionMode::KeepRatio,
    );

    let u = find_bezier_parameter(prev, next, knot.time());

    // Get Bezier control points
    let cp = [
        Vec2d::new(prev.time(), prev.value),
        Vec2d::new(
            prev.time() + prev.post_tan_width(),
            prev.value + prev.post_tan_height(),
        ),
        Vec2d::new(
            next.time() - next.pre_tan_width(),
            next.value + next.pre_tan_height(),
        ),
        Vec2d::new(next.time(), next.value),
    ];

    // De Casteljau interpolation
    let lerp = |t: f64, a: Vec2d, b: Vec2d| -> Vec2d {
        Vec2d::new(a[0] * (1.0 - t) + b[0] * t, a[1] * (1.0 - t) + b[1] * t)
    };

    let cp01 = lerp(u, cp[0], cp[1]);
    let cp12 = lerp(u, cp[1], cp[2]);
    let cp23 = lerp(u, cp[2], cp[3]);

    let cp012 = lerp(u, cp01, cp12);
    let cp123 = lerp(u, cp12, cp23);

    let cp0123 = lerp(u, cp012, cp123);

    // Rebuild tangents
    let prev_post_tan = cp01 - cp[0];
    let knot_pre_tan = cp0123 - cp012;
    let knot_post_tan = cp123 - cp0123;
    let next_pre_tan = cp[3] - cp23;

    // Convert tangent vectors to width/slope
    let convert_tan = |tan: Vec2d| -> (f64, f64) {
        if tan[0].abs() < 1e-10 {
            if tan[1].abs() < 1e-10 {
                (0.0, 0.0)
            } else {
                let near_vertical = 200000.0;
                (
                    tan[1].abs() / near_vertical,
                    tan[1].signum() * near_vertical,
                )
            }
        } else {
            (tan[0], tan[1] / tan[0])
        }
    };

    let (pw, ps) = convert_tan(prev_post_tan);
    prev.base.post_tan_width = pw;
    prev.post_tan_slope = ps;

    let (kpw, kps) = convert_tan(knot_pre_tan);
    knot.base.pre_tan_width = kpw;
    knot.pre_tan_slope = kps;
    knot.value = cp0123[1];

    let (kow, kos) = convert_tan(knot_post_tan);
    knot.base.post_tan_width = kow;
    knot.post_tan_slope = kos;

    let (nw, ns) = convert_tan(next_pre_tan);
    next.base.pre_tan_width = nw;
    next.pre_tan_slope = ns;
}

//=============================================================================
// EVAL TYPES
//=============================================================================

/// What aspect of the spline to evaluate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EvalAspect {
    /// Evaluate the value.
    #[default]
    Value = 0,
    /// Evaluate the held value (ignoring interpolation).
    HeldValue = 1,
    /// Evaluate the derivative (rate of change).
    Derivative = 2,
}

impl EvalAspect {
    /// Returns true if evaluating value.
    #[inline]
    #[must_use]
    pub fn is_value(&self) -> bool {
        matches!(self, Self::Value)
    }

    /// Returns true if evaluating held value.
    #[inline]
    #[must_use]
    pub fn is_held_value(&self) -> bool {
        matches!(self, Self::HeldValue)
    }

    /// Returns true if evaluating derivative.
    #[inline]
    #[must_use]
    pub fn is_derivative(&self) -> bool {
        matches!(self, Self::Derivative)
    }
}

impl fmt::Display for EvalAspect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value => write!(f, "Value"),
            Self::HeldValue => write!(f, "HeldValue"),
            Self::Derivative => write!(f, "Derivative"),
        }
    }
}

/// Where in time to evaluate the spline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EvalLocation {
    /// Evaluate the left limit (before time).
    Pre = 0,
    /// Evaluate at exact time.
    #[default]
    AtTime = 1,
    /// Evaluate the right limit (after time).
    Post = 2,
}

impl EvalLocation {
    /// Returns true if evaluating pre-time limit.
    #[inline]
    #[must_use]
    pub fn is_pre(&self) -> bool {
        matches!(self, Self::Pre)
    }

    /// Returns true if evaluating at exact time.
    #[inline]
    #[must_use]
    pub fn is_at_time(&self) -> bool {
        matches!(self, Self::AtTime)
    }

    /// Returns true if evaluating post-time limit.
    #[inline]
    #[must_use]
    pub fn is_post(&self) -> bool {
        matches!(self, Self::Post)
    }
}

impl fmt::Display for EvalLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pre => write!(f, "Pre"),
            Self::AtTime => write!(f, "AtTime"),
            Self::Post => write!(f, "Post"),
        }
    }
}

//=============================================================================
// SAMPLE TYPES
//=============================================================================

/// A polyline vertex type marker trait.
pub trait SampleVertex:
    Clone + Copy + Default + PartialEq + fmt::Debug + Send + Sync + 'static
{
    /// Creates a vertex from time and value.
    fn from_time_value(time: f64, value: f64) -> Self;

    /// Returns the time component.
    fn time(&self) -> f64;

    /// Returns the value component.
    fn value(&self) -> f64;
}

impl SampleVertex for Vec2d {
    #[inline]
    fn from_time_value(time: f64, value: f64) -> Self {
        Self::new(time, value)
    }

    #[inline]
    fn time(&self) -> f64 {
        self[0]
    }

    #[inline]
    fn value(&self) -> f64 {
        self[1]
    }
}

impl SampleVertex for Vec2f {
    #[inline]
    fn from_time_value(time: f64, value: f64) -> Self {
        Self::new(time as f32, value as f32)
    }

    #[inline]
    fn time(&self) -> f64 {
        f64::from(self[0])
    }

    #[inline]
    fn value(&self) -> f64 {
        f64::from(self[1])
    }
}

/// A polyline represented as a sequence of vertices.
pub type Polyline<V> = Vec<V>;

/// Collection of piecewise linear polylines approximating a spline.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SplineSamples<V: SampleVertex> {
    /// Collection of polylines.
    pub polylines: Vec<Polyline<V>>,
}

impl<V: SampleVertex> SplineSamples<V> {
    /// Creates empty spline samples.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            polylines: Vec::new(),
        }
    }

    /// Clears all polylines.
    #[inline]
    pub fn clear(&mut self) {
        self.polylines.clear();
    }

    /// Returns true if there are no polylines.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.polylines.is_empty()
    }

    /// Returns the total number of vertices across all polylines.
    #[must_use]
    pub fn total_vertices(&self) -> usize {
        self.polylines.iter().map(Vec::len).sum()
    }

    /// Adds a segment to the samples.
    pub fn add_segment(&mut self, time0: f64, value0: f64, time1: f64, value1: f64) {
        let (t0, v0, t1, v1) = if time0 > time1 {
            (time1, value1, time0, value0)
        } else {
            (time0, value0, time1, value1)
        };

        let vertex0 = V::from_time_value(t0, v0);
        let vertex1 = V::from_time_value(t1, v1);

        let need_new_polyline = self.polylines.is_empty()
            || self.polylines.last().is_some_and(|p| p.is_empty())
            || self
                .polylines
                .last()
                .and_then(|p| p.last())
                .is_some_and(|last| *last != vertex0);

        if need_new_polyline {
            self.polylines.push(vec![vertex0, vertex1]);
        } else if let Some(polyline) = self.polylines.last_mut() {
            polyline.push(vertex1);
        }
    }
}

/// Collection of polylines with source information.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SplineSamplesWithSources<V: SampleVertex> {
    /// Collection of polylines.
    pub polylines: Vec<Polyline<V>>,
    /// Source for each polyline (parallel to polylines).
    pub sources: Vec<SplineSampleSource>,
}

impl<V: SampleVertex> SplineSamplesWithSources<V> {
    /// Creates empty spline samples with sources.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            polylines: Vec::new(),
            sources: Vec::new(),
        }
    }

    /// Clears all polylines and sources.
    #[inline]
    pub fn clear(&mut self) {
        self.polylines.clear();
        self.sources.clear();
    }

    /// Returns true if there are no polylines.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.polylines.is_empty()
    }

    /// Returns the total number of vertices across all polylines.
    #[must_use]
    pub fn total_vertices(&self) -> usize {
        self.polylines.iter().map(Vec::len).sum()
    }

    /// Adds a segment with source to the samples.
    pub fn add_segment_with_source(
        &mut self,
        time0: f64,
        value0: f64,
        time1: f64,
        value1: f64,
        source: SplineSampleSource,
    ) {
        let (t0, v0, t1, v1) = if time0 > time1 {
            (time1, value1, time0, value0)
        } else {
            (time0, value0, time1, value1)
        };

        let vertex0 = V::from_time_value(t0, v0);
        let vertex1 = V::from_time_value(t1, v1);

        let need_new_polyline = self.polylines.is_empty()
            || self.sources.last().is_some_and(|s| *s != source)
            || self.polylines.last().is_some_and(|p| p.is_empty())
            || self
                .polylines
                .last()
                .and_then(|p| p.last())
                .is_some_and(|last| *last != vertex0);

        if need_new_polyline {
            self.polylines.push(vec![vertex0, vertex1]);
            self.sources.push(source);
        } else if let Some(polyline) = self.polylines.last_mut() {
            polyline.push(vertex1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knot_data::KnotData;

    #[test]
    fn test_quadratic_eval() {
        // f(t) = t^2 - 2t + 1 = (t-1)^2
        let q = Quadratic {
            a: 1.0,
            b: -2.0,
            c: 1.0,
        };
        assert!((q.eval(0.0) - 1.0).abs() < 1e-10);
        assert!((q.eval(1.0) - 0.0).abs() < 1e-10);
        assert!((q.eval(2.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cubic_from_points() {
        // Linear Bezier: all points on a line
        let c = Cubic::from_points(0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0);
        assert!((c.eval(0.0) - 0.0).abs() < 1e-10);
        assert!((c.eval(1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cubic_derivative() {
        let c = Cubic {
            a: 1.0,
            b: 2.0,
            c: 3.0,
            d: 4.0,
        };
        let d = c.derivative();
        // Derivative of t^3 + 2t^2 + 3t + 4 = 3t^2 + 4t + 3
        assert!((d.a - 3.0).abs() < 1e-10);
        assert!((d.b - 4.0).abs() < 1e-10);
        assert!((d.c - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_filter_zeros() {
        // Closest to 0.5
        assert!((filter_zeros_2(0.2, 0.6) - 0.6).abs() < 1e-10);
        assert!((filter_zeros_3(0.1, 0.4, 0.9) - 0.4).abs() < 1e-10);
    }

    #[test]
    fn test_eval_aspect() {
        assert!(EvalAspect::Value.is_value());
        assert!(EvalAspect::HeldValue.is_held_value());
        assert!(EvalAspect::Derivative.is_derivative());
        assert_eq!(EvalAspect::default(), EvalAspect::Value);
    }

    #[test]
    fn test_eval_location() {
        assert!(EvalLocation::Pre.is_pre());
        assert!(EvalLocation::AtTime.is_at_time());
        assert!(EvalLocation::Post.is_post());
        assert_eq!(EvalLocation::default(), EvalLocation::AtTime);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", EvalAspect::Value), "Value");
        assert_eq!(format!("{}", EvalLocation::AtTime), "AtTime");
    }

    #[test]
    fn test_sample_vertex_vec2d() {
        let v = Vec2d::from_time_value(1.5, 2.5);
        assert!((v.time() - 1.5).abs() < 1e-10);
        assert!((v.value() - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_sample_vertex_vec2f() {
        let v = Vec2f::from_time_value(1.5, 2.5);
        assert!((v.time() - 1.5).abs() < 0.001);
        assert!((v.value() - 2.5).abs() < 0.001);
    }

    #[test]
    fn test_spline_samples_new() {
        let samples: SplineSamples<Vec2d> = SplineSamples::new();
        assert!(samples.is_empty());
        assert_eq!(samples.total_vertices(), 0);
    }

    #[test]
    fn test_spline_samples_add_segment() {
        let mut samples: SplineSamples<Vec2d> = SplineSamples::new();

        // First segment starts new polyline
        samples.add_segment(0.0, 0.0, 1.0, 1.0);
        assert_eq!(samples.polylines.len(), 1);
        assert_eq!(samples.polylines[0].len(), 2);

        // Continuous segment extends polyline
        samples.add_segment(1.0, 1.0, 2.0, 0.5);
        assert_eq!(samples.polylines.len(), 1);
        assert_eq!(samples.polylines[0].len(), 3);

        // Discontinuous segment starts new polyline
        samples.add_segment(5.0, 0.0, 6.0, 1.0);
        assert_eq!(samples.polylines.len(), 2);
    }

    #[test]
    fn test_spline_samples_reversed_times() {
        let mut samples: SplineSamples<Vec2d> = SplineSamples::new();

        // Times are reversed, should be swapped internally
        samples.add_segment(2.0, 1.0, 1.0, 0.0);
        assert_eq!(samples.polylines.len(), 1);
        assert!((samples.polylines[0][0].time() - 1.0).abs() < 1e-10);
        assert!((samples.polylines[0][1].time() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_spline_samples_clear() {
        let mut samples: SplineSamples<Vec2d> = SplineSamples::new();
        samples.add_segment(0.0, 0.0, 1.0, 1.0);
        assert!(!samples.is_empty());

        samples.clear();
        assert!(samples.is_empty());
    }

    #[test]
    fn test_spline_samples_with_sources_new() {
        let samples: SplineSamplesWithSources<Vec2d> = SplineSamplesWithSources::new();
        assert!(samples.is_empty());
        assert_eq!(samples.total_vertices(), 0);
    }

    #[test]
    fn test_spline_samples_with_sources_add() {
        let mut samples: SplineSamplesWithSources<Vec2d> = SplineSamplesWithSources::new();

        // First segment
        samples.add_segment_with_source(0.0, 0.0, 1.0, 1.0, SplineSampleSource::KnotInterp);
        assert_eq!(samples.polylines.len(), 1);
        assert_eq!(samples.sources.len(), 1);
        assert_eq!(samples.sources[0], SplineSampleSource::KnotInterp);

        // Same source, continuous - extends
        samples.add_segment_with_source(1.0, 1.0, 2.0, 0.5, SplineSampleSource::KnotInterp);
        assert_eq!(samples.polylines.len(), 1);
        assert_eq!(samples.polylines[0].len(), 3);

        // Different source - new polyline
        samples.add_segment_with_source(2.0, 0.5, 3.0, 0.0, SplineSampleSource::PostExtrap);
        assert_eq!(samples.polylines.len(), 2);
        assert_eq!(samples.sources.len(), 2);
        assert_eq!(samples.sources[1], SplineSampleSource::PostExtrap);
    }

    #[test]
    fn test_spline_samples_with_sources_clear() {
        let mut samples: SplineSamplesWithSources<Vec2d> = SplineSamplesWithSources::new();
        samples.add_segment_with_source(0.0, 0.0, 1.0, 1.0, SplineSampleSource::KnotInterp);
        assert!(!samples.is_empty());

        samples.clear();
        assert!(samples.is_empty());
        assert!(samples.sources.is_empty());
    }

    #[test]
    fn test_total_vertices() {
        let mut samples: SplineSamples<Vec2d> = SplineSamples::new();

        samples.add_segment(0.0, 0.0, 1.0, 1.0);
        samples.add_segment(1.0, 1.0, 2.0, 0.5);
        // First polyline: 3 vertices

        samples.add_segment(5.0, 0.0, 6.0, 1.0);
        // Second polyline: 2 vertices

        assert_eq!(samples.total_vertices(), 5);
    }

    #[test]
    fn test_hermite_eval_linear() {
        // Hermite through linear points (0,0) to (1,1) with slope 1
        let begin = TypedKnotData::<f64> {
            base: KnotData {
                time: 0.0,
                ..Default::default()
            },
            value: 0.0,
            post_tan_slope: 1.0,
            ..Default::default()
        };
        let end = TypedKnotData::<f64> {
            base: KnotData {
                time: 1.0,
                ..Default::default()
            },
            value: 1.0,
            pre_value: 1.0,
            pre_tan_slope: 1.0,
            ..Default::default()
        };

        // Should pass through midpoint (0.5, 0.5)
        let v = eval_hermite(&begin, &end, 0.5, EvalAspect::Value);
        assert!((v - 0.5).abs() < 1e-10);
    }
}
