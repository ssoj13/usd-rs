//! Port of testUsdGeomExtentFromPlugins.py
//!
//! Tests extent computation from plugin functions for built-in geometry types
//! (Capsule, Cone, Cube, Cylinder, Sphere) at both default and time-sampled values.

use std::path::PathBuf;
use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_geom::*;
use usd_gf::vec3::Vec3f;
use usd_sdf::TimeCode;

const TOLERANCE: f32 = 0.00001;

fn testenv_path() -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("testenv");
    p.push("testUsdGeomExtentFromPlugins");
    p.push("test.usda");
    p.to_string_lossy().to_string()
}

fn open_stage() -> Arc<Stage> {
    usd_sdf::init();
    Stage::open(testenv_path(), InitialLoadSet::LoadAll).expect("Failed to open test stage")
}

fn vec3f_close(a: &Vec3f, b: &Vec3f, tol: f32) -> bool {
    (a.x - b.x).abs() < tol && (a.y - b.y).abs() < tol && (a.z - b.z).abs() < tol
}

fn assert_extent_eq(computed: &[Vec3f; 2], expected: &[Vec3f; 2], prim_path: &str) {
    assert!(
        vec3f_close(&computed[0], &expected[0], TOLERANCE),
        "{prim_path}: min mismatch: {:?} vs {:?}",
        computed[0],
        expected[0]
    );
    assert!(
        vec3f_close(&computed[1], &expected[1], TOLERANCE),
        "{prim_path}: max mismatch: {:?} vs {:?}",
        computed[1],
        expected[1]
    );
}

// ============================================================================
// test_default - extent at default time
// ============================================================================

#[test]
fn test_default() {
    let stage = open_stage();
    let tc = TimeCode::default_time();

    // Expected extents at default time (from the Python test)
    let cases: &[(&str, [Vec3f; 2])] = &[
        (
            "/capsule",
            [Vec3f::new(-2.0, -2.0, -3.0), Vec3f::new(2.0, 2.0, 3.0)],
        ),
        (
            "/cone",
            [Vec3f::new(-2.0, -2.0, -2.0), Vec3f::new(2.0, 2.0, 2.0)],
        ),
        (
            "/cube",
            [Vec3f::new(-2.0, -2.0, -2.0), Vec3f::new(2.0, 2.0, 2.0)],
        ),
        (
            "/cylinder",
            [Vec3f::new(-2.0, -2.0, -2.0), Vec3f::new(2.0, 2.0, 2.0)],
        ),
        (
            "/sphere",
            [Vec3f::new(-2.0, -2.0, -2.0), Vec3f::new(2.0, 2.0, 2.0)],
        ),
    ];

    for (prim_path, expected) in cases {
        let path = usd_sdf::Path::from_string(prim_path).expect("bad path");
        let prim = stage.get_prim_at_path(&path).expect("prim not found");
        let boundable = Boundable::new(prim);
        let extent = compute_extent_from_plugins(&boundable, tc, None)
            .unwrap_or_else(|| panic!("compute_extent_from_plugins returned None for {prim_path}"));
        assert_extent_eq(&extent, expected, prim_path);
    }
}

// ============================================================================
// test_time_sampled - extent at time=2.0
// ============================================================================

#[test]
fn test_time_sampled() {
    let stage = open_stage();
    let tc = TimeCode::new(2.0);

    // Expected extents at time=2.0 (from the Python test)
    // capsule: radius=4, height=4, axis=Z -> half_height = 4/2 + 4 = 6
    // cone: radius=4, height=6, axis=Z -> half_height=3
    // cube: size=6 -> half=3
    // cylinder: radius=4, height=6, axis=Z -> half_height=3
    // sphere: radius=4
    let cases: &[(&str, [Vec3f; 2])] = &[
        (
            "/capsule",
            [Vec3f::new(-4.0, -4.0, -6.0), Vec3f::new(4.0, 4.0, 6.0)],
        ),
        (
            "/cone",
            [Vec3f::new(-4.0, -4.0, -3.0), Vec3f::new(4.0, 4.0, 3.0)],
        ),
        (
            "/cube",
            [Vec3f::new(-3.0, -3.0, -3.0), Vec3f::new(3.0, 3.0, 3.0)],
        ),
        (
            "/cylinder",
            [Vec3f::new(-4.0, -4.0, -3.0), Vec3f::new(4.0, 4.0, 3.0)],
        ),
        (
            "/sphere",
            [Vec3f::new(-4.0, -4.0, -4.0), Vec3f::new(4.0, 4.0, 4.0)],
        ),
    ];

    for (prim_path, expected) in cases {
        let path = usd_sdf::Path::from_string(prim_path).expect("bad path");
        let prim = stage.get_prim_at_path(&path).expect("prim not found");
        let boundable = Boundable::new(prim);
        let extent = compute_extent_from_plugins(&boundable, tc, None)
            .unwrap_or_else(|| panic!("compute_extent_from_plugins returned None for {prim_path}"));
        assert_extent_eq(&extent, expected, prim_path);
    }
}

// ============================================================================
// test_compute_extent_and_extent_from_plugin
// ============================================================================

#[test]
fn test_compute_extent_and_extent_from_plugin() {
    let stage = open_stage();
    let tc = TimeCode::default_time();

    // /AuthoredExtentSphere has authored extent=(-10,-10,-10)..(10,10,10) and radius=2
    let authored_extent = [
        Vec3f::new(-10.0, -10.0, -10.0),
        Vec3f::new(10.0, 10.0, 10.0),
    ];
    let explicitly_computed_extent = [Vec3f::new(-2.0, -2.0, -2.0), Vec3f::new(2.0, 2.0, 2.0)];

    // ComputeExtent on AuthoredExtentSphere should return authored extent
    let path = usd_sdf::Path::from_string("/AuthoredExtentSphere").expect("bad path");
    let prim = stage.get_prim_at_path(&path).expect("prim not found");
    let boundable = Boundable::new(prim);
    let extent1 = boundable
        .compute_extent(tc)
        .expect("compute_extent returned None");
    assert!(
        vec3f_close(&extent1[0], &authored_extent[0], TOLERANCE),
        "Authored extent min mismatch: {:?} vs {:?}",
        extent1[0],
        authored_extent[0]
    );
    assert!(
        vec3f_close(&extent1[1], &authored_extent[1], TOLERANCE),
        "Authored extent max mismatch: {:?} vs {:?}",
        extent1[1],
        authored_extent[1]
    );

    // ComputeExtentFromPlugins ignores authored extent, computes from geometry
    let extent2 = compute_extent_from_plugins(&boundable, tc, None)
        .expect("compute_extent_from_plugins returned None");
    assert_extent_eq(
        &extent2,
        &explicitly_computed_extent,
        "/AuthoredExtentSphere",
    );

    // ComputeExtent on /sphere (no authored extent) should compute from geometry
    let path = usd_sdf::Path::from_string("/sphere").expect("bad path");
    let prim = stage.get_prim_at_path(&path).expect("prim not found");
    let boundable = Boundable::new(prim);
    let extent = boundable
        .compute_extent(tc)
        .expect("compute_extent returned None");
    assert!(
        vec3f_close(&extent[0], &explicitly_computed_extent[0], TOLERANCE),
        "/sphere extent min mismatch: {:?} vs {:?}",
        extent[0],
        explicitly_computed_extent[0]
    );
    assert!(
        vec3f_close(&extent[1], &explicitly_computed_extent[1], TOLERANCE),
        "/sphere extent max mismatch: {:?} vs {:?}",
        extent[1],
        explicitly_computed_extent[1]
    );
}
