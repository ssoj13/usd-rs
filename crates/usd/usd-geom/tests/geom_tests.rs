//! Integration tests for usd-geom crate.
//!
//! Ported from C++ reference: pxr/usd/usdGeom/testenv/

use std::path::PathBuf;
use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_geom::*;
use usd_gf::vec3::Vec3f;
use usd_sdf::TimeCode;
use usd_tf::Token;

/// Helper: path to testenv data files
fn testenv_path(subdir: &str, file: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("testenv");
    path.push(subdir);
    path.push(file);
    path.to_string_lossy().to_string()
}

/// Helper: open a stage from testenv
fn open_stage(subdir: &str, file: &str) -> Arc<Stage> {
    // Register file formats (USDA, USDC, etc.) before opening any files
    usd_sdf::init();
    Stage::open(testenv_path(subdir, file), InitialLoadSet::LoadAll)
        .expect("Failed to open test stage")
}

// ============================================================================
// Mesh tests (from testUsdGeomMesh.py)
// ============================================================================

#[test]
fn test_mesh_validate_topology_mismatched_counts() {
    // sum(vertexCounts) != len(vertexIndices)
    let face_vertex_indices = vec![0i32, 1, 2];
    let face_vertex_counts = vec![2i32, 2];
    let mut reason = String::new();

    let valid = Mesh::validate_topology(
        &face_vertex_indices,
        &face_vertex_counts,
        3,
        Some(&mut reason),
    );
    assert!(
        !valid,
        "Topology should be invalid when sum(counts) != len(indices)"
    );
    assert!(
        !reason.is_empty(),
        "Should have a reason for invalid topology"
    );
}

#[test]
fn test_mesh_validate_topology_negative_indices() {
    let face_vertex_indices = vec![0i32, -1, 1];
    let face_vertex_counts = vec![3i32];
    let mut reason = String::new();

    let valid = Mesh::validate_topology(
        &face_vertex_indices,
        &face_vertex_counts,
        3,
        Some(&mut reason),
    );
    assert!(!valid, "Topology should be invalid with negative indices");
    assert!(
        !reason.is_empty(),
        "Should have a reason for invalid topology"
    );
}

#[test]
fn test_mesh_validate_topology_out_of_range() {
    // Out of range vertex indices (index 3 with only 3 points: 0,1,2)
    let face_vertex_indices = vec![1i32, 2, 3];
    let face_vertex_counts = vec![3i32];
    let mut reason = String::new();

    let valid = Mesh::validate_topology(
        &face_vertex_indices,
        &face_vertex_counts,
        3,
        Some(&mut reason),
    );
    assert!(
        !valid,
        "Topology should be invalid with out-of-range indices"
    );
    assert!(
        !reason.is_empty(),
        "Should have a reason for invalid topology"
    );
}

#[test]
fn test_mesh_validate_topology_valid() {
    let face_vertex_indices = vec![0i32, 1, 2, 3, 4, 5];
    let face_vertex_counts = vec![3i32, 3];
    let mut reason = String::new();

    let valid = Mesh::validate_topology(
        &face_vertex_indices,
        &face_vertex_counts,
        6,
        Some(&mut reason),
    );
    assert!(valid, "Topology should be valid");
    assert!(
        reason.is_empty(),
        "Should not have a reason for valid topology"
    );
}

#[test]
fn test_mesh_validate_topology_no_reason() {
    // Validate without capturing reason
    let face_vertex_indices = vec![0i32, 1, 2];
    let face_vertex_counts = vec![2i32, 2];

    let valid = Mesh::validate_topology(&face_vertex_indices, &face_vertex_counts, 3, None);
    assert!(!valid);
}

#[test]
fn test_mesh_sharpness_infinite() {
    assert!(Mesh::is_sharpness_infinite(SHARPNESS_INFINITE));
    assert!(Mesh::is_sharpness_infinite(10.0));
    assert!(Mesh::is_sharpness_infinite(11.0));
    assert!(!Mesh::is_sharpness_infinite(9.9));
    assert!(!Mesh::is_sharpness_infinite(0.0));
}

#[test]
fn test_mesh_sharpness_infinite_value() {
    // C++ defines SHARPNESS_INFINITE as 10.0f
    assert_eq!(SHARPNESS_INFINITE, 10.0f32);
}

// ============================================================================
// Compute Extent tests
// ============================================================================

#[test]
fn test_cube_compute_extent() {
    let mut extent = [Vec3f::new(0.0, 0.0, 0.0); 2];
    assert!(Cube::compute_extent(2.0, &mut extent));
    assert_eq!(extent[0], Vec3f::new(-1.0, -1.0, -1.0));
    assert_eq!(extent[1], Vec3f::new(1.0, 1.0, 1.0));
}

#[test]
fn test_sphere_compute_extent() {
    let mut extent = [Vec3f::new(0.0, 0.0, 0.0); 2];
    assert!(Sphere::compute_extent(1.0, &mut extent));
    assert_eq!(extent[0], Vec3f::new(-1.0, -1.0, -1.0));
    assert_eq!(extent[1], Vec3f::new(1.0, 1.0, 1.0));
}

#[test]
fn test_cone_compute_extent_z_axis() {
    let axis = Token::new("Z");
    let mut extent = [Vec3f::new(0.0, 0.0, 0.0); 2];
    assert!(Cone::compute_extent(2.0, 1.0, &axis, &mut extent));
    assert_eq!(extent[0], Vec3f::new(-1.0, -1.0, -1.0));
    assert_eq!(extent[1], Vec3f::new(1.0, 1.0, 1.0));
}

#[test]
fn test_cone_compute_extent_x_axis() {
    let axis = Token::new("X");
    let mut extent = [Vec3f::new(0.0, 0.0, 0.0); 2];
    assert!(Cone::compute_extent(4.0, 0.5, &axis, &mut extent));
    assert_eq!(extent[0], Vec3f::new(-2.0, -0.5, -0.5));
    assert_eq!(extent[1], Vec3f::new(2.0, 0.5, 0.5));
}

#[test]
fn test_cylinder_compute_extent() {
    let axis = Token::new("Z");
    let mut extent = [Vec3f::new(0.0, 0.0, 0.0); 2];
    assert!(Cylinder::compute_extent(2.0, 1.0, &axis, &mut extent));
    assert_eq!(extent[0], Vec3f::new(-1.0, -1.0, -1.0));
    assert_eq!(extent[1], Vec3f::new(1.0, 1.0, 1.0));
}

#[test]
fn test_capsule_compute_extent() {
    let axis = Token::new("Z");
    let mut extent = [Vec3f::new(0.0, 0.0, 0.0); 2];
    // height=1, radius=0.5 -> half_height_with_cap = 0.5 + 0.5 = 1.0
    assert!(Capsule::compute_extent(1.0, 0.5, &axis, &mut extent));
    assert_eq!(extent[0], Vec3f::new(-0.5, -0.5, -1.0));
    assert_eq!(extent[1], Vec3f::new(0.5, 0.5, 1.0));
}

#[test]
fn test_plane_compute_extent_z_axis() {
    let axis = Token::new("Z");
    let mut extent = [Vec3f::new(0.0, 0.0, 0.0); 2];
    assert!(Plane::compute_extent(2.0, 2.0, &axis, &mut extent));
    assert_eq!(extent[0], Vec3f::new(-1.0, -1.0, 0.0));
    assert_eq!(extent[1], Vec3f::new(1.0, 1.0, 0.0));
}

#[test]
fn test_cylinder1_compute_extent() {
    let axis = Token::new("Z");
    let mut extent = [Vec3f::new(0.0, 0.0, 0.0); 2];
    // Different top/bottom radii: uses max
    assert!(Cylinder::compute_extent_cylinder1(
        2.0,
        0.5,
        1.5,
        &axis,
        &mut extent
    ));
    assert_eq!(extent[1].x, 1.5f32); // max radius
    assert_eq!(extent[1].z, 1.0f32); // half height
}

#[test]
fn test_capsule1_compute_extent() {
    let axis = Token::new("Z");
    let mut extent = [Vec3f::new(0.0, 0.0, 0.0); 2];
    // Equal radii: same as normal capsule
    assert!(Capsule::compute_extent_capsule1(
        1.0,
        0.5,
        0.5,
        &axis,
        &mut extent
    ));
    assert_eq!(extent[0], Vec3f::new(-0.5, -0.5, -1.0));
    assert_eq!(extent[1], Vec3f::new(0.5, 0.5, 1.0));

    // Different radii: uses max radius for bounding
    assert!(Capsule::compute_extent_capsule1(
        1.0,
        0.3,
        0.7,
        &axis,
        &mut extent
    ));
    assert_eq!(extent[1].x, 0.7f32); // max radius
    assert_eq!(extent[1].z, 1.2f32); // 0.5 + 0.7 = 1.2
}

#[test]
fn test_cone_compute_extent_invalid_axis() {
    let axis = Token::new("W"); // invalid
    let mut extent = [Vec3f::new(0.0, 0.0, 0.0); 2];
    assert!(!Cone::compute_extent(2.0, 1.0, &axis, &mut extent));
}

// ============================================================================
// Schema type name tests
// ============================================================================

#[test]
fn test_schema_type_names() {
    assert_eq!(Scope::schema_type_name().as_str(), "Scope");
    assert_eq!(Xform::schema_type_name().as_str(), "Xform");
    assert_eq!(Cube::schema_type_name().as_str(), "Cube");
    assert_eq!(Sphere::schema_type_name().as_str(), "Sphere");
    assert_eq!(Cone::schema_type_name().as_str(), "Cone");
    assert_eq!(Cylinder::schema_type_name().as_str(), "Cylinder");
    assert_eq!(Capsule::schema_type_name().as_str(), "Capsule");
    assert_eq!(Plane::schema_type_name().as_str(), "Plane");
    assert_eq!(Mesh::schema_type_name().as_str(), "Mesh");
    assert_eq!(Points::schema_type_name().as_str(), "Points");
    assert_eq!(BasisCurves::schema_type_name().as_str(), "BasisCurves");
    assert_eq!(NurbsCurves::schema_type_name().as_str(), "NurbsCurves");
    assert_eq!(NurbsPatch::schema_type_name().as_str(), "NurbsPatch");
    assert_eq!(HermiteCurves::schema_type_name().as_str(), "HermiteCurves");
    assert_eq!(TetMesh::schema_type_name().as_str(), "TetMesh");
    assert_eq!(Subset::schema_type_name().as_str(), "GeomSubset");
    assert_eq!(Camera::schema_type_name().as_str(), "Camera");
    assert_eq!(Capsule1::schema_type_name().as_str(), "Capsule_1");
    assert_eq!(Cylinder1::schema_type_name().as_str(), "Cylinder_1");
}

// ============================================================================
// Metrics tests (from testUsdGeomMetrics.py)
// ============================================================================

#[test]
fn test_metrics_linear_units() {
    assert_eq!(LinearUnits::CENTIMETERS, 0.01);
    assert_eq!(LinearUnits::METERS, 1.0);
    assert_eq!(LinearUnits::MILLIMETERS, 0.001);
    assert_eq!(LinearUnits::INCHES, 0.0254);
    assert_eq!(LinearUnits::FEET, 0.3048);
    assert_eq!(LinearUnits::YARDS, 0.9144);
}

#[test]
fn test_metrics_linear_units_are() {
    // 12 inches == 1 foot (within tolerance)
    let from_inches = 12.0 * LinearUnits::INCHES;
    assert!(linear_units_are(LinearUnits::FEET, from_inches, 1e-10));

    // 3 feet == 1 yard (within tolerance)
    let from_feet = 3.0 * LinearUnits::FEET;
    assert!(linear_units_are(LinearUnits::YARDS, from_feet, 1e-10));

    // Different units should not match
    assert!(!linear_units_are(
        LinearUnits::METERS,
        LinearUnits::FEET,
        1e-10
    ));
}

// ============================================================================
// Stage-based tests
// ============================================================================

#[test]
fn test_create_in_memory_stage_define_prims() {
    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");

    let path = usd_sdf::Path::from_string("/TestScope").unwrap();
    let scope = Scope::define(&stage, &path);
    assert!(scope.is_valid());
    assert_eq!(scope.prim().get_type_name(), "Scope");

    let cube_path = usd_sdf::Path::from_string("/TestCube").unwrap();
    let cube = Cube::define(&stage, &cube_path);
    assert!(cube.is_valid());
    assert_eq!(cube.prim().get_type_name(), "Cube");

    let mesh_path = usd_sdf::Path::from_string("/TestMesh").unwrap();
    let mesh = Mesh::define(&stage, &mesh_path);
    assert!(mesh.is_valid());
    assert_eq!(mesh.prim().get_type_name(), "Mesh");
}

#[test]
fn test_create_in_memory_xform() {
    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");

    let path = usd_sdf::Path::from_string("/TestXform").unwrap();
    let xform = Xform::define(&stage, &path);
    assert!(xform.is_valid());
    assert_eq!(xform.prim().get_type_name(), "Xform");
}

#[test]
fn test_imageable_ordered_purpose_tokens() {
    let tokens = Imageable::get_ordered_purpose_tokens();
    assert_eq!(tokens.len(), 4);
    // Order must be: [default, render, proxy, guide]
    assert_eq!(tokens[0].as_str(), "default");
    assert_eq!(tokens[1].as_str(), "render");
    assert_eq!(tokens[2].as_str(), "proxy");
    assert_eq!(tokens[3].as_str(), "guide");
}

#[test]
fn test_schema_attribute_names_cube() {
    let local = Cube::get_schema_attribute_names(false);
    let local_strs: Vec<&str> = local.iter().map(|t| t.as_str()).collect();
    assert!(
        local_strs.contains(&"size"),
        "Cube should have 'size' attribute"
    );
    assert!(
        local_strs.contains(&"extent"),
        "Cube should have 'extent' attribute"
    );
}

#[test]
fn test_schema_attribute_names_mesh() {
    let local = Mesh::get_schema_attribute_names(false);
    let local_strs: Vec<&str> = local.iter().map(|t| t.as_str()).collect();
    assert!(local_strs.contains(&"faceVertexIndices"));
    assert!(local_strs.contains(&"faceVertexCounts"));
    assert!(local_strs.contains(&"subdivisionScheme"));
    assert!(local_strs.contains(&"interpolateBoundary"));
    assert!(local_strs.contains(&"faceVaryingLinearInterpolation"));
    assert!(local_strs.contains(&"triangleSubdivisionRule"));
    assert!(local_strs.contains(&"holeIndices"));
    assert!(local_strs.contains(&"cornerIndices"));
    assert!(local_strs.contains(&"cornerSharpnesses"));
    assert!(local_strs.contains(&"creaseIndices"));
    assert!(local_strs.contains(&"creaseLengths"));
    assert!(local_strs.contains(&"creaseSharpnesses"));
}

#[test]
fn test_schema_attribute_names_point_instancer() {
    let local = PointInstancer::get_schema_attribute_names(false);
    let local_strs: Vec<&str> = local.iter().map(|t| t.as_str()).collect();
    assert!(local_strs.contains(&"protoIndices"));
    assert!(local_strs.contains(&"ids"));
    assert!(local_strs.contains(&"positions"));
    assert!(local_strs.contains(&"orientations"));
    assert!(local_strs.contains(&"scales"));
    assert!(local_strs.contains(&"velocities"));
    assert!(local_strs.contains(&"accelerations"));
    assert!(local_strs.contains(&"angularVelocities"));
    assert!(local_strs.contains(&"invisibleIds"));
}

#[test]
fn test_geom_tokens() {
    let tokens = usd_geom_tokens();
    assert_eq!(tokens.x.as_str(), "X");
    assert_eq!(tokens.y.as_str(), "Y");
    assert_eq!(tokens.z.as_str(), "Z");
    assert_eq!(tokens.default_.as_str(), "default");
    assert_eq!(tokens.render.as_str(), "render");
    assert_eq!(tokens.proxy.as_str(), "proxy");
    assert_eq!(tokens.guide.as_str(), "guide");
    assert_eq!(tokens.invisible.as_str(), "invisible");
    assert_eq!(tokens.inherited.as_str(), "inherited");
}

// ============================================================================
// Mesh face count tests (from testUsdGeomMesh.py::test_ComputeFaceCount)
// ============================================================================

#[test]
fn test_mesh_face_count_from_file() {
    let stage = open_stage("testUsdGeomMesh", "mesh.usda");

    // Time code for the earliest available sample (matches C++ UsdTimeCode::EarliestTime)
    let earliest = TimeCode::new(f64::MIN);

    // UnsetVertexCounts: should be 0
    let unset_path = usd_sdf::Path::from_string("/UnsetVertexCounts").unwrap();
    let unset = Mesh::get(&stage, &unset_path);
    assert!(unset.is_valid());
    assert_eq!(unset.get_face_count(earliest), 0);

    // BlockedVertexCounts: should be 0
    let blocked_path = usd_sdf::Path::from_string("/BlockedVertexCounts").unwrap();
    let blocked = Mesh::get(&stage, &blocked_path);
    assert!(blocked.is_valid());
    assert_eq!(blocked.get_face_count(earliest), 0);

    // EmptyVertexCounts: should be 0
    let empty_path = usd_sdf::Path::from_string("/EmptyVertexCounts").unwrap();
    let empty = Mesh::get(&stage, &empty_path);
    assert!(empty.is_valid());
    assert_eq!(empty.get_face_count(earliest), 0);

    // TimeSampledVertexCounts: 3 at time=1 (the only sample)
    let time_sampled_path = usd_sdf::Path::from_string("/TimeSampledVertexCounts").unwrap();
    let time_sampled = Mesh::get(&stage, &time_sampled_path);
    assert!(time_sampled.is_valid());
    // Query at time 1.0 (where the sample is)
    assert_eq!(time_sampled.get_face_count(TimeCode::new(1.0)), 3);

    // TimeSampledAndDefaultVertexCounts: 5 at time=1 (time sample)
    let ts_and_default_path =
        usd_sdf::Path::from_string("/TimeSampledAndDefaultVertexCounts").unwrap();
    let ts_and_default = Mesh::get(&stage, &ts_and_default_path);
    assert!(ts_and_default.is_valid());
    assert_eq!(ts_and_default.get_face_count(TimeCode::new(1.0)), 5);

    // Default value should be 4
    assert_eq!(ts_and_default.get_face_count(TimeCode::default_time()), 4);
}
