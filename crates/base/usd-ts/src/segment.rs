//! Ts_Segment - Spline segment between two knots.
//!
//! Port of pxr/base/ts/segment.h

use super::types::{CurveType, InterpMode, TsTime};
use usd_gf::Vec2d;

/// Interpolation type for a segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SegmentInterp {
    /// This segment explicitly has no value.
    #[default]
    ValueBlock = 0,
    /// The value is always the starting value.
    Held = 1,
    /// Interpolate linearly from start to end.
    Linear = 2,
    /// The segment uses Bezier interpolation.
    Bezier = 3,
    /// The segment uses Hermite interpolation.
    Hermite = 4,
    /// Linear extrapolation from -infinity to end value with fixed slope.
    PreExtrap = 5,
    /// Linear extrapolation to +infinity from start value with fixed slope.
    PostExtrap = 6,
}

impl SegmentInterp {
    /// Returns true if this is a curve interpolation (Bezier or Hermite).
    #[inline]
    pub fn is_curve(&self) -> bool {
        matches!(self, Self::Bezier | Self::Hermite)
    }

    /// Returns true if this is an extrapolation segment.
    #[inline]
    pub fn is_extrap(&self) -> bool {
        matches!(self, Self::PreExtrap | Self::PostExtrap)
    }

    /// Creates from InterpMode and CurveType.
    pub fn from_interp_and_curve(interp: InterpMode, curve_type: CurveType) -> Self {
        match interp {
            InterpMode::ValueBlock => Self::ValueBlock,
            InterpMode::Held => Self::Held,
            InterpMode::Linear => Self::Linear,
            InterpMode::Curve => match curve_type {
                CurveType::Bezier => Self::Bezier,
                CurveType::Hermite => Self::Hermite,
            },
        }
    }
}

/// A segment of a spline between two knots.
///
/// Contains the post-side values of one knot and pre-side of next knot.
/// Data stored as 4 Vec2d points: knot points and tangent endpoints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    /// Start point (time, value).
    pub p0: Vec2d,
    /// Start tangent endpoint.
    pub t0: Vec2d,
    /// End tangent endpoint.
    pub t1: Vec2d,
    /// End point (time, value).
    pub p1: Vec2d,
    /// Interpolation type.
    pub interp: SegmentInterp,
}

impl Default for Segment {
    fn default() -> Self {
        Self {
            p0: Vec2d::zero(),
            t0: Vec2d::zero(),
            t1: Vec2d::zero(),
            p1: Vec2d::zero(),
            interp: SegmentInterp::ValueBlock,
        }
    }
}

impl Segment {
    /// Creates a new segment with given endpoints and interpolation.
    pub fn new(p0: Vec2d, p1: Vec2d, interp: SegmentInterp) -> Self {
        Self {
            p0,
            t0: p0,
            t1: p1,
            p1,
            interp,
        }
    }

    /// Creates a linear segment between two points.
    pub fn linear(t0: TsTime, v0: f64, t1: TsTime, v1: f64) -> Self {
        Self {
            p0: Vec2d::new(t0, v0),
            t0: Vec2d::new(t0, v0),
            t1: Vec2d::new(t1, v1),
            p1: Vec2d::new(t1, v1),
            interp: SegmentInterp::Linear,
        }
    }

    /// Creates a held segment.
    pub fn held(t0: TsTime, v0: f64, t1: TsTime, v1: f64) -> Self {
        Self {
            p0: Vec2d::new(t0, v0),
            t0: Vec2d::new(t0, v0),
            t1: Vec2d::new(t1, v1),
            p1: Vec2d::new(t1, v1),
            interp: SegmentInterp::Held,
        }
    }

    /// Creates a Bezier segment with control points.
    pub fn bezier(p0: Vec2d, t0: Vec2d, t1: Vec2d, p1: Vec2d) -> Self {
        Self {
            p0,
            t0,
            t1,
            p1,
            interp: SegmentInterp::Bezier,
        }
    }

    /// Creates a Hermite segment with tangent slopes.
    pub fn hermite(t0: TsTime, v0: f64, slope0: f64, t1: TsTime, v1: f64, slope1: f64) -> Self {
        let dt = t1 - t0;
        let third = dt / 3.0;
        Self {
            p0: Vec2d::new(t0, v0),
            t0: Vec2d::new(t0 + third, v0 + slope0 * third),
            t1: Vec2d::new(t1 - third, v1 - slope1 * third),
            p1: Vec2d::new(t1, v1),
            interp: SegmentInterp::Hermite,
        }
    }

    /// Creates a pre-extrapolation segment.
    pub fn pre_extrap(end_time: TsTime, end_value: f64, slope: f64) -> Self {
        Self {
            p0: Vec2d::new(f64::NEG_INFINITY, slope),
            t0: Vec2d::zero(),
            t1: Vec2d::zero(),
            p1: Vec2d::new(end_time, end_value),
            interp: SegmentInterp::PreExtrap,
        }
    }

    /// Creates a post-extrapolation segment.
    pub fn post_extrap(start_time: TsTime, start_value: f64, slope: f64) -> Self {
        Self {
            p0: Vec2d::new(start_time, start_value),
            t0: Vec2d::zero(),
            t1: Vec2d::zero(),
            p1: Vec2d::new(f64::INFINITY, slope),
            interp: SegmentInterp::PostExtrap,
        }
    }

    /// Sets interpolation from InterpMode and CurveType.
    pub fn set_interp(&mut self, interp_mode: InterpMode, curve_type: CurveType) {
        self.interp = SegmentInterp::from_interp_and_curve(interp_mode, curve_type);
    }

    /// Returns the start time.
    #[inline]
    pub fn start_time(&self) -> TsTime {
        self.p0[0]
    }

    /// Returns the end time.
    #[inline]
    pub fn end_time(&self) -> TsTime {
        self.p1[0]
    }

    /// Returns the start value.
    #[inline]
    pub fn start_value(&self) -> f64 {
        self.p0[1]
    }

    /// Returns the end value.
    #[inline]
    pub fn end_value(&self) -> f64 {
        self.p1[1]
    }

    /// Returns the time span of the segment.
    #[inline]
    pub fn time_span(&self) -> TsTime {
        self.p1[0] - self.p0[0]
    }

    /// Evaluates the segment at the given time.
    pub fn eval(&self, time: TsTime) -> f64 {
        match self.interp {
            SegmentInterp::ValueBlock => f64::NAN,
            SegmentInterp::Held => self.p0[1],
            SegmentInterp::Linear => self.eval_linear(time),
            SegmentInterp::Bezier | SegmentInterp::Hermite => self.eval_bezier(time),
            SegmentInterp::PreExtrap => {
                let slope = self.p0[1];
                self.p1[1] + slope * (time - self.p1[0])
            }
            SegmentInterp::PostExtrap => {
                let slope = self.p1[1];
                self.p0[1] + slope * (time - self.p0[0])
            }
        }
    }

    fn eval_linear(&self, time: TsTime) -> f64 {
        let t0 = self.p0[0];
        let t1 = self.p1[0];
        let v0 = self.p0[1];
        let v1 = self.p1[1];

        if (t1 - t0).abs() < 1e-10 {
            return v0;
        }

        let t = (time - t0) / (t1 - t0);
        v0 + t * (v1 - v0)
    }

    fn eval_bezier(&self, time: TsTime) -> f64 {
        let t0 = self.p0[0];
        let t1 = self.p1[0];

        if (t1 - t0).abs() < 1e-10 {
            return self.p0[1];
        }

        // Find parameter u such that bezier_x(u) = time
        // Using Newton-Raphson iteration
        let mut u = (time - t0) / (t1 - t0);
        u = u.clamp(0.0, 1.0);

        for _ in 0..10 {
            let x = self.bezier_x(u);
            let dx = self.bezier_dx(u);
            if dx.abs() < 1e-10 {
                break;
            }
            let new_u = u - (x - time) / dx;
            if (new_u - u).abs() < 1e-10 {
                break;
            }
            u = new_u.clamp(0.0, 1.0);
        }

        self.bezier_y(u)
    }

    fn bezier_x(&self, u: f64) -> f64 {
        let u2 = u * u;
        let u3 = u2 * u;
        let inv = 1.0 - u;
        let inv2 = inv * inv;
        let inv3 = inv2 * inv;

        inv3 * self.p0[0]
            + 3.0 * inv2 * u * self.t0[0]
            + 3.0 * inv * u2 * self.t1[0]
            + u3 * self.p1[0]
    }

    fn bezier_y(&self, u: f64) -> f64 {
        let u2 = u * u;
        let u3 = u2 * u;
        let inv = 1.0 - u;
        let inv2 = inv * inv;
        let inv3 = inv2 * inv;

        inv3 * self.p0[1]
            + 3.0 * inv2 * u * self.t0[1]
            + 3.0 * inv * u2 * self.t1[1]
            + u3 * self.p1[1]
    }

    fn bezier_dx(&self, u: f64) -> f64 {
        let u2 = u * u;
        let inv = 1.0 - u;
        let inv2 = inv * inv;

        3.0 * inv2 * (self.t0[0] - self.p0[0])
            + 6.0 * inv * u * (self.t1[0] - self.t0[0])
            + 3.0 * u2 * (self.p1[0] - self.t1[0])
    }

    /// Evaluates the derivative at the given time.
    pub fn eval_derivative(&self, time: TsTime) -> f64 {
        match self.interp {
            SegmentInterp::ValueBlock => 0.0,
            SegmentInterp::Held => 0.0,
            SegmentInterp::Linear => {
                let dt = self.p1[0] - self.p0[0];
                if dt.abs() < 1e-10 {
                    0.0
                } else {
                    (self.p1[1] - self.p0[1]) / dt
                }
            }
            SegmentInterp::Bezier | SegmentInterp::Hermite => {
                // Numerical derivative
                let eps = 1e-6;
                let v0 = self.eval(time - eps);
                let v1 = self.eval(time + eps);
                (v1 - v0) / (2.0 * eps)
            }
            SegmentInterp::PreExtrap => self.p0[1],  // slope
            SegmentInterp::PostExtrap => self.p1[1], // slope
        }
    }

    /// Adds a time/value delta to all points.
    pub fn offset(&mut self, delta: Vec2d) {
        self.p0 += delta;
        self.t0 += delta;
        self.t1 += delta;
        self.p1 += delta;
    }

    /// Adds a time delta to all points.
    pub fn offset_time(&mut self, delta: TsTime) {
        self.p0[0] += delta;
        self.t0[0] += delta;
        self.t1[0] += delta;
        self.p1[0] += delta;
    }

    /// Adds a value delta to all points.
    pub fn offset_value(&mut self, delta: f64) {
        self.p0[1] += delta;
        self.t0[1] += delta;
        self.t1[1] += delta;
        self.p1[1] += delta;
    }

    /// Scales time by the given factor.
    pub fn scale_time(&mut self, factor: f64) {
        self.p0[0] *= factor;
        self.t0[0] *= factor;
        self.t1[0] *= factor;
        self.p1[0] *= factor;
    }

    /// Scales value by the given factor.
    pub fn scale_value(&mut self, factor: f64) {
        self.p0[1] *= factor;
        self.t0[1] *= factor;
        self.t1[1] *= factor;
        self.p1[1] *= factor;
    }

    /// Returns true if the given time is within the segment's range.
    pub fn contains_time(&self, time: TsTime) -> bool {
        time >= self.p0[0] && time <= self.p1[0]
    }
}

impl Segment {
    /// Computes the derivative at parameter u (0 to 1).
    ///
    /// For Bezier/Hermite curves, computes dy/dt at the given parametric position.
    /// For other interpolation types, returns the constant slope.
    pub fn compute_derivative(&self, u: f64) -> f64 {
        match self.interp {
            SegmentInterp::ValueBlock | SegmentInterp::Held => 0.0,
            SegmentInterp::Linear => {
                let dt = self.p1[0] - self.p0[0];
                if dt.abs() < 1e-10 {
                    0.0
                } else {
                    (self.p1[1] - self.p0[1]) / dt
                }
            }
            SegmentInterp::Bezier | SegmentInterp::Hermite => {
                // Derivative of Bezier: 3 * [(1-u)^2 * (P1-P0) + 2*(1-u)*u*(P2-P1) + u^2*(P3-P2)]
                let inv = 1.0 - u;
                let inv2 = inv * inv;
                let u2 = u * u;

                // dx/du
                let dx = 3.0 * inv2 * (self.t0[0] - self.p0[0])
                    + 6.0 * inv * u * (self.t1[0] - self.t0[0])
                    + 3.0 * u2 * (self.p1[0] - self.t1[0]);

                // dy/du
                let dy = 3.0 * inv2 * (self.t0[1] - self.p0[1])
                    + 6.0 * inv * u * (self.t1[1] - self.t0[1])
                    + 3.0 * u2 * (self.p1[1] - self.t1[1]);

                // dy/dt = (dy/du) / (dx/du)
                if dx.abs() < 1e-10 { 0.0 } else { dy / dx }
            }
            SegmentInterp::PreExtrap => self.p0[1], // slope stored in p0[1]
            SegmentInterp::PostExtrap => self.p1[1], // slope stored in p1[1]
        }
    }

    /// Negates the segment (for oscillating loops).
    pub fn negate(&self) -> Self {
        Self {
            p0: Vec2d::new(-self.p0[0], self.p0[1]),
            t0: Vec2d::new(-self.t0[0], self.t0[1]),
            t1: Vec2d::new(-self.t1[0], self.t1[1]),
            p1: Vec2d::new(-self.p1[0], self.p1[1]),
            interp: self.interp,
        }
    }

    /// Returns (segment - shift2) for oscillating loops.
    pub fn sub_shift(&self, shift: f64) -> Self {
        Self {
            p0: Vec2d::new(self.p0[0] - shift, self.p0[1]),
            t0: Vec2d::new(self.t0[0] - shift, self.t0[1]),
            t1: Vec2d::new(self.t1[0] - shift, self.t1[1]),
            p1: Vec2d::new(self.p1[0] - shift, self.p1[1]),
            interp: self.interp,
        }
    }

    /// Returns -segment + shift for oscillating loops.
    pub fn negate_add_shift(&self, shift: f64) -> Self {
        Self {
            p0: Vec2d::new(-self.p0[0] + shift, self.p0[1]),
            t0: Vec2d::new(-self.t0[0] + shift, self.t0[1]),
            t1: Vec2d::new(-self.t1[0] + shift, self.t1[1]),
            p1: Vec2d::new(-self.p1[0] + shift, self.p1[1]),
            interp: self.interp,
        }
    }

    /// Applies -(segment - shift2) + shift1 transformation for oscillating loops.
    pub fn transform_oscillate(&self, shift1: f64, shift2: f64) -> Self {
        Self {
            p0: Vec2d::new(-(self.p0[0] - shift2) + shift1, self.p0[1]),
            t0: Vec2d::new(-(self.t0[0] - shift2) + shift1, self.t0[1]),
            t1: Vec2d::new(-(self.t1[0] - shift2) + shift1, self.t1[1]),
            p1: Vec2d::new(-(self.p1[0] - shift2) + shift1, self.p1[1]),
            interp: self.interp,
        }
    }
}

impl std::ops::AddAssign<Vec2d> for Segment {
    fn add_assign(&mut self, delta: Vec2d) {
        self.offset(delta);
    }
}

impl std::ops::AddAssign<f64> for Segment {
    fn add_assign(&mut self, delta: f64) {
        self.offset_time(delta);
    }
}
