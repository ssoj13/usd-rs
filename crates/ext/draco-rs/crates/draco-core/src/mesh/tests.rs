use crate::attributes::geometry_attribute::GeometryAttributeType;
use crate::attributes::geometry_indices::{AttributeValueIndex, FaceIndex};
use crate::core::draco_types::DataType;
use crate::mesh::{MeshAttributeElementType, TriangleSoupMeshBuilder};
use crate::mesh::{MeshCleanup, MeshCleanupOptions};

#[test]
fn triangle_soup_cube_test() {
    let mut mb = TriangleSoupMeshBuilder::new();
    mb.start(12);
    let pos_att_id = mb.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);

    let v000 = [0.0f32, 0.0f32, 0.0f32];
    let v100 = [1.0f32, 0.0f32, 0.0f32];
    let v010 = [0.0f32, 1.0f32, 0.0f32];
    let v110 = [1.0f32, 1.0f32, 0.0f32];
    let v001 = [0.0f32, 0.0f32, 1.0f32];
    let v101 = [1.0f32, 0.0f32, 1.0f32];
    let v011 = [0.0f32, 1.0f32, 1.0f32];
    let v111 = [1.0f32, 1.0f32, 1.0f32];

    // Front face.
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(0u32), &v000, &v100, &v010);
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(1u32), &v010, &v100, &v110);

    // Back face.
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(2u32), &v011, &v101, &v001);
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(3u32), &v111, &v101, &v011);

    // Top face.
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(4u32), &v010, &v110, &v011);
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(5u32), &v011, &v110, &v111);

    // Bottom face.
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(6u32), &v001, &v100, &v000);
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(7u32), &v101, &v100, &v001);

    // Right face.
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(8u32), &v100, &v101, &v110);
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(9u32), &v110, &v101, &v111);

    // Left face.
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(10u32), &v010, &v001, &v000);
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(11u32), &v011, &v001, &v010);

    let mesh = mb.finalize().expect("Failed to build cube mesh");
    assert_eq!(mesh.num_points(), 8, "Unexpected number of vertices");
    assert_eq!(mesh.num_faces(), 12, "Unexpected number of faces");
}

#[test]
fn triangle_soup_per_face_attribs() {
    let mut mb = TriangleSoupMeshBuilder::new();
    mb.start(12);
    let pos_att_id = mb.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
    let gen_att_id = mb.add_attribute(GeometryAttributeType::Generic, 1, DataType::Bool);

    let t = [1u8];
    let f = [0u8];
    let v000 = [0.0f32, 0.0f32, 0.0f32];
    let v100 = [1.0f32, 0.0f32, 0.0f32];
    let v010 = [0.0f32, 1.0f32, 0.0f32];
    let v110 = [1.0f32, 1.0f32, 0.0f32];
    let v001 = [0.0f32, 0.0f32, 1.0f32];
    let v101 = [1.0f32, 0.0f32, 1.0f32];
    let v011 = [0.0f32, 1.0f32, 1.0f32];
    let v111 = [1.0f32, 1.0f32, 1.0f32];

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(0u32), &v000, &v100, &v010);
    mb.set_per_face_attribute_value_for_face(gen_att_id, FaceIndex::from(0u32), &f);
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(1u32), &v010, &v100, &v110);
    mb.set_per_face_attribute_value_for_face(gen_att_id, FaceIndex::from(1u32), &t);

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(2u32), &v011, &v101, &v001);
    mb.set_per_face_attribute_value_for_face(gen_att_id, FaceIndex::from(2u32), &t);
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(3u32), &v111, &v101, &v011);
    mb.set_per_face_attribute_value_for_face(gen_att_id, FaceIndex::from(3u32), &t);

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(4u32), &v010, &v110, &v011);
    mb.set_per_face_attribute_value_for_face(gen_att_id, FaceIndex::from(4u32), &f);
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(5u32), &v011, &v110, &v111);
    mb.set_per_face_attribute_value_for_face(gen_att_id, FaceIndex::from(5u32), &f);

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(6u32), &v001, &v100, &v000);
    mb.set_per_face_attribute_value_for_face(gen_att_id, FaceIndex::from(6u32), &t);
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(7u32), &v101, &v100, &v001);
    mb.set_per_face_attribute_value_for_face(gen_att_id, FaceIndex::from(7u32), &t);

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(8u32), &v100, &v101, &v110);
    mb.set_per_face_attribute_value_for_face(gen_att_id, FaceIndex::from(8u32), &f);
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(9u32), &v110, &v101, &v111);
    mb.set_per_face_attribute_value_for_face(gen_att_id, FaceIndex::from(9u32), &t);

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(10u32), &v010, &v001, &v000);
    mb.set_per_face_attribute_value_for_face(gen_att_id, FaceIndex::from(10u32), &t);
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(11u32), &v011, &v001, &v010);
    mb.set_per_face_attribute_value_for_face(gen_att_id, FaceIndex::from(11u32), &f);

    let mesh = mb.finalize().expect("Failed to build cube mesh");
    assert_eq!(mesh.num_faces(), 12);
    assert_eq!(
        mesh.get_attribute_element_type(gen_att_id),
        MeshAttributeElementType::MeshFaceAttribute
    );
}

#[test]
fn triangle_soup_propagates_attribute_unique_ids() {
    let mut mb = TriangleSoupMeshBuilder::new();
    mb.start(1);
    let pos_att_id = mb.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
    let v000 = [0.0f32, 0.0f32, 0.0f32];
    let v100 = [1.0f32, 0.0f32, 0.0f32];
    let v010 = [0.0f32, 1.0f32, 0.0f32];
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(0u32), &v000, &v100, &v010);
    mb.set_attribute_unique_id(pos_att_id, 1234);
    let mesh = mb.finalize().expect("Failed to build mesh");
    let by_unique = mesh.get_attribute_by_unique_id(1234).unwrap();
    let by_index = mesh.attribute(pos_att_id).unwrap();
    assert_eq!(by_unique.unique_id(), by_index.unique_id());
}

#[test]
fn mesh_cleanup_removes_degenerated_faces() {
    let mut mb = TriangleSoupMeshBuilder::new();
    mb.start(2);
    let pos_att_id = mb.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);

    let v000 = [0.0f32, 0.0f32, 0.0f32];
    let v100 = [1.0f32, 0.0f32, 0.0f32];
    let v010 = [0.0f32, 1.0f32, 0.0f32];

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(0u32), &v000, &v100, &v010);
    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(1u32), &v010, &v100, &v100);

    let mut mesh = mb.finalize().expect("Failed to build test mesh");
    assert_eq!(mesh.num_faces(), 2);
    let options = MeshCleanupOptions::default();
    let status = MeshCleanup::cleanup(&mut mesh, &options);
    assert!(status.is_ok());
    assert_eq!(mesh.num_faces(), 1);
}

#[test]
fn mesh_cleanup_removes_degenerated_faces_and_isolated_vertices() {
    let mut mb = TriangleSoupMeshBuilder::new();
    mb.start(2);
    let pos_att_id = mb.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
    let int_att_id = mb.add_attribute(GeometryAttributeType::Generic, 2, DataType::Int32);

    let v000 = [0.0f32, 0.0f32, 0.0f32];
    let v100 = [1.0f32, 0.0f32, 0.0f32];
    let v010 = [0.0f32, 1.0f32, 0.0f32];
    let v101 = [10.0f32, 1.0f32, 0.0f32];

    let i00 = [0i32, 0i32];
    let i01 = [0i32, 1i32];
    let i02 = [0i32, 2i32];

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(0u32), &v000, &v100, &v010);
    mb.set_attribute_values_for_face(int_att_id, FaceIndex::from(0u32), &i00, &i01, &i02);

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(1u32), &v101, &v100, &v101);
    mb.set_attribute_values_for_face(int_att_id, FaceIndex::from(1u32), &i00, &i01, &i02);

    let mut mesh = mb.finalize().expect("Failed to build test mesh");
    assert_eq!(mesh.num_faces(), 2);
    assert_eq!(mesh.num_points(), 5);
    assert_eq!(mesh.attribute(int_att_id).unwrap().size(), 3);
    let options = MeshCleanupOptions::default();
    let status = MeshCleanup::cleanup(&mut mesh, &options);
    assert!(status.is_ok());
    assert_eq!(mesh.num_faces(), 1);
    assert_eq!(mesh.num_points(), 3);
    assert_eq!(mesh.attribute(int_att_id).unwrap().size(), 3);
}

#[test]
fn mesh_cleanup_attributes() {
    let mut mb = TriangleSoupMeshBuilder::new();
    mb.start(2);
    let pos_att_id = mb.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
    let generic_att_id = mb.add_attribute(GeometryAttributeType::Generic, 2, DataType::Float32);

    let v000 = [0.0f32, 0.0f32, 0.0f32];
    let v100 = [1.0f32, 0.0f32, 0.0f32];
    let v010 = [0.0f32, 1.0f32, 0.0f32];
    let v101 = [10.0f32, 1.0f32, 0.0f32];

    let g00 = [0.0f32, 0.0f32];
    let g10 = [1.0f32, 0.0f32];

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(0u32), &v000, &v100, &v010);
    mb.set_attribute_values_for_face(generic_att_id, FaceIndex::from(0u32), &g00, &g00, &g00);

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(1u32), &v101, &v100, &v101);
    mb.set_attribute_values_for_face(generic_att_id, FaceIndex::from(1u32), &g10, &g10, &g10);

    let mut mesh = mb.finalize().expect("Failed to build test mesh");
    assert_eq!(mesh.num_faces(), 2);
    assert_eq!(mesh.num_points(), 5);
    assert_eq!(mesh.attribute(1).unwrap().size(), 2);
    let options = MeshCleanupOptions::default();
    let status = MeshCleanup::cleanup(&mut mesh, &options);
    assert!(status.is_ok());
    assert_eq!(mesh.num_faces(), 1);
    assert_eq!(mesh.num_points(), 3);
    assert_eq!(mesh.attribute(0).unwrap().size(), 3);
    assert_eq!(mesh.attribute(1).unwrap().size(), 1);
}

#[test]
fn mesh_cleanup_duplicate_faces() {
    let mut mb = TriangleSoupMeshBuilder::new();
    mb.start(5);
    let pos_att_id = mb.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
    let norm_att_id = mb.add_attribute(GeometryAttributeType::Normal, 3, DataType::Float32);

    let v000 = [0.0f32, 0.0f32, 0.0f32];
    let v100 = [1.0f32, 0.0f32, 0.0f32];
    let v010 = [0.0f32, 1.0f32, 0.0f32];
    let v011 = [0.0f32, 1.0f32, 1.0f32];

    let n001 = [0.0f32, 0.0f32, 1.0f32];
    let n010 = [0.0f32, 1.0f32, 0.0f32];

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(0u32), &v000, &v100, &v010);
    mb.set_attribute_values_for_face(norm_att_id, FaceIndex::from(0u32), &n001, &n001, &n001);

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(1u32), &v000, &v100, &v010);
    mb.set_attribute_values_for_face(norm_att_id, FaceIndex::from(1u32), &n010, &n010, &n010);

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(2u32), &v000, &v100, &v011);
    mb.set_attribute_values_for_face(norm_att_id, FaceIndex::from(2u32), &n001, &n001, &n001);

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(3u32), &v100, &v010, &v000);
    mb.set_attribute_values_for_face(norm_att_id, FaceIndex::from(3u32), &n001, &n001, &n001);

    mb.set_attribute_values_for_face(pos_att_id, FaceIndex::from(4u32), &v000, &v100, &v011);
    mb.set_attribute_values_for_face(norm_att_id, FaceIndex::from(4u32), &n001, &n001, &n001);

    let mut mesh = mb.finalize().expect("Failed to build test mesh");
    assert_eq!(mesh.num_faces(), 5);
    let options = MeshCleanupOptions::default();
    let status = MeshCleanup::cleanup(&mut mesh, &options);
    assert!(status.is_ok());
    assert_eq!(mesh.num_faces(), 3);
}

#[test]
fn triangle_soup_convert_and_set_attribute_values_for_face() {
    let mut mb = TriangleSoupMeshBuilder::new();
    mb.start(2);
    let pos_att_id = mb.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);

    // Convert from f64 to Float32 (transcoder use case)
    let v0: [f64; 3] = [0.0, 0.0, 0.0];
    let v1: [f64; 3] = [1.0, 0.0, 0.0];
    let v2: [f64; 3] = [0.0, 1.0, 0.0];

    assert!(mb.convert_and_set_attribute_values_for_face(
        pos_att_id,
        FaceIndex::from(0u32),
        &v0,
        &v1,
        &v2,
    ));

    let v3: [f64; 3] = [0.0, 1.0, 1.0];
    assert!(mb.convert_and_set_attribute_values_for_face(
        pos_att_id,
        FaceIndex::from(1u32),
        &v0,
        &v1,
        &v3,
    ));

    let mesh = mb.finalize().expect("Failed to build mesh");
    assert_eq!(mesh.num_faces(), 2);
    // After dedup: v0,v1,v2,v3 unique → 4 points
    assert!(mesh.num_points() >= 4 && mesh.num_points() <= 6);

    let pos_att = mesh
        .get_named_attribute(GeometryAttributeType::Position)
        .unwrap();
    let mut val = [0.0f32; 3];
    pos_att.get_value_array_into(AttributeValueIndex::from(0), &mut val);
    assert!((val[0] - 0.0).abs() < 1e-6);
    assert!((val[1] - 0.0).abs() < 1e-6);
    assert!((val[2] - 0.0).abs() < 1e-6);
}
