//! Mesh utils tests ported from Draco C++ reference.
//!
//! What: Covers `mesh_utils_test.cc` parity cases (transforms, UV flips,
//! degenerate faces, quantization, metadata merge, and feature cleanup).
//! Why: Validates transcoder-facing mesh utilities against the reference.
//! Where used: `cargo test -p draco-rs mesh_utils_`.

use crate::attributes::geometry_attribute::GeometryAttributeType;
use crate::attributes::geometry_indices::AttributeValueIndex;
use crate::attributes::point_attribute::PointAttribute;
use crate::core::draco_types::DataType;
use crate::core::vector_d::{cross_product, Vector3f};
use crate::io::{mesh_io, scene_io};
use crate::mesh::{Matrix3d, Matrix4d, Mesh, MeshUtils};
use crate::scene::{MeshGroupIndex, MeshIndex, Scene};
use std::env;
use std::path::PathBuf;

/// Asserts that a Status-like value is OK (test-only helper).
macro_rules! draco_assert_ok {
    ($expression:expr) => {{
        let _local_status = $expression;
        assert!(
            _local_status.is_ok(),
            "{}",
            _local_status.error_msg_string()
        );
    }};
}

/// Returns the value from a StatusOr-like result or panics (test-only helper).
macro_rules! draco_assign_or_assert {
    ($expression:expr) => {{
        let _statusor = $expression;
        assert!(
            _statusor.is_ok(),
            "{}",
            _statusor.status().error_msg_string()
        );
        _statusor.into_value()
    }};
}

fn test_data_dir() -> PathBuf {
    if let Ok(dir) = env::var("DRACO_TEST_DATA_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test")
}

fn read_mesh_from_test_file(file_name: &str) -> Box<Mesh> {
    let path = test_data_dir()
        .join(file_name)
        .to_string_lossy()
        .into_owned();
    let status_or = mesh_io::read_mesh_from_file(&path, None, None);
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    status_or.into_value()
}

fn read_scene_from_test_file(file_name: &str) -> Box<Scene> {
    let path = test_data_dir()
        .join(file_name)
        .to_string_lossy()
        .into_owned();
    let status_or = scene_io::read_scene_from_file(&path);
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    status_or.into_value()
}

fn rotation_x(angle: f64) -> Matrix3d {
    let cos = angle.cos();
    let sin = angle.sin();
    Matrix3d {
        m: [[1.0, 0.0, 0.0], [0.0, cos, -sin], [0.0, sin, cos]],
    }
}

fn compare_rotated_normals(mesh_0: &Mesh, mesh_1: &Mesh, angle: f32) {
    let norm_att_0 = mesh_0
        .get_named_attribute(GeometryAttributeType::Normal)
        .expect("normal attribute missing");
    let norm_att_1 = mesh_1
        .get_named_attribute(GeometryAttributeType::Normal)
        .expect("normal attribute missing");
    assert_eq!(norm_att_0.size(), norm_att_1.size());

    for avi in 0..norm_att_0.size() {
        let mut n0 = [0.0f32; 3];
        let mut n1 = [0.0f32; 3];
        assert!(norm_att_0.get_value_array_into(AttributeValueIndex::from(avi as u32), &mut n0));
        assert!(norm_att_1.get_value_array_into(AttributeValueIndex::from(avi as u32), &mut n1));

        let mut norm_0 = Vector3f::new3(n0[0], n0[1], n0[2]);
        let mut norm_1 = Vector3f::new3(n1[0], n1[1], n1[2]);

        // Project the normals into yz plane.
        norm_0[0] = 0.0;
        norm_1[0] = 0.0;

        if norm_0.squared_norm() < 1e-6 {
            // Normal pointing towards X. Ensure rotated normal is about the same.
            assert!((norm_1.squared_norm()).abs() < 1e-6);
            continue;
        }

        norm_0.normalize();
        norm_1.normalize();
        let cross = cross_product(&norm_0, &norm_1);
        let norm_angle = f32::atan2(cross.squared_norm().sqrt(), norm_0.dot(&norm_1));
        assert!((norm_angle.abs() - angle).abs() < 1e-6);
    }
}

fn set_material_attribute_value(att: &PointAttribute, index: AttributeValueIndex, value: u32) {
    match att.data_type() {
        DataType::Uint8 => {
            let v = value as u8;
            att.set_attribute_value(index, &v);
        }
        DataType::Uint16 => {
            let v = value as u16;
            att.set_attribute_value(index, &v);
        }
        DataType::Uint32 => {
            let v = value as u32;
            att.set_attribute_value(index, &v);
        }
        DataType::Int8 => {
            let v = value as i8;
            att.set_attribute_value(index, &v);
        }
        DataType::Int16 => {
            let v = value as i16;
            att.set_attribute_value(index, &v);
        }
        DataType::Int32 => {
            let v = value as i32;
            att.set_attribute_value(index, &v);
        }
        _ => {
            let v = value as u32;
            att.set_attribute_value(index, &v);
        }
    }
}

#[test]
fn mesh_utils_transform() {
    let mesh = read_mesh_from_test_file("cube_att.obj");

    let mut transformed_mesh = Mesh::new();
    transformed_mesh.copy_from(&mesh);
    let mut transform = Matrix4d::identity();
    MeshUtils::transform_mesh(&transform, &mut transformed_mesh);

    // Rotate the mesh by 45 deg around the x-axis.
    transform.set_block_3x3(rotation_x(std::f64::consts::PI / 4.0));
    MeshUtils::transform_mesh(&transform, &mut transformed_mesh);
    compare_rotated_normals(&mesh, &transformed_mesh, std::f32::consts::PI / 4.0);

    // Rotate the cube back.
    transform.set_block_3x3(rotation_x(-std::f64::consts::PI / 4.0));
    MeshUtils::transform_mesh(&transform, &mut transformed_mesh);
    compare_rotated_normals(&mesh, &transformed_mesh, 0.0);
}

#[test]
fn mesh_utils_texture_uv_flips() {
    let mut mesh = read_mesh_from_test_file("cube_att.obj");

    // Check that FlipTextureUvValues only works on texture coordinates.
    {
        let pos_att = mesh.attribute(0).expect("missing position attribute");
        assert_eq!(pos_att.attribute_type(), GeometryAttributeType::Position);
    }
    {
        let pos_att_mut = mesh.attribute_mut(0).expect("missing position attribute");
        assert!(!MeshUtils::flip_texture_uv_values(false, true, pos_att_mut));
    }

    let tex_att_id = mesh.get_named_attribute_id(GeometryAttributeType::TexCoord);
    assert!(tex_att_id >= 0);
    let tex_att = mesh
        .attribute(tex_att_id)
        .expect("missing tex coord attribute");
    assert_eq!(tex_att.attribute_type(), GeometryAttributeType::TexCoord);

    let mut check_uv_values: Vec<[f32; 2]> = vec![[0.0f32; 2]; tex_att.size()];
    for avi in 0..tex_att.size() {
        let mut value = [0.0f32; 2];
        tex_att.get_value_array_into(AttributeValueIndex::from(avi as u32), &mut value);
        value[1] = 1.0 - value[1];
        check_uv_values[avi] = value;
    }

    {
        let tex_att_mut = mesh
            .attribute_mut(tex_att_id)
            .expect("missing tex coord attribute");
        assert!(MeshUtils::flip_texture_uv_values(false, true, tex_att_mut));
    }

    {
        let tex_att = mesh
            .attribute(tex_att_id)
            .expect("missing tex coord attribute");
        for avi in 0..tex_att.size() {
            let mut value = [0.0f32; 2];
            tex_att.get_value_array_into(AttributeValueIndex::from(avi as u32), &mut value);
            assert_eq!(value[0], check_uv_values[avi][0]);
            assert_eq!(value[1], check_uv_values[avi][1]);
        }
    }

    // Flip the U values.
    for value in &mut check_uv_values {
        value[0] = 1.0 - value[0];
    }

    {
        let tex_att_mut = mesh
            .attribute_mut(tex_att_id)
            .expect("missing tex coord attribute");
        assert!(MeshUtils::flip_texture_uv_values(true, false, tex_att_mut));
    }

    let tex_att = mesh
        .attribute(tex_att_id)
        .expect("missing tex coord attribute");
    for avi in 0..tex_att.size() {
        let mut value = [0.0f32; 2];
        tex_att.get_value_array_into(AttributeValueIndex::from(avi as u32), &mut value);
        assert_eq!(value[0], check_uv_values[avi][0]);
        assert_eq!(value[1], check_uv_values[avi][1]);
    }
}

#[test]
fn mesh_utils_count_degenerate_values_lantern() {
    let mut degenerate_positions_scene = 0;
    let mut degenerate_tex_coords_scene = 0;
    let scene = read_scene_from_test_file("Lantern/glTF/Lantern.gltf");

    for mgi in 0..scene.num_mesh_groups() {
        let group = scene.mesh_group(MeshGroupIndex::from(mgi as u32));
        for mi in 0..group.num_mesh_instances() {
            let mesh_index = group.mesh_instance(mi).mesh_index;
            let mesh = scene.mesh(mesh_index);
            for ai in 0..mesh.num_attributes() {
                let att = mesh.attribute(ai).expect("missing attribute");
                if att.attribute_type() == GeometryAttributeType::Position {
                    degenerate_positions_scene += MeshUtils::count_degenerate_faces(mesh, ai);
                } else if att.attribute_type() == GeometryAttributeType::TexCoord {
                    degenerate_tex_coords_scene += MeshUtils::count_degenerate_faces(mesh, ai);
                }
            }
        }
    }

    assert_eq!(degenerate_positions_scene, 0);
    assert_eq!(degenerate_tex_coords_scene, 2);

    let mesh = read_mesh_from_test_file("Lantern/glTF/Lantern.gltf");
    for ai in 0..mesh.num_attributes() {
        let att = mesh.attribute(ai).expect("missing attribute");
        if att.attribute_type() == GeometryAttributeType::Position {
            assert_eq!(
                MeshUtils::count_degenerate_faces(&mesh, ai),
                degenerate_positions_scene
            );
        } else if att.attribute_type() == GeometryAttributeType::TexCoord {
            assert_eq!(
                MeshUtils::count_degenerate_faces(&mesh, ai),
                degenerate_tex_coords_scene
            );
        }
    }
}

#[test]
fn mesh_utils_find_lowest_texture_quantization_lantern_mesh() {
    let mesh = read_mesh_from_test_file("Lantern/glTF/Lantern.gltf");

    let pos_quantization_bits = 11;
    let pos_att = mesh
        .get_named_attribute(GeometryAttributeType::Position)
        .expect("missing position attribute");
    let tex_att = mesh
        .get_named_attribute(GeometryAttributeType::TexCoord)
        .expect("missing tex coord attribute");

    let target_no_quantization_bits = 0;
    let no_quantization_bits =
        draco_assign_or_assert!(MeshUtils::find_lowest_texture_quantization(
            &mesh,
            pos_att,
            pos_quantization_bits,
            tex_att,
            target_no_quantization_bits
        ));
    assert_eq!(no_quantization_bits, 0);

    let out_of_range_low = -1;
    let statusor_low = MeshUtils::find_lowest_texture_quantization(
        &mesh,
        pos_att,
        pos_quantization_bits,
        tex_att,
        out_of_range_low,
    );
    assert!(!statusor_low.is_ok());

    let out_of_range_high = 30;
    let statusor_high = MeshUtils::find_lowest_texture_quantization(
        &mesh,
        pos_att,
        pos_quantization_bits,
        tex_att,
        out_of_range_high,
    );
    assert!(!statusor_high.is_ok());

    let target_bits = 6;
    let lowest_bits = draco_assign_or_assert!(MeshUtils::find_lowest_texture_quantization(
        &mesh,
        pos_att,
        pos_quantization_bits,
        tex_att,
        target_bits
    ));
    assert_eq!(lowest_bits, 14);
}

#[test]
fn mesh_utils_find_lowest_texture_quantization_lantern_scene() {
    let scene = read_scene_from_test_file("Lantern/glTF/Lantern.gltf");
    let expected_bits = [11, 8, 14];

    for mi in 0..scene.num_meshes() {
        let mesh = scene.mesh(MeshIndex::from(mi as u32));
        let pos_quantization_bits = 11;
        let pos_att = mesh
            .get_named_attribute(GeometryAttributeType::Position)
            .expect("missing position attribute");
        let tex_att = mesh
            .get_named_attribute(GeometryAttributeType::TexCoord)
            .expect("missing tex coord attribute");

        let target_bits = 8;
        let lowest_bits = draco_assign_or_assert!(MeshUtils::find_lowest_texture_quantization(
            mesh,
            pos_att,
            pos_quantization_bits,
            tex_att,
            target_bits
        ));
        assert_eq!(lowest_bits, expected_bits[mi as usize]);
    }
}

#[test]
fn mesh_utils_check_auto_generated_tangents() {
    let mesh = read_mesh_from_test_file("sphere_no_tangents.gltf");
    assert!(MeshUtils::has_auto_generated_tangents(&mesh));
}

#[test]
fn mesh_utils_check_merge_metadata() {
    let mut mesh = read_mesh_from_test_file("sphere_no_tangents.gltf");
    let mut other_mesh = read_mesh_from_test_file("cube_att.obj");

    {
        let mesh_metadata = mesh.get_metadata().expect("missing mesh metadata");
        assert_eq!(mesh_metadata.attribute_metadatas().len(), 1);
        assert_eq!(mesh_metadata.num_entries(), 0);
    }
    assert!(other_mesh.get_metadata().is_none());

    // Merge |other_mesh| metadata to |mesh|. Should do nothing.
    MeshUtils::merge_metadata(&other_mesh, &mut mesh);
    {
        let mesh_metadata = mesh.get_metadata().expect("missing mesh metadata");
        assert_eq!(mesh_metadata.attribute_metadatas().len(), 1);
        assert_eq!(mesh_metadata.num_entries(), 0);
    }

    // Merge |mesh| metadata to |other_mesh|. Should create empty metadata.
    MeshUtils::merge_metadata(&mesh, &mut other_mesh);
    {
        let other_metadata = other_mesh.get_metadata().expect("missing metadata");
        assert_eq!(other_metadata.attribute_metadatas().len(), 0);
        assert_eq!(other_metadata.num_entries(), 0);
    }
    assert!(!MeshUtils::has_auto_generated_tangents(&other_mesh));

    // Add dummy tangent attribute to the |other_mesh|.
    let mut tang_att = PointAttribute::new();
    tang_att.set_attribute_type(GeometryAttributeType::Tangent);
    let tang_att_id = other_mesh.add_attribute(tang_att);
    let tang_unique_id = other_mesh
        .attribute(tang_att_id)
        .expect("missing tangent attribute")
        .unique_id();

    // Merge |mesh| metadata to |other_mesh| again. Tangent metadata should copy.
    MeshUtils::merge_metadata(&mesh, &mut other_mesh);
    let other_metadata = other_mesh.get_metadata().expect("missing metadata");
    assert_eq!(other_metadata.attribute_metadatas().len(), 1);
    assert!(other_metadata
        .get_attribute_metadata_by_unique_id(tang_unique_id as i32)
        .is_some());
    assert!(MeshUtils::has_auto_generated_tangents(&other_mesh));

    // Add some entries and merge again.
    {
        let meta = mesh.metadata_mut().expect("missing metadata");
        meta.add_entry_int("test_int_0", 0);
        meta.add_entry_int("test_int_1", 1);
        meta.add_entry_int("test_int_shared", 2);
    }
    {
        let meta = other_mesh.metadata_mut().expect("missing metadata");
        meta.add_entry_int("test_int_shared", 3);
    }

    MeshUtils::merge_metadata(&mesh, &mut other_mesh);
    let other_metadata = other_mesh.get_metadata().expect("missing metadata");
    assert_eq!(other_metadata.attribute_metadatas().len(), 1);

    let att_meta = other_metadata
        .get_attribute_metadata_by_unique_id(tang_unique_id as i32)
        .expect("missing tangent metadata");
    assert_eq!(att_meta.num_entries(), 1);

    assert_eq!(other_metadata.num_entries(), 3);
    let mut metadata_value = 0;
    assert!(other_metadata.get_entry_int("test_int_0", &mut metadata_value));
    assert_eq!(metadata_value, 0);
    assert!(other_metadata.get_entry_int("test_int_1", &mut metadata_value));
    assert_eq!(metadata_value, 1);
    assert!(other_metadata.get_entry_int("test_int_shared", &mut metadata_value));
    assert_eq!(metadata_value, 3);
}

#[test]
fn mesh_utils_remove_unused_mesh_features() {
    let mut mesh = read_mesh_from_test_file("BoxesMeta/glTF/BoxesMeta.gltf");

    assert_eq!(mesh.num_mesh_features(), 5);
    assert_eq!(mesh.non_material_texture_library().num_textures(), 2);

    draco_assert_ok!(MeshUtils::remove_unused_mesh_features(&mut mesh));
    assert_eq!(mesh.num_mesh_features(), 5);
    assert_eq!(mesh.non_material_texture_library().num_textures(), 2);

    let mat_att_id = mesh.get_named_attribute_id(GeometryAttributeType::Material);
    {
        let mat_att = mesh
            .attribute_mut(mat_att_id)
            .expect("missing material attribute");
        set_material_attribute_value(mat_att, AttributeValueIndex::from(1u32), 0);
    }

    draco_assert_ok!(MeshUtils::remove_unused_mesh_features(&mut mesh));

    assert_eq!(mesh.num_mesh_features(), 2);
    assert_eq!(mesh.non_material_texture_library().num_textures(), 1);

    for mfi in 0..mesh.num_mesh_features() {
        let index = crate::mesh::MeshFeaturesIndex::from(mfi as u32);
        assert_eq!(mesh.num_mesh_features_material_masks(index), 1);
        assert_eq!(mesh.mesh_features_material_mask(index, 0), 0);
    }
}

#[test]
fn mesh_utils_remove_unused_property_attributes_indices() {
    let mut mesh = read_mesh_from_test_file("BoxesMeta/glTF/BoxesMeta.gltf");

    assert_eq!(mesh.num_property_attributes_indices(), 2);
    assert_eq!(mesh.property_attributes_index(0), 0);
    assert_eq!(mesh.property_attributes_index(1), 1);
    assert_eq!(mesh.num_property_attributes_index_material_masks(0), 1);
    assert_eq!(mesh.num_property_attributes_index_material_masks(1), 1);
    assert_eq!(mesh.property_attributes_index_material_mask(0, 0), 0);
    assert_eq!(mesh.property_attributes_index_material_mask(1, 0), 1);

    draco_assert_ok!(MeshUtils::remove_unused_property_attributes_indices(
        &mut mesh
    ));
    assert_eq!(mesh.num_property_attributes_indices(), 2);

    let mat_att_id = mesh.get_named_attribute_id(GeometryAttributeType::Material);
    {
        let mat_att = mesh
            .attribute_mut(mat_att_id)
            .expect("missing material attribute");
        set_material_attribute_value(mat_att, AttributeValueIndex::from(1u32), 0);
    }

    draco_assert_ok!(MeshUtils::remove_unused_property_attributes_indices(
        &mut mesh
    ));

    assert_eq!(mesh.num_property_attributes_indices(), 1);
    assert_eq!(mesh.num_property_attributes_index_material_masks(0), 1);
    assert_eq!(mesh.property_attributes_index_material_mask(0, 0), 0);
}
