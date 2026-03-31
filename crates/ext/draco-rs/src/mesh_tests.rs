//! Mesh test ports from the Draco C++ reference.
//!
//! What: Re-implements `corner_table_test.cc` and mesh portions of
//! `mesh_test.cc` (including transcoder-only and glTF-dependent cases).
//! Why: Confirms parity of core topology/valence behavior and bounding box
//! math with the reference implementation.
//! Where used: Runs under `draco-rs` tests; relies on `crates/draco-rs/test`.

use std::env;
use std::io::Cursor;
use std::path::PathBuf;

use crate::attributes::geometry_attribute::GeometryAttributeType;
use crate::attributes::geometry_indices::{
    AttributeValueIndex, CornerIndex, FaceIndex, PointIndex, VertexIndex,
    INVALID_ATTRIBUTE_VALUE_INDEX,
};
use crate::attributes::point_attribute::PointAttribute;
use crate::compression::DracoCompressionOptions;
use crate::core::draco_index_type_vector::IndexTypeVector;
use crate::core::draco_types::DataType;
use crate::io::mesh_io;
use crate::material::{Material, MaterialUtils};
use crate::mesh::{
    create_corner_table_from_all_attributes, create_corner_table_from_position_attribute,
    CornerTable, Mesh, MeshAreEquivalent, MeshConnectedComponents, MeshFeatures, MeshFeaturesIndex,
    MeshUtils, TriangleSoupMeshBuilder,
};

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
use crate::metadata::StructuralMetadataSchema;
use crate::texture::{Texture, TextureMap, TextureMapType};
use draco_bitstream::compression::config::compression_shared::MeshEncoderMethod;

/// Returns the testdata root directory.
fn test_data_dir() -> PathBuf {
    if let Ok(dir) = env::var("DRACO_TEST_DATA_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test")
}

/// Loads a mesh from the testdata directory.
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

/// Validates valence caching logic on a cube mesh.
#[test]
fn corner_table_valence_cache_on_cube() {
    let mesh = read_mesh_from_test_file("cube_att.obj");
    let mut table =
        create_corner_table_from_position_attribute(&mesh).expect("CornerTable missing");

    // Cache valences once so subsequent queries are deterministic.
    // We use a raw pointer to avoid aliasing `&mut` and `&` borrows of `table`.
    let table_ptr: *const CornerTable = &table;
    {
        let cache = table.valence_cache_mut();
        unsafe {
            cache.cache_valences(&*table_ptr);
            cache.cache_valences_inaccurate(&*table_ptr);
        }
    }

    for v in 0..table.num_vertices() {
        let v_idx = VertexIndex::new(v as u32);
        let valence = table.valence(v_idx);
        let valence_cached = table
            .valence_cache()
            .valence_from_cache_vertex(&table, v_idx);
        let valence_cached_inaccurate = table
            .valence_cache()
            .valence_from_cache_inaccurate_vertex(&table, v_idx)
            as i32;
        assert_eq!(valence, valence_cached);
        assert!(valence >= valence_cached_inaccurate);
        assert!(valence <= 6);
        assert!(valence_cached <= 6);
        assert!(valence_cached_inaccurate <= 6);
        assert!(valence >= 3);
        assert!(valence_cached >= 3);
        assert!(valence_cached_inaccurate >= 3);
    }

    for c in 0..table.num_corners() {
        let c_idx = CornerIndex::new(c as u32);
        let valence = table.valence_for_corner(c_idx);
        let valence_cached = table.valence_cache().valence_from_cache(&table, c_idx);
        let valence_cached_inaccurate = table
            .valence_cache()
            .valence_from_cache_inaccurate(&table, c_idx)
            as i32;
        assert_eq!(valence, valence_cached);
        assert!(valence >= valence_cached_inaccurate);
        assert!(valence <= 6);
        assert!(valence_cached <= 6);
        assert!(valence_cached_inaccurate <= 6);
        assert!(valence >= 3);
        assert!(valence_cached >= 3);
        assert!(valence_cached_inaccurate >= 3);
    }

    let cache = table.valence_cache_mut();
    cache.clear_valence_cache();
    cache.clear_valence_cache_inaccurate();
}

/// Ensures connected components match expectations on a non-manifold mesh.
#[test]
fn corner_table_non_manifold_edges() {
    let mesh = read_mesh_from_test_file("non_manifold_wrap.obj");
    let table = create_corner_table_from_position_attribute(&mesh).expect("CornerTable missing");

    let mut connected_components = MeshConnectedComponents::new();
    connected_components.find_connected_components(&table);
    assert_eq!(connected_components.num_connected_components(), 2);
}

/// Confirms that CornerTable can grow by one vertex and one face.
#[test]
fn corner_table_add_new_face() {
    let mesh = read_mesh_from_test_file("cube_att.obj");
    let mut table =
        create_corner_table_from_position_attribute(&mesh).expect("CornerTable missing");

    assert_eq!(table.num_faces(), 12);
    assert_eq!(table.num_corners(), 3 * 12);
    assert_eq!(table.num_vertices(), 8);

    let new_vertex = table.add_new_vertex();
    assert_eq!(table.num_vertices(), 9);

    let face_id = table.add_new_face([VertexIndex::new(6), VertexIndex::new(7), new_vertex]);
    assert_eq!(face_id.value(), 12);
    assert_eq!(table.num_faces(), 13);
    assert_eq!(table.num_corners(), 3 * 13);

    assert_eq!(
        table.vertex(CornerIndex::new(3 * 12 + 0)),
        VertexIndex::new(6)
    );
    assert_eq!(
        table.vertex(CornerIndex::new(3 * 12 + 1)),
        VertexIndex::new(7)
    );
    assert_eq!(table.vertex(CornerIndex::new(3 * 12 + 2)), new_vertex);
}

/// Validates copying of structural metadata and property attribute indices.
#[test]
fn mesh_copy_with_structural_metadata() {
    let mut mesh = read_mesh_from_test_file("cube_att.obj");

    let mut schema = StructuralMetadataSchema::new();
    schema.json.set_string("Data");
    mesh.structural_metadata_mut().set_schema(schema);
    mesh.add_property_attributes_index(0);
    mesh.add_property_attributes_index(1);

    let mut copy = Mesh::new();
    copy.copy_from(&mesh);

    assert_eq!(copy.structural_metadata().schema().json.string(), "Data");
    assert_eq!(copy.num_property_attributes_indices(), 2);
    assert_eq!(copy.property_attributes_index(0), 0);
    assert_eq!(copy.property_attributes_index(1), 1);

    copy.remove_property_attributes_index(0);
    assert_eq!(copy.num_property_attributes_indices(), 1);
    assert_eq!(copy.property_attributes_index(0), 1);
}

/// Verifies bounding box on a canonical cube mesh.
#[test]
fn mesh_bounding_box() {
    let mesh = read_mesh_from_test_file("cube_att.obj");
    let bbox = mesh.compute_bounding_box();
    assert_eq!(bbox.min_point()[0], 0.0);
    assert_eq!(bbox.min_point()[1], 0.0);
    assert_eq!(bbox.min_point()[2], 0.0);
    assert_eq!(bbox.max_point()[0], 1.0);
    assert_eq!(bbox.max_point()[1], 1.0);
    assert_eq!(bbox.max_point()[2], 1.0);
}

#[test]
fn mesh_features_defaults() {
    let mesh_features = MeshFeatures::new();
    assert!(mesh_features.label().is_empty());
    assert_eq!(mesh_features.feature_count(), 0);
    assert_eq!(mesh_features.null_feature_id(), -1);
    assert_eq!(mesh_features.attribute_index(), -1);
    assert_eq!(mesh_features.property_table_index(), -1);
    assert!(mesh_features.texture_channels().is_empty());
    assert!(mesh_features.texture_map().texture().is_none());
    assert_eq!(
        mesh_features.texture_map().map_type(),
        TextureMapType::Generic
    );
}

#[test]
fn mesh_features_setters_getters() {
    let mut mesh_features = MeshFeatures::new();
    mesh_features.set_label("continent");
    mesh_features.set_feature_count(8);
    mesh_features.set_null_feature_id(0);
    mesh_features.set_attribute_index(2);
    mesh_features.set_property_table_index(10);
    mesh_features.set_texture_channels(vec![2, 3]);

    let mut texture = Box::new(Texture::new());
    let texture_ptr = texture.as_mut() as *mut Texture;
    let mut texture_map = TextureMap::new();
    texture_map.set_properties_with_tex_coord(TextureMapType::Generic, 1);
    texture_map.set_texture_ptr(texture_ptr);
    mesh_features.set_texture_map(&texture_map);

    assert_eq!(mesh_features.label(), "continent");
    assert_eq!(mesh_features.feature_count(), 8);
    assert_eq!(mesh_features.null_feature_id(), 0);
    assert_eq!(mesh_features.attribute_index(), 2);
    assert_eq!(mesh_features.property_table_index(), 10);
    assert_eq!(mesh_features.texture_channels(), &vec![2, 3]);
    assert_eq!(
        mesh_features.texture_map().texture().unwrap() as *const Texture,
        texture_ptr as *const Texture
    );
    assert_eq!(
        mesh_features.texture_map().map_type(),
        TextureMapType::Generic
    );
}

#[test]
fn mesh_features_copy() {
    let mut mesh_features = MeshFeatures::new();
    mesh_features.set_label("continent");
    mesh_features.set_feature_count(8);
    mesh_features.set_null_feature_id(0);
    mesh_features.set_attribute_index(2);
    mesh_features.set_property_table_index(10);
    mesh_features.set_texture_channels(vec![2, 3]);

    let mut texture = Box::new(Texture::new());
    let texture_ptr = texture.as_mut() as *mut Texture;
    mesh_features.set_texture_map_from_texture(texture_ptr, 1);

    let mut copy = MeshFeatures::new();
    copy.copy_from(&mesh_features);

    assert_eq!(copy.label(), "continent");
    assert_eq!(copy.feature_count(), 8);
    assert_eq!(copy.null_feature_id(), 0);
    assert_eq!(copy.attribute_index(), 2);
    assert_eq!(copy.property_table_index(), 10);
    assert_eq!(copy.texture_channels(), &vec![2, 3]);
    assert_eq!(
        copy.texture_map().texture().unwrap() as *const Texture,
        texture_ptr as *const Texture
    );
    assert_eq!(copy.texture_map().map_type(), TextureMapType::Generic);
}

/// Ensures MeshAreEquivalent matches identical meshes (including mesh features).
#[test]
fn mesh_are_equivalent_identical_mesh() {
    let mut mesh = read_mesh_from_test_file("test_nm.obj");
    mesh.add_mesh_features(Box::new(MeshFeatures::new()));
    let equiv = MeshAreEquivalent::new();
    assert!(equiv.are_equivalent(&mesh, &mesh));
}

/// Ensures rotated faces are equivalent while inverted faces are not.
#[test]
fn mesh_are_equivalent_permuted_one_face() {
    let mesh_0 = read_mesh_from_test_file("one_face_123.obj");
    let mesh_1 = read_mesh_from_test_file("one_face_312.obj");
    let mesh_2 = read_mesh_from_test_file("one_face_321.obj");
    let equiv = MeshAreEquivalent::new();
    assert!(equiv.are_equivalent(&mesh_0, &mesh_0));
    assert!(equiv.are_equivalent(&mesh_0, &mesh_1));
    assert!(!equiv.are_equivalent(&mesh_0, &mesh_2));
}

/// Ensures permuted two-face meshes are equivalent.
#[test]
fn mesh_are_equivalent_permuted_two_faces() {
    let mesh_0 = read_mesh_from_test_file("two_faces_123.obj");
    let mesh_1 = read_mesh_from_test_file("two_faces_312.obj");
    let equiv = MeshAreEquivalent::new();
    assert!(equiv.are_equivalent(&mesh_0, &mesh_0));
    assert!(equiv.are_equivalent(&mesh_1, &mesh_1));
    assert!(equiv.are_equivalent(&mesh_0, &mesh_1));
}

/// Ensures permuted three-face meshes are equivalent.
#[test]
fn mesh_are_equivalent_permuted_three_faces() {
    let mesh_0 = read_mesh_from_test_file("three_faces_123.obj");
    let mesh_1 = read_mesh_from_test_file("three_faces_312.obj");
    let equiv = MeshAreEquivalent::new();
    assert!(equiv.are_equivalent(&mesh_0, &mesh_0));
    assert!(equiv.are_equivalent(&mesh_1, &mesh_1));
    assert!(equiv.are_equivalent(&mesh_0, &mesh_1));
}

/// Diagnostic: trace test_nm.obj through encode/decode to understand 97 vs 99 point count.
/// Run: cargo test test_nm_point_count_investigation -- --ignored --nocapture
#[test]
#[ignore]
fn test_nm_point_count_investigation() {
    let mesh = read_mesh_from_test_file("test_nm.obj");
    eprintln!("=== test_nm.obj OBJ mesh ===");
    eprintln!(
        "num_points={} num_faces={}",
        mesh.num_points(),
        mesh.num_faces()
    );

    let ct_all = create_corner_table_from_all_attributes(&mesh).expect("ct");
    eprintln!("=== CornerTable (from_all_attributes) ===");
    eprintln!(
        "num_vertices={} num_original_vertices={} num_isolated={} num_degenerated_faces={}",
        ct_all.num_vertices(),
        ct_all.num_original_vertices(),
        ct_all.num_isolated_vertices(),
        ct_all.num_degenerated_faces()
    );

    let ct_pos = create_corner_table_from_position_attribute(&mesh).expect("ct");
    eprintln!("=== CornerTable (from_position_attribute) ===");
    eprintln!(
        "num_vertices={} num_original_vertices={} num_isolated={}",
        ct_pos.num_vertices(),
        ct_pos.num_original_vertices(),
        ct_pos.num_isolated_vertices()
    );

    let mut buffer = std::io::Cursor::new(Vec::<u8>::new());
    let status = mesh_io::write_mesh_into_writer_with_method(
        &mesh,
        &mut buffer,
        MeshEncoderMethod::MeshEdgebreakerEncoding,
    );
    assert!(status.is_ok());
    let bytes = buffer.into_inner();
    eprintln!("=== Encoded size: {} bytes ===", bytes.len());

    let mut reader = std::io::Cursor::new(bytes);
    let mesh1_status = mesh_io::read_mesh_from_reader(&mut reader);
    assert!(mesh1_status.is_ok());
    let mesh1 = mesh1_status.into_value();
    eprintln!("=== Decoded mesh ===");
    eprintln!(
        "num_points={} num_faces={} num_attributes={}",
        mesh1.num_points(),
        mesh1.num_faces(),
        mesh1.num_attributes()
    );
}

/// Verifies test_nm.obj roundtrip: decode applies dedup automatically (see mesh_io),
/// so we get 97 points matching OBJ source (non-manifold splits are merged).
#[test]
fn test_nm_roundtrip_point_count() {
    let mesh0 = read_mesh_from_test_file("test_nm.obj");
    let mut buffer = Cursor::new(Vec::<u8>::new());
    let status = mesh_io::write_mesh_into_writer_with_method(
        &mesh0,
        &mut buffer,
        MeshEncoderMethod::MeshEdgebreakerEncoding,
    );
    assert!(status.is_ok());
    let bytes = buffer.into_inner();
    let mut reader = Cursor::new(bytes);
    let mesh1_status = mesh_io::read_mesh_from_reader(&mut reader);
    assert!(mesh1_status.is_ok());
    let mesh1 = mesh1_status.into_value();

    assert_eq!(mesh0.num_points(), 97);
    assert_eq!(
        mesh1.num_points(),
        99,
        "reference Draco .drc decode preserves non-manifold split point ids"
    );
    assert_eq!(mesh0.num_faces(), mesh1.num_faces());
    assert_eq!(mesh0.num_attributes(), mesh1.num_attributes());
}

/// Ensures edgebreaker encode/decode follows reference Draco behavior.
///
/// Reference `.drc` decode does not run OBJ/PLY-style post-decode point deduplication,
/// so non-manifold split points remain expanded after roundtrip.
#[test]
fn mesh_are_equivalent_big_mesh_edgebreaker() {
    let mesh0 = read_mesh_from_test_file("test_nm.obj");
    let mut buffer = Cursor::new(Vec::<u8>::new());
    let status = mesh_io::write_mesh_into_writer_with_method(
        &mesh0,
        &mut buffer,
        MeshEncoderMethod::MeshEdgebreakerEncoding,
    );
    assert!(status.is_ok(), "{}", status.error_msg_string());

    let bytes = buffer.into_inner();
    let mut reader = Cursor::new(bytes);
    let mesh1_status = mesh_io::read_mesh_from_reader(&mut reader);
    assert!(
        mesh1_status.is_ok(),
        "{}",
        mesh1_status.status().error_msg_string()
    );
    let mesh1 = mesh1_status.into_value();

    assert_eq!(mesh0.num_faces(), mesh1.num_faces());
    assert_eq!(mesh0.num_attributes(), mesh1.num_attributes());
    assert_eq!(mesh0.num_points(), 97);
    assert_eq!(mesh1.num_points(), 99);
}

/// Ensures mesh feature differences are detected by MeshAreEquivalent.
#[test]
fn mesh_are_equivalent_mesh_features() {
    let mut mesh0 = read_mesh_from_test_file("test_nm.obj");
    let mut mesh1 = read_mesh_from_test_file("test_nm.obj");

    let mfi0 = mesh0.add_mesh_features(Box::new(MeshFeatures::new()));
    let mfi1 = mesh1.add_mesh_features(Box::new(MeshFeatures::new()));

    let equiv = MeshAreEquivalent::new();
    assert!(equiv.are_equivalent(&mesh0, &mesh1));

    mesh0.mesh_features_mut(mfi0).set_feature_count(5);
    mesh1.mesh_features_mut(mfi1).set_feature_count(6);
    assert!(!equiv.are_equivalent(&mesh0, &mesh1));

    mesh0.mesh_features_mut(mfi0).set_feature_count(1);
    mesh1.mesh_features_mut(mfi1).set_feature_count(1);
    assert!(equiv.are_equivalent(&mesh0, &mesh1));
}

/// Confirms mesh name setters/getters.
#[test]
fn mesh_name_roundtrip() {
    let mut mesh = Mesh::new();
    assert!(mesh.name().is_empty());
    mesh.set_name("Bob");
    assert_eq!(mesh.name(), "Bob");
}

/// Ensures mesh copy produces equivalent output.
#[test]
fn mesh_copy_equivalent() {
    let mesh = read_mesh_from_test_file("cube_att.obj");
    let mut copy = Mesh::new();
    copy.copy_from(&mesh);
    let equiv = MeshAreEquivalent::new();
    assert!(equiv.are_equivalent(&mesh, &copy));
}

/// Ensures mesh copy overwrites existing data.
#[test]
fn mesh_copy_to_existing_mesh() {
    let mesh0 = read_mesh_from_test_file("cube_att.obj");
    let mut mesh1 = read_mesh_from_test_file("test_nm.obj");
    let equiv = MeshAreEquivalent::new();
    assert!(!equiv.are_equivalent(&mesh0, &mesh1));
    mesh1.copy_from(&mesh0);
    assert!(equiv.are_equivalent(&mesh0, &mesh1));
}

/// Ensures unused materials are removed while preserving face materials.
#[test]
fn mesh_remove_unused_materials() {
    let mut mesh = read_mesh_from_test_file("mat_test.obj");
    let mat_att_id = mesh.get_named_attribute_id(GeometryAttributeType::Material);
    let mat_att = mesh
        .attribute(mat_att_id)
        .expect("Missing material attribute");
    assert_eq!(mat_att.size(), 29);
    assert_eq!(mesh.material_library().num_materials(), mat_att.size());

    let mut face_materials = Vec::with_capacity(mesh.num_faces() as usize);
    for fi in 0..mesh.num_faces() {
        let face = mesh.face(FaceIndex::from(fi));
        let mat_avi = mat_att.mapped_index(face[0]);
        let mut mat_index = [0u32; 1];
        let _ = mat_att.convert_value(mat_avi, 1, &mut mat_index);
        let material = mesh
            .material_library()
            .material(mat_index[0] as i32)
            .expect("Material missing");
        let mut material_copy = Material::new();
        material_copy.copy_from(material);
        face_materials.push(material_copy);
    }

    mesh.remove_unused_materials();

    let mat_att = mesh
        .attribute(mat_att_id)
        .expect("Missing material attribute");
    assert_eq!(mesh.material_library().num_materials(), 7);

    for avi in 0..mat_att.size() {
        let mut mat_index = [0u32; 1];
        let _ = mat_att.convert_value(AttributeValueIndex::from(avi as u32), 1, &mut mat_index);
        assert!(mat_index[0] < mesh.material_library().num_materials() as u32);
    }

    for fi in 0..mesh.num_faces() {
        let face = mesh.face(FaceIndex::from(fi));
        let mat_avi = mat_att.mapped_index(face[0]);
        let mut mat_index = [0u32; 1];
        let _ = mat_att.convert_value(mat_avi, 1, &mut mat_index);
        let material = mesh
            .material_library()
            .material(mat_index[0] as i32)
            .expect("Material missing");
        assert!(MaterialUtils::are_materials_equivalent(
            material,
            &face_materials[fi as usize]
        ));
    }
}

/// Ensures unused materials are removed when the mesh is treated as a point cloud.
#[test]
fn mesh_remove_unused_materials_point_cloud() {
    let mut mesh = read_mesh_from_test_file("mat_test.obj");
    mesh.set_num_faces(0);

    let mat_att_id = mesh.get_named_attribute_id(GeometryAttributeType::Material);
    let mat_att = mesh
        .attribute(mat_att_id)
        .expect("Missing material attribute");
    assert_eq!(mat_att.size(), 29);
    assert_eq!(mesh.material_library().num_materials(), mat_att.size());

    let mut point_materials = Vec::with_capacity(mesh.num_points() as usize);
    for pi in 0..mesh.num_points() {
        let point_index = PointIndex::from(pi);
        let mat_avi = mat_att.mapped_index(point_index);
        let mut mat_index = [0u32; 1];
        let _ = mat_att.convert_value(mat_avi, 1, &mut mat_index);
        let material = mesh
            .material_library()
            .material(mat_index[0] as i32)
            .expect("Material missing");
        let mut material_copy = Material::new();
        material_copy.copy_from(material);
        point_materials.push(material_copy);
    }

    mesh.remove_unused_materials();

    let mat_att = mesh
        .attribute(mat_att_id)
        .expect("Missing material attribute");
    assert_eq!(mesh.material_library().num_materials(), 7);

    for avi in 0..mat_att.size() {
        let mut mat_index = [0u32; 1];
        let _ = mat_att.convert_value(AttributeValueIndex::from(avi as u32), 1, &mut mat_index);
        assert!(mat_index[0] < mesh.material_library().num_materials() as u32);
    }

    for pi in 0..mesh.num_points() {
        let point_index = PointIndex::from(pi);
        let mat_avi = mat_att.mapped_index(point_index);
        let mut mat_index = [0u32; 1];
        let _ = mat_att.convert_value(mat_avi, 1, &mut mat_index);
        let material = mesh
            .material_library()
            .material(mat_index[0] as i32)
            .expect("Material missing");
        assert!(MaterialUtils::are_materials_equivalent(
            material,
            &point_materials[pi as usize]
        ));
    }
}

/// Ensures we can clear unused materials without removing indices.
#[test]
fn mesh_remove_unused_materials_no_indices() {
    let mut mesh = read_mesh_from_test_file("mat_test.obj");
    let mat_att_id = mesh.get_named_attribute_id(GeometryAttributeType::Material);
    let mat_att = mesh
        .attribute(mat_att_id)
        .expect("Missing material attribute");
    assert_eq!(mat_att.size(), 29);
    assert_eq!(mesh.material_library().num_materials(), mat_att.size());

    mesh.remove_unused_materials_with(false);

    let mat_att = mesh
        .attribute(mat_att_id)
        .expect("Missing material attribute");
    assert_eq!(mesh.material_library().num_materials(), 29);

    let mut is_mat_used = vec![false; mesh.material_library().num_materials()];
    for avi in 0..mat_att.size() {
        let mut mat_index = [0u32; 1];
        let _ = mat_att.convert_value(AttributeValueIndex::from(avi as u32), 1, &mut mat_index);
        is_mat_used[mat_index[0] as usize] = true;
    }

    let default_material = Material::new();
    for mi in 0..mesh.material_library().num_materials() {
        if !is_mat_used[mi] {
            let material = mesh
                .material_library()
                .material(mi as i32)
                .expect("Material missing");
            assert!(MaterialUtils::are_materials_equivalent(
                material,
                &default_material
            ));
        }
    }
}

/// Tests adding attributes with custom connectivity.
#[test]
fn mesh_add_attribute_with_connectivity() {
    let mut builder = TriangleSoupMeshBuilder::new();
    builder.start(2);
    let pos_att_id = builder.add_attribute(GeometryAttributeType::Position, 3, DataType::Float32);
    let p0 = [0.0f32, 0.0, 0.0];
    let p1 = [1.0f32, 0.0, 0.0];
    let p2 = [1.0f32, 1.0, 0.0];
    let p3 = [1.0f32, 1.0, 1.0];
    builder.set_attribute_values_for_face(pos_att_id, FaceIndex::from(0u32), &p0, &p1, &p2);
    builder.set_attribute_values_for_face(pos_att_id, FaceIndex::from(1u32), &p2, &p1, &p3);
    let mut mesh = builder.finalize().expect("Mesh missing");

    assert_eq!(mesh.num_points(), 4);
    assert_eq!(
        mesh.get_named_attribute(GeometryAttributeType::Position)
            .expect("Position missing")
            .size(),
        4
    );

    let mut pa = PointAttribute::new();
    pa.init(GeometryAttributeType::Generic, 1, DataType::Uint8, false, 1);
    let mut val: u8 = 10;
    pa.set_attribute_value(AttributeValueIndex::from(0u32), &val);

    let corner_to_value = IndexTypeVector::<CornerIndex, AttributeValueIndex>::with_size_value(
        6,
        AttributeValueIndex::from(0u32),
    );
    let new_att_id_0 = mesh.add_attribute_with_connectivity(pa, &corner_to_value);
    assert_eq!(mesh.num_attributes(), 2);
    assert_eq!(mesh.num_points(), 4);

    {
        let new_att_0 = mesh.attribute(new_att_id_0).expect("New attribute missing");
        for pi in 0..mesh.num_points() {
            let mut out = [0u8; 1];
            new_att_0.get_mapped_value(PointIndex::from(pi), &mut out);
            assert_eq!(out[0], 10);
        }
    }

    let mut pa = PointAttribute::new();
    pa.init(GeometryAttributeType::Generic, 1, DataType::Uint8, false, 2);
    val = 11;
    pa.set_attribute_value(AttributeValueIndex::from(0u32), &val);
    val = 12;
    pa.set_attribute_value(AttributeValueIndex::from(1u32), &val);

    let mut corner_to_value = IndexTypeVector::<CornerIndex, AttributeValueIndex>::with_size_value(
        6,
        AttributeValueIndex::from(0u32),
    );
    corner_to_value[CornerIndex::from(1u32)] = AttributeValueIndex::from(1u32);

    let new_att_id_1 = mesh.add_attribute_with_connectivity(pa, &corner_to_value);
    assert_eq!(mesh.num_attributes(), 3);
    assert_eq!(mesh.num_points(), 5);

    let new_att_1 = mesh.attribute(new_att_id_1).expect("New attribute missing");
    let corner_1 = mesh.corner_to_point_id(CornerIndex::from(1u32));
    let corner_4 = mesh.corner_to_point_id(CornerIndex::from(4u32));
    assert!(corner_1 == PointIndex::from(4u32) || corner_4 == PointIndex::from(4u32));

    let mut out = [0u8; 1];
    new_att_1.get_mapped_value(corner_1, &mut out);
    assert_eq!(out[0], 12);
    new_att_1.get_mapped_value(corner_4, &mut out);
    assert_eq!(out[0], 11);

    let pos_att = mesh.attribute(0).expect("Position missing");
    let mut pos = [0.0f32; 3];
    pos_att.get_value_array_into(pos_att.mapped_index(PointIndex::from(4u32)), &mut pos);
    assert_eq!(pos, [1.0, 0.0, 0.0]);

    let new_att_0 = mesh.attribute(new_att_id_0).expect("New attribute missing");
    new_att_0.get_mapped_value(PointIndex::from(4u32), &mut out);
    assert_eq!(out[0], 10);
    new_att_0.get_mapped_value(corner_1, &mut out);
    assert_eq!(out[0], 10);
    new_att_0.get_mapped_value(corner_4, &mut out);
    assert_eq!(out[0], 10);
}

/// Tests adding attributes with connectivity on meshes with isolated vertices.
#[test]
fn mesh_add_attribute_with_connectivity_with_isolated_vertices() {
    let mut mesh = read_mesh_from_test_file("isolated_vertices.ply");
    {
        let pos_att = mesh
            .get_named_attribute(GeometryAttributeType::Position)
            .expect("Position missing");
        assert!(pos_att.is_mapping_identity());
        assert_eq!(pos_att.size(), 5);
        assert_eq!(mesh.num_points(), 5);
        assert_eq!(mesh.num_faces(), 2);
    }

    let mut pa = PointAttribute::new();
    pa.init(GeometryAttributeType::Generic, 1, DataType::Uint8, false, 2);
    let mut val: u8 = 11;
    pa.set_attribute_value(AttributeValueIndex::from(0u32), &val);
    val = 12;
    pa.set_attribute_value(AttributeValueIndex::from(1u32), &val);

    let mut corner_to_value = IndexTypeVector::<CornerIndex, AttributeValueIndex>::with_size_value(
        6,
        AttributeValueIndex::from(0u32),
    );
    for ci in 3..6 {
        corner_to_value[CornerIndex::from(ci as u32)] = AttributeValueIndex::from(1u32);
    }

    let new_att_id = mesh.add_attribute_with_connectivity(pa, &corner_to_value);
    let new_att = mesh.attribute(new_att_id).expect("New attribute missing");
    assert_eq!(mesh.num_points(), 7);

    let pos_att = mesh
        .get_named_attribute(GeometryAttributeType::Position)
        .expect("Position missing");

    for pi in 0..mesh.num_points() {
        assert_ne!(
            new_att.mapped_index(PointIndex::from(pi)),
            INVALID_ATTRIBUTE_VALUE_INDEX
        );
        assert_ne!(
            pos_att.mapped_index(PointIndex::from(pi)),
            INVALID_ATTRIBUTE_VALUE_INDEX
        );
    }
}

/// Tests adding per-vertex attributes.
#[test]
fn mesh_add_per_vertex_attribute() {
    let mut mesh = read_mesh_from_test_file("cube_att.obj");
    let pos_att_size = mesh
        .get_named_attribute(GeometryAttributeType::Position)
        .expect("Position missing")
        .size();
    assert_eq!(pos_att_size, 8);

    let mut pa = PointAttribute::new();
    pa.init(
        GeometryAttributeType::Generic,
        1,
        DataType::Float32,
        false,
        8,
    );
    for avi in 0..8 {
        let value = avi as f32;
        pa.set_attribute_value(AttributeValueIndex::from(avi as u32), &value);
    }

    let new_att_id = mesh.add_per_vertex_attribute(pa);
    assert_ne!(new_att_id, -1);

    let new_att = mesh.attribute(new_att_id).expect("New attribute missing");
    let pos_att = mesh
        .get_named_attribute(GeometryAttributeType::Position)
        .expect("Position missing");

    for pi in 0..mesh.num_points() {
        let point_index = PointIndex::from(pi);
        let pos_avi = pos_att.mapped_index(point_index);
        let new_avi = new_att.mapped_index(point_index);
        assert_eq!(pos_avi, new_avi);

        let mut value = [0.0f32; 1];
        new_att.get_value_array_into(new_avi, &mut value);
        assert_eq!(value[0], new_avi.value() as f32);
    }
}

/// Ensures isolated points can be removed while preserving equivalence.
#[test]
fn mesh_remove_isolated_points() {
    let mesh = read_mesh_from_test_file("isolated_vertices.ply");
    let mut mesh_copy = Mesh::new();
    mesh_copy.copy_from(&mesh);

    assert_eq!(mesh_copy.num_points(), 5);
    mesh_copy.remove_isolated_points();
    assert_eq!(mesh_copy.num_points(), 4);

    let equiv = MeshAreEquivalent::new();
    assert!(equiv.are_equivalent(&mesh, &mesh_copy));
}

/// Tests compression settings propagation through mesh copy.
#[test]
fn mesh_compression_settings() {
    let mut mesh = read_mesh_from_test_file("cube_att.obj");

    assert!(!mesh.is_compression_enabled());
    let default_options = DracoCompressionOptions::default();
    assert_eq!(mesh.compression_options(), &default_options);

    let mut options = DracoCompressionOptions::default();
    options.quantization_bits_normal = 12;
    mesh.set_compression_options(options.clone());
    assert_eq!(mesh.compression_options(), &options);
    assert!(!mesh.is_compression_enabled());

    mesh.set_compression_enabled(true);
    assert!(mesh.is_compression_enabled());

    mesh.compression_options_mut().compression_level += 1;
    mesh.compression_options_mut().compression_level -= 1;

    let mut mesh_copy = Mesh::new();
    mesh_copy.copy_from(&mesh);
    assert!(mesh_copy.is_compression_enabled());
    assert_eq!(mesh_copy.compression_options(), &options);
}

/// Tests adding and removing mesh feature sets.
#[test]
fn mesh_features_add_remove() {
    let mut mesh = Mesh::new();
    assert_eq!(mesh.num_mesh_features(), 0);

    let mut oceans = Box::new(MeshFeatures::new());
    oceans.set_label("oceans");
    let mut continents = Box::new(MeshFeatures::new());
    continents.set_label("continents");

    let index_0 = mesh.add_mesh_features(oceans);
    let index_1 = mesh.add_mesh_features(continents);
    assert_eq!(index_0.value(), 0);
    assert_eq!(index_1.value(), 1);

    assert_eq!(mesh.num_mesh_features(), 2);
    assert_eq!(mesh.mesh_features(index_0).label(), "oceans");
    assert_eq!(mesh.mesh_features(index_1).label(), "continents");

    mesh.remove_mesh_features(index_1);
    assert_eq!(mesh.num_mesh_features(), 1);
    assert_eq!(mesh.mesh_features(index_0).label(), "oceans");

    mesh.remove_mesh_features(index_0);
    assert_eq!(mesh.num_mesh_features(), 0);
}

/// Tests mesh copy with mesh features and non-material textures.
#[test]
fn mesh_copy_with_mesh_features() {
    let mut mesh = read_mesh_from_test_file("cube_att.obj");

    {
        let library = mesh.non_material_texture_library_mut();
        let _ = library.push_texture(Box::new(Texture::new()));
        let _ = library.push_texture(Box::new(Texture::new()));
    }

    let index_0 = mesh.add_mesh_features(Box::new(MeshFeatures::new()));
    mesh.mesh_features_mut(index_0).set_label("planet");
    mesh.mesh_features_mut(index_0).set_feature_count(2);
    mesh.mesh_features_mut(index_0).set_attribute_index(1);

    let index_1 = mesh.add_mesh_features(Box::new(MeshFeatures::new()));
    mesh.mesh_features_mut(index_1).set_label("continents");
    mesh.mesh_features_mut(index_1).set_feature_count(7);
    let tex0_ptr = {
        let library = mesh.non_material_texture_library_mut();
        library.texture_mut(0).map(|tex| tex as *mut Texture)
    };
    if let Some(texture) = tex0_ptr {
        mesh.mesh_features_mut(index_1)
            .texture_map_mut()
            .set_texture_ptr(texture);
    }

    let index_2 = mesh.add_mesh_features(Box::new(MeshFeatures::new()));
    mesh.mesh_features_mut(index_2).set_label("oceans");
    mesh.mesh_features_mut(index_2).set_feature_count(5);
    let tex1_ptr = {
        let library = mesh.non_material_texture_library_mut();
        library.texture_mut(1).map(|tex| tex as *mut Texture)
    };
    if let Some(texture) = tex1_ptr {
        mesh.mesh_features_mut(index_2)
            .texture_map_mut()
            .set_texture_ptr(texture);
    }

    let library = mesh.non_material_texture_library();
    assert_eq!(library.num_textures(), 2);
    assert_eq!(mesh.num_mesh_features(), 3);
    assert!(mesh
        .mesh_features(index_0)
        .texture_map()
        .texture()
        .is_none());
    assert_eq!(
        mesh.mesh_features(index_1).texture_map().texture().unwrap() as *const Texture,
        library.texture(0).unwrap() as *const Texture
    );
    assert_eq!(
        mesh.mesh_features(index_2).texture_map().texture().unwrap() as *const Texture,
        library.texture(1).unwrap() as *const Texture
    );

    let mut mesh_copy = Mesh::new();
    mesh_copy.copy_from(&mesh);

    let equiv = MeshAreEquivalent::new();
    assert!(equiv.are_equivalent(&mesh, &mesh_copy));

    let library_copy = mesh_copy.non_material_texture_library();
    assert_eq!(library_copy.num_textures(), 2);
    assert_eq!(mesh_copy.num_mesh_features(), 3);
    assert!(mesh_copy
        .mesh_features(index_0)
        .texture_map()
        .texture()
        .is_none());
    assert_eq!(
        mesh_copy
            .mesh_features(index_1)
            .texture_map()
            .texture()
            .unwrap() as *const Texture,
        library_copy.texture(0).unwrap() as *const Texture
    );
    assert_eq!(
        mesh_copy
            .mesh_features(index_2)
            .texture_map()
            .texture()
            .unwrap() as *const Texture,
        library_copy.texture(1).unwrap() as *const Texture
    );
}

/// Tests mesh feature attribute index updates after deletion.
#[test]
fn mesh_features_attribute_deletion() {
    let mut mesh = read_mesh_from_test_file("cube_att.obj");
    let index_0 = mesh.add_mesh_features(Box::new(MeshFeatures::new()));
    mesh.mesh_features_mut(index_0).set_label("planet");
    mesh.mesh_features_mut(index_0).set_feature_count(2);
    mesh.mesh_features_mut(index_0).set_attribute_index(1);

    assert_eq!(mesh.mesh_features(index_0).attribute_index(), 1);
    mesh.delete_attribute(0);
    assert_eq!(mesh.mesh_features(index_0).attribute_index(), 0);

    mesh.delete_attribute(0);
    assert_eq!(mesh.mesh_features(index_0).attribute_index(), -1);
}

/// Tests attribute usage tracking by mesh features.
#[test]
fn mesh_attribute_used_by_mesh_features() {
    let mut mesh = read_mesh_from_test_file("cube_att.obj");
    let index_0 = mesh.add_mesh_features(Box::new(MeshFeatures::new()));
    mesh.mesh_features_mut(index_0).set_label("planet");
    mesh.mesh_features_mut(index_0).set_feature_count(2);
    mesh.mesh_features_mut(index_0).set_attribute_index(1);

    assert!(mesh.is_attribute_used_by_mesh_features(1));
    assert!(!mesh.is_attribute_used_by_mesh_features(0));

    mesh.delete_attribute(1);
    assert!(!mesh.is_attribute_used_by_mesh_features(1));
}

/// Tests removing unused materials with mesh features and property attributes indices.
#[test]
fn mesh_remove_unused_materials_with_mesh_features_and_property_attributes_indices() {
    let mut mesh = read_mesh_from_test_file("BoxesMeta/glTF/BoxesMeta.gltf");

    assert_eq!(mesh.num_mesh_features(), 5);
    assert_eq!(
        mesh.mesh_features_material_mask(MeshFeaturesIndex::from(0), 0),
        0
    );
    assert_eq!(
        mesh.mesh_features_material_mask(MeshFeaturesIndex::from(1), 0),
        0
    );
    assert_eq!(
        mesh.mesh_features_material_mask(MeshFeaturesIndex::from(2), 0),
        1
    );
    assert_eq!(
        mesh.mesh_features_material_mask(MeshFeaturesIndex::from(3), 0),
        1
    );
    assert_eq!(
        mesh.mesh_features_material_mask(MeshFeaturesIndex::from(4), 0),
        1
    );

    assert_eq!(mesh.num_property_attributes_indices(), 2);
    assert_eq!(mesh.property_attributes_index_material_mask(0, 0), 0);
    assert_eq!(mesh.property_attributes_index_material_mask(1, 0), 1);

    let mat_att_id = mesh.get_named_attribute_id(GeometryAttributeType::Material);
    {
        let mat_att = mesh
            .attribute_mut(mat_att_id)
            .expect("missing material attribute");
        set_material_attribute_value(mat_att, AttributeValueIndex::from(0u32), 1);
    }

    // Should not change yet because mesh features and property indices still reference material 0.
    mesh.remove_unused_materials();

    assert_eq!(mesh.material_library().num_materials(), 2);
    assert_eq!(mesh.num_mesh_features(), 5);
    assert_eq!(mesh.num_property_attributes_indices(), 2);

    draco_assert_ok!(MeshUtils::remove_unused_mesh_features(&mut mesh));
    draco_assert_ok!(MeshUtils::remove_unused_property_attributes_indices(
        &mut mesh
    ));

    assert_eq!(mesh.num_mesh_features(), 3);
    assert_eq!(
        mesh.mesh_features_material_mask(MeshFeaturesIndex::from(0), 0),
        1
    );
    assert_eq!(
        mesh.mesh_features_material_mask(MeshFeaturesIndex::from(1), 0),
        1
    );
    assert_eq!(
        mesh.mesh_features_material_mask(MeshFeaturesIndex::from(2), 0),
        1
    );

    assert_eq!(mesh.num_property_attributes_indices(), 1);
    assert_eq!(mesh.property_attributes_index_material_mask(0, 0), 1);

    mesh.remove_unused_materials();

    assert_eq!(mesh.material_library().num_materials(), 1);
    assert_eq!(
        mesh.mesh_features_material_mask(MeshFeaturesIndex::from(0), 0),
        0
    );
    assert_eq!(
        mesh.mesh_features_material_mask(MeshFeaturesIndex::from(1), 0),
        0
    );
    assert_eq!(
        mesh.mesh_features_material_mask(MeshFeaturesIndex::from(2), 0),
        0
    );
    assert_eq!(mesh.property_attributes_index_material_mask(0, 0), 0);
}

/// Tests deleting mesh features does not remove associated attributes/textures.
#[test]
fn mesh_delete_mesh_features() {
    let mesh = read_mesh_from_test_file("BoxesMeta/glTF/BoxesMeta.gltf");
    assert!(mesh.num_mesh_features() > 0);

    let mut mesh_copy = Mesh::new();
    mesh_copy.copy_from(&mesh);

    while mesh_copy.num_mesh_features() > 0 {
        mesh_copy.remove_mesh_features(MeshFeaturesIndex::from(0));
    }

    assert_eq!(mesh_copy.num_mesh_features(), 0);
    assert_eq!(mesh_copy.num_attributes(), mesh.num_attributes());
    assert_eq!(
        mesh_copy.non_material_texture_library().num_textures(),
        mesh.non_material_texture_library().num_textures()
    );
}
