//! Timecode Splines (ts) - Spline-based animation system.
//!
//! This module provides spline-based animation curves for USD.
//! Splines define how values change over time, supporting various
//! interpolation and extrapolation modes.
//!
//! # Key Concepts
//!
//! - **Knots**: Control points at specific times with values and tangents
//! - **Segments**: Regions between knots with interpolation
//! - **Extrapolation**: Value behavior beyond the defined knot range
//! - **Inner Loops**: Repeating patterns within a spline
//!
//! # Interpolation Modes
//!
//! - `Held`: Constant value (step function)
//! - `Linear`: Straight-line interpolation
//! - `Curve`: Bezier or Hermite curves
//!
//! # Examples
//!
//! ```
//! use usd_ts::{InterpMode, ExtrapMode, Extrapolation, LoopParams};
//!
//! // Basic interpolation
//! let interp = InterpMode::Curve;
//!
//! // Extrapolation with looping
//! let extrap = Extrapolation::new(ExtrapMode::LoopRepeat);
//! assert!(extrap.is_looping());
//!
//! // Inner loop setup
//! let loops = LoopParams::new(0.0, 24.0)
//!     .with_pre_loops(1)
//!     .with_post_loops(2);
//! assert!(loops.is_enabled());
//! ```

pub mod binary;
pub mod debug_codes;
pub mod diff;
pub mod eval;
pub mod iterator;
pub mod knot;
pub mod knot_data;
pub mod knot_map;
pub mod raii;
pub mod regression_preventer;
pub mod sample;
pub mod segment;
pub mod spline;
pub mod spline_data;
pub mod tangent_conversions;
pub mod type_helpers;
pub mod types;
pub mod value_type_dispatch;

// Re-export main types
pub use binary::{
    BinaryDataAccess, BinaryError, CURRENT_BINARY_VERSION, check_binary_version,
    current_binary_version,
};
pub use diff::{diff as spline_diff, splines_equal, splines_exactly_equal};
pub use eval::{
    EvalAspect, EvalLocation, Polyline, SampleVertex, SplineSamples, SplineSamplesWithSources,
};
pub use iterator::{
    SegmentIterator, SegmentKnotIterator, SegmentLoopIterator, SegmentPrototypeIterator,
};
pub use knot::{DoubleKnot, FloatKnot, HalfKnot, Knot, Tangent, TypedKnot};
pub use knot_data::{AnyKnotData, KnotData, KnotValueType, TypedKnotData};
pub use knot_map::KnotMap;
pub use raii::{AntiRegressionAuthoringSelector, EditBehaviorBlock};
pub use regression_preventer::{
    InteractiveMode, RegressionPreventer, RegressionPreventerBatch, SetResult,
};
pub use sample::{SampleDataInterface, bake_spline, sample_spline, sample_spline_with_sources};
pub use segment::{Segment, SegmentInterp};
pub use spline::Spline;
pub use spline_data::{CustomData, CustomValue, OrderedTime, SplineData, TypedSplineData};
pub use type_helpers::{SupportedValueType, get_type_from_name, get_type_name, is_finite};
pub use types::{
    AntiRegressionMode, CurveType, ExtrapMode, Extrapolation, InterpMode, LoopParams,
    SplineSampleSource, TangentAlgorithm, TsTime,
};
pub use value_type_dispatch::{
    ValueTypeOperation, ValueTypeVisitor, dispatch_to_value_type, dispatch_visitor, dispatch_with,
};

#[cfg(test)]
mod tests {
    use super::*;
    use usd_gf::Vec2d;

    #[test]
    fn test_exports() {
        // Verify all exports are accessible
        let _interp = InterpMode::Curve;
        let _curve = CurveType::Bezier;
        let _extrap = ExtrapMode::Held;
        let _source = SplineSampleSource::KnotInterp;
        let _algo = TangentAlgorithm::None;
        let _mode = AntiRegressionMode::None;
        let _params = LoopParams::default();
        let _ex = Extrapolation::held();

        // Knot data types
        let _knot = KnotData::new();
        let _typed = TypedKnotData::<f64>::new();
        let _any = AnyKnotData::new_double();
        let _vtype = KnotValueType::Double;

        // Eval types
        let _aspect = EvalAspect::Value;
        let _location = EvalLocation::AtTime;
        let _samples: SplineSamples<Vec2d> = SplineSamples::new();
        let _with_src: SplineSamplesWithSources<Vec2d> = SplineSamplesWithSources::new();
    }

    #[test]
    fn test_tangent_conversions() {
        use tangent_conversions::{maya_to_standard, standard_to_maya};

        // Roundtrip test
        let (maya_w, maya_h) = standard_to_maya(1.0, 2.0);
        let (w, s) = maya_to_standard(maya_w, maya_h);
        assert!((w - 1.0).abs() < 1e-10);
        assert!((s - 2.0).abs() < 1e-10);
    }
}
