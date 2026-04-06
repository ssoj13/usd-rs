use std::sync::Once;

static INIT: Once = Once::new();
fn setup() {
    INIT.call_once(|| usd_sdf::init());
}

//! Tests for UsdGeomTetMesh.
//!
//! Ported from: testenv/testUsdGeomTetMesh.py

use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_geom::{TetMesh, usd_geom_tokens};
use usd_gf::vec3::Vec3f;
use usd_gf::vec4::Vec4i;
use usd_sdf::TimeCode;
use usd_vt::Value;

fn stage() -> Arc<Stage> {
    Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap()
}

fn path(s: &str) -> usd_sdf::Path {
    usd_sdf::Path::from_string(s).unwrap()
}

// ============================================================================
// test_ComputeSurfaceExtractionFromUsdGeomTetMeshRightHanded
// ============================================================================

/// Time-varying topology and surface computation for a rightHanded tet mesh.
#[test]
fn test_compute_surface_extraction_right_handed() {
    setup();
    let s = stage();
    let tet_mesh = TetMesh::define(&s, &path("/tetMesh"));
    assert!(tet_mesh.is_valid());

    let points_attr = tet_mesh.get_points_attr();

    // Points at time 0
    let points_time0: Vec<Vec3f> = vec![
        Vec3f::new(0.0, 0.0, 0.0),
        Vec3f::new(2.0, 0.0, 0.0),
        Vec3f::new(0.0, 2.0, 0.0),
        Vec3f::new(0.0, 0.0, 2.0),
        Vec3f::new(0.0, 0.0, -2.0),
    ];
    points_attr.set(Value::from_no_hash(points_time0), TimeCode::new(0.0));

    // Points at time 10
    let points_time10: Vec<Vec3f> = vec![
        Vec3f::new(0.0, 0.0, 3.0),
        Vec3f::new(2.0, 0.0, 3.0),
        Vec3f::new(0.0, 2.0, 3.0),
        Vec3f::new(0.0, 0.0, 5.0),
        Vec3f::new(0.0, 0.0, -3.0),
        Vec3f::new(2.0, 0.0, -3.0),
        Vec3f::new(0.0, 2.0, -3.0),
        Vec3f::new(0.0, 0.0, -5.0),
    ];
    points_attr.set(Value::from_no_hash(points_time10), TimeCode::new(10.0));

    // Tet vertex indices at time 0
    let tet_vertex_indices_attr = tet_mesh.get_tet_vertex_indices_attr();
    let tet_indices_time0: Vec<Vec4i> = vec![Vec4i::new(0, 1, 2, 3), Vec4i::new(0, 2, 1, 4)];
    tet_vertex_indices_attr.set(Value::from_no_hash(tet_indices_time0), TimeCode::new(0.0));

    // Tet vertex indices at time 10
    let tet_indices_time10: Vec<Vec4i> = vec![Vec4i::new(0, 1, 2, 3), Vec4i::new(4, 6, 5, 7)];
    tet_vertex_indices_attr.set(Value::from_no_hash(tet_indices_time10), TimeCode::new(10.0));

    // Check for inverted elements at frame 0 (Python uses time 10.0 for both calls)
    let inverted_time0 = tet_mesh.find_inverted_elements(TimeCode::new(10.0));
    assert!(inverted_time0.is_some());
    assert_eq!(inverted_time0.unwrap().len(), 0);

    // Check for inverted elements at frame 10
    let inverted_time10 = tet_mesh.find_inverted_elements(TimeCode::new(10.0));
    assert!(inverted_time10.is_some());
    assert_eq!(inverted_time10.unwrap().len(), 0);

    // Compute surface faces at time 0
    let surface_faces_time0 = tet_mesh.compute_surface_faces(TimeCode::new(0.0));
    assert!(surface_faces_time0.is_some());
    let surface_faces_time0 = surface_faces_time0.unwrap();
    // When the tets are joined we have 6 faces
    assert_eq!(surface_faces_time0.len(), 6);

    // Compute surface faces at time 10
    let surface_faces_time10 = tet_mesh.compute_surface_faces(TimeCode::new(10.0));
    assert!(surface_faces_time10.is_some());
    let surface_faces_time10 = surface_faces_time10.unwrap();
    // When they separate we have 8 faces
    assert_eq!(surface_faces_time10.len(), 8);
}

// ============================================================================
// test_ComputeSurfaceExtractionFromUsdGeomTetMeshLeftHanded
// ============================================================================

/// Time-varying topology and surface computation for a leftHanded tet mesh.
#[test]
fn test_compute_surface_extraction_left_handed() {
    setup();
    let s = stage();
    let tet_mesh = TetMesh::define(&s, &path("/tetMesh"));
    assert!(tet_mesh.is_valid());

    // Set orientation to leftHanded
    let orientation_attr = tet_mesh.point_based().gprim().get_orientation_attr();
    orientation_attr.set(
        usd_geom_tokens().left_handed.clone(),
        TimeCode::default_time(),
    );

    let points_attr = tet_mesh.get_points_attr();

    // Points at time 0 (leftHanded: x=-2 instead of +2)
    let points_time0: Vec<Vec3f> = vec![
        Vec3f::new(0.0, 0.0, 0.0),
        Vec3f::new(-2.0, 0.0, 0.0),
        Vec3f::new(0.0, 2.0, 0.0),
        Vec3f::new(0.0, 0.0, 2.0),
        Vec3f::new(0.0, 0.0, -2.0),
    ];
    points_attr.set(Value::from_no_hash(points_time0), TimeCode::new(0.0));

    // Points at time 10
    let points_time10: Vec<Vec3f> = vec![
        Vec3f::new(0.0, 0.0, 3.0),
        Vec3f::new(-2.0, 0.0, 3.0),
        Vec3f::new(0.0, 2.0, 3.0),
        Vec3f::new(0.0, 0.0, 5.0),
        Vec3f::new(0.0, 0.0, -3.0),
        Vec3f::new(-2.0, 0.0, -3.0),
        Vec3f::new(0.0, 2.0, -3.0),
        Vec3f::new(0.0, 0.0, -5.0),
    ];
    points_attr.set(Value::from_no_hash(points_time10), TimeCode::new(10.0));

    // Tet vertex indices at time 0
    let tet_vertex_indices_attr = tet_mesh.get_tet_vertex_indices_attr();
    let tet_indices_time0: Vec<Vec4i> = vec![Vec4i::new(0, 1, 2, 3), Vec4i::new(0, 2, 1, 4)];
    tet_vertex_indices_attr.set(Value::from_no_hash(tet_indices_time0), TimeCode::new(0.0));

    // Tet vertex indices at time 10
    let tet_indices_time10: Vec<Vec4i> = vec![Vec4i::new(0, 1, 2, 3), Vec4i::new(4, 6, 5, 7)];
    tet_vertex_indices_attr.set(Value::from_no_hash(tet_indices_time10), TimeCode::new(10.0));

    // Check for inverted elements at frame 0 (Python uses time 10.0 for both calls)
    let inverted_time0 = tet_mesh.find_inverted_elements(TimeCode::new(10.0));
    assert!(inverted_time0.is_some());
    assert_eq!(inverted_time0.unwrap().len(), 0);

    // Check for inverted elements at frame 10
    let inverted_time10 = tet_mesh.find_inverted_elements(TimeCode::new(10.0));
    assert!(inverted_time10.is_some());
    assert_eq!(inverted_time10.unwrap().len(), 0);

    // Compute surface faces at time 0
    let surface_faces_time0 = tet_mesh.compute_surface_faces(TimeCode::new(0.0));
    assert!(surface_faces_time0.is_some());
    let surface_faces_time0 = surface_faces_time0.unwrap();
    // When the tets are joined we have 6 faces
    assert_eq!(surface_faces_time0.len(), 6);

    // Compute surface faces at time 10
    let surface_faces_time10 = tet_mesh.compute_surface_faces(TimeCode::new(10.0));
    assert!(surface_faces_time10.is_some());
    let surface_faces_time10 = surface_faces_time10.unwrap();
    // When they separate we have 8 faces
    assert_eq!(surface_faces_time10.len(), 8);
}

// ============================================================================
// test_UsdGeomTetMeshFindInvertedElements
// ============================================================================

/// Inverted element detection with various orientation / point combos.
#[test]
fn test_find_inverted_elements() {
    setup();
    let s = stage();
    let tet_mesh = TetMesh::define(&s, &path("/tetMesh"));
    assert!(tet_mesh.is_valid());

    let points_attr = tet_mesh.get_points_attr();
    let tet_vertex_indices_attr = tet_mesh.get_tet_vertex_indices_attr();
    let orientation_attr = tet_mesh.point_based().gprim().get_orientation_attr();

    // --- rightHanded orientation (default) wrt. rightHanded element ---
    let points_rh: Vec<Vec3f> = vec![
        Vec3f::new(0.0, 0.0, 0.0),
        Vec3f::new(0.0, 0.0, 1.0),
        Vec3f::new(-1.0, 0.0, 0.0),
        Vec3f::new(0.0, -1.0, 0.0),
    ];
    points_attr.set(Value::from_no_hash(points_rh), TimeCode::new(0.0));

    let tet_indices: Vec<Vec4i> = vec![Vec4i::new(0, 1, 2, 3)];
    tet_vertex_indices_attr.set(Value::from_no_hash(tet_indices), TimeCode::new(0.0));

    let inverted = tet_mesh.find_inverted_elements(TimeCode::new(0.0));
    assert!(inverted.is_some());
    assert_eq!(inverted.unwrap().len(), 0);

    // --- rightHanded element with leftHanded orientation -> 1 inverted ---
    orientation_attr.set(
        usd_geom_tokens().left_handed.clone(),
        TimeCode::default_time(),
    );
    let inverted = tet_mesh.find_inverted_elements(TimeCode::new(0.0));
    assert!(inverted.is_some());
    assert_eq!(inverted.unwrap().len(), 1);

    // --- rightHanded orientation with an inverted element ---
    orientation_attr.set(
        usd_geom_tokens().right_handed.clone(),
        TimeCode::default_time(),
    );
    let points_inv: Vec<Vec3f> = vec![
        Vec3f::new(0.0, 0.0, 0.0),
        Vec3f::new(0.0, 0.0, 1.0),
        Vec3f::new(1.0, 0.0, 0.0),
        Vec3f::new(0.0, -1.0, 0.0),
    ];
    points_attr.set(Value::from_no_hash(points_inv), TimeCode::new(0.0));

    let inverted = tet_mesh.find_inverted_elements(TimeCode::new(0.0));
    assert!(inverted.is_some());
    assert_eq!(inverted.unwrap().len(), 1);

    // --- leftHanded orientation wrt. leftHanded element -> 0 inverted ---
    orientation_attr.set(
        usd_geom_tokens().left_handed.clone(),
        TimeCode::default_time(),
    );
    let points_lh: Vec<Vec3f> = vec![
        Vec3f::new(0.0, 0.0, 0.0),
        Vec3f::new(0.0, 0.0, 1.0),
        Vec3f::new(1.0, 0.0, 0.0),
        Vec3f::new(0.0, -1.0, 0.0),
    ];
    points_attr.set(Value::from_no_hash(points_lh), TimeCode::new(0.0));

    let inverted = tet_mesh.find_inverted_elements(TimeCode::new(0.0));
    assert!(inverted.is_some());
    assert_eq!(inverted.unwrap().len(), 0);

    // --- leftHanded element with rightHanded orientation -> 1 inverted ---
    orientation_attr.set(
        usd_geom_tokens().right_handed.clone(),
        TimeCode::default_time(),
    );
    let inverted = tet_mesh.find_inverted_elements(TimeCode::new(0.0));
    assert!(inverted.is_some());
    assert_eq!(inverted.unwrap().len(), 1);

    // --- leftHanded orientation with inverted element -> 1 inverted ---
    orientation_attr.set(
        usd_geom_tokens().left_handed.clone(),
        TimeCode::default_time(),
    );
    let points_lh_inv: Vec<Vec3f> = vec![
        Vec3f::new(0.0, 0.0, 0.0),
        Vec3f::new(0.0, 0.0, 1.0),
        Vec3f::new(1.0, 0.0, 0.0),
        Vec3f::new(0.0, 1.0, 0.0),
    ];
    points_attr.set(Value::from_no_hash(points_lh_inv), TimeCode::new(0.0));

    let inverted = tet_mesh.find_inverted_elements(TimeCode::new(0.0));
    assert!(inverted.is_some());
    assert_eq!(inverted.unwrap().len(), 1);
}
