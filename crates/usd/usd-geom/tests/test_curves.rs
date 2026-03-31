//! Tests for UsdGeomCurves, UsdGeomBasisCurves, and UsdGeomNurbsCurves.
//!
//! Ported from:
//!   testenv/testUsdGeomCurves.py
//!   testenv/testUsdGeomBasisCurves.py

use std::path::PathBuf;
use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_geom::*;
use usd_sdf::TimeCode;

// ============================================================================
// Helpers
// ============================================================================

fn testenv_path(subdir: &str, file: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("testenv");
    path.push(subdir);
    path.push(file);
    path.to_string_lossy().to_string()
}

fn open_stage(subdir: &str, file: &str) -> Arc<Stage> {
    usd_sdf::init();
    Stage::open(testenv_path(subdir, file), InitialLoadSet::LoadAll)
        .expect("Failed to open test stage")
}

fn in_memory_stage() -> Arc<Stage> {
    Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap()
}

fn path(s: &str) -> usd_sdf::Path {
    usd_sdf::Path::from_string(s).unwrap()
}

// ============================================================================
// testUsdGeomCurves.py :: TestBasisCurves
// ============================================================================

#[test]
fn test_basis_curves_create() {
    let stage = in_memory_stage();
    let schema = BasisCurves::define(&stage, &path("/TestCurves"));
    assert!(schema.is_valid());
    assert_eq!(schema.prim().get_name().as_str(), "TestCurves");
}

#[test]
fn test_basis_curves_schema_attribute_names() {
    let stage = in_memory_stage();
    let schema = BasisCurves::define(&stage, &path("/TestCurves"));
    assert!(schema.is_valid());

    let attr_names = BasisCurves::get_schema_attribute_names(true);
    let name_strs: Vec<&str> = attr_names.iter().map(|t| t.as_str()).collect();

    assert!(name_strs.contains(&"widths"), "missing 'widths'");
    assert!(name_strs.contains(&"points"), "missing 'points'");
    assert!(name_strs.contains(&"normals"), "missing 'normals'");
    assert!(
        name_strs.contains(&"curveVertexCounts"),
        "missing 'curveVertexCounts'"
    );
    assert!(name_strs.contains(&"type"), "missing 'type'");
    assert!(name_strs.contains(&"basis"), "missing 'basis'");
    assert!(name_strs.contains(&"wrap"), "missing 'wrap'");

    let basis_attr = schema.get_basis_attr();
    assert!(basis_attr.is_valid());
}

// ============================================================================
// testUsdGeomCurves.py :: TestNurbsCurves
// ============================================================================

#[test]
fn test_nurbs_curves_create() {
    let stage = in_memory_stage();
    let schema = NurbsCurves::define(&stage, &path("/TestCurves"));
    assert!(schema.is_valid());
    assert_eq!(schema.prim().get_name().as_str(), "TestCurves");
}

#[test]
fn test_nurbs_curves_schema_attribute_names() {
    let stage = in_memory_stage();
    let schema = NurbsCurves::define(&stage, &path("/TestCurves"));
    assert!(schema.is_valid());

    let attr_names = NurbsCurves::get_schema_attribute_names(true);
    let name_strs: Vec<&str> = attr_names.iter().map(|t| t.as_str()).collect();

    assert!(name_strs.contains(&"widths"), "missing 'widths'");
    assert!(name_strs.contains(&"points"), "missing 'points'");
    assert!(name_strs.contains(&"normals"), "missing 'normals'");
    assert!(
        name_strs.contains(&"curveVertexCounts"),
        "missing 'curveVertexCounts'"
    );
    assert!(name_strs.contains(&"knots"), "missing 'knots'");
    assert!(name_strs.contains(&"order"), "missing 'order'");
    assert!(
        name_strs.contains(&"pointWeights"),
        "missing 'pointWeights'"
    );

    let knots_attr = schema.get_knots_attr();
    assert!(knots_attr.is_valid());
}

// ============================================================================
// testUsdGeomBasisCurves.py :: test_InterpolationTypes
// ============================================================================

#[test]
fn test_interpolation_types() {
    let stage = open_stage("testUsdGeomBasisCurves", "basisCurves.usda");
    let tc = TimeCode::default_time();
    let c = BasisCurves::get(&stage, &path("/BezierCubic"));
    assert!(c.is_valid());

    assert_eq!(c.compute_uniform_data_size(tc), 2);
    assert_eq!(c.compute_varying_data_size(tc), 4);
    assert_eq!(c.compute_vertex_data_size(tc), 10);

    let tokens = usd_geom_tokens();

    assert_eq!(
        c.compute_interpolation_for_size(1, tc, None).as_str(),
        tokens.constant.as_str()
    );
    assert_eq!(
        c.compute_interpolation_for_size(2, tc, None).as_str(),
        tokens.uniform.as_str()
    );
    assert_eq!(
        c.compute_interpolation_for_size(4, tc, None).as_str(),
        tokens.varying.as_str()
    );
    assert_eq!(
        c.compute_interpolation_for_size(10, tc, None).as_str(),
        tokens.vertex.as_str()
    );

    // No match -> empty token
    assert!(
        c.compute_interpolation_for_size(100, tc, None)
            .as_str()
            .is_empty()
    );
    assert!(
        c.compute_interpolation_for_size(0, tc, None)
            .as_str()
            .is_empty()
    );
}

// ============================================================================
// testUsdGeomBasisCurves.py :: test_ComputeCurveCount
// ============================================================================

#[test]
fn test_compute_curve_count() {
    let stage = open_stage("testUsdGeomBasisCurves", "basisCurves.usda");
    let earliest = TimeCode::new(f64::MIN);
    let default_tc = TimeCode::default_time();

    // Time-sampled queries
    let test_time_samples: Vec<(&str, TimeCode, usize)> = vec![
        ("/UnsetVertexCounts", earliest, 0),
        ("/BlockedVertexCounts", earliest, 0),
        ("/EmptyVertexCounts", earliest, 0),
        ("/TimeSampledVertexCounts", earliest, 3),
        ("/TimeSampledAndDefaultVertexCounts", earliest, 5),
    ];

    for (prim_path, time_code, expected) in &test_time_samples {
        let schema = BasisCurves::get(&stage, &path(prim_path));
        assert!(schema.is_valid(), "Schema at {prim_path} should be valid");
        assert_eq!(
            schema.curves().get_curve_count(*time_code),
            *expected,
            "GetCurveCount({prim_path}, earliest) should be {expected}"
        );
    }

    // Default-time queries
    let test_defaults: Vec<(&str, usize)> = vec![
        ("/UnsetVertexCounts", 0),
        ("/BlockedVertexCounts", 0),
        ("/EmptyVertexCounts", 0),
        ("/TimeSampledVertexCounts", 0),
        ("/TimeSampledAndDefaultVertexCounts", 4),
    ];

    for (prim_path, expected) in &test_defaults {
        let schema = BasisCurves::get(&stage, &path(prim_path));
        assert!(schema.is_valid(), "Schema at {prim_path} should be valid");
        assert_eq!(
            schema.curves().get_curve_count(default_tc),
            *expected,
            "GetCurveCount({prim_path}, default) should be {expected}"
        );
    }

    // Invalid prim -> invalid schema
    let invalid = BasisCurves::invalid();
    assert!(!invalid.is_valid());
}

// ============================================================================
// testUsdGeomBasisCurves.py :: test_ComputeSegmentCount
// ============================================================================

#[test]
fn test_compute_segment_counts() {
    let stage = open_stage("testUsdGeomBasisCurves", "basisCurves.usda");
    let earliest = TimeCode::new(f64::MIN);

    let data: Vec<(&str, Vec<i32>)> = vec![
        ("/LinearNonperiodic", vec![1, 2, 1, 4]),
        ("/LinearPeriodic", vec![3, 7]),
        ("/CubicNonperiodicBezier", vec![1, 2, 3, 1, 2]),
        ("/CubicNonperiodicBspline", vec![2, 1, 3, 4]),
        ("/CubicPeriodicBezier", vec![2, 3, 2]),
        ("/CubicPeriodicBspline", vec![6, 9, 6]),
        ("/CubicPinnedCatmullRom", vec![1, 2, 1, 4]),
        ("/CubicPinnedBezier", vec![1, 2, 3, 1, 2]),
    ];

    for (prim_path, expected_segments) in &data {
        let curve = BasisCurves::get(&stage, &path(prim_path));
        assert!(curve.is_valid(), "Curve at {prim_path} should be valid");

        let segments = curve.compute_segment_counts(earliest);
        assert_eq!(
            &segments, expected_segments,
            "ComputeSegmentCounts({prim_path}) = {segments:?}, expected {expected_segments:?}"
        );
    }
}
