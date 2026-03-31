//! glTF decoder tests.
//!
//! What: Parity tests for glTF decoding into Mesh and Scene.
//! Why: Ensures Draco glTF decoder matches C++ behavior and extensions.
//! How: Loads reference assets and validates attributes, materials, and metadata.
//! Where used: `cargo test -p draco-rs gltf_decoder_`.

use std::collections::HashSet;

use crate::animation::{ChannelTransformation, NodeAnimationDataType, SamplerInterpolation};
use crate::core::constants::DRACO_PI;
use crate::core::status::StatusCode;
use crate::core::vector_d::Vector3f;
use crate::io::file_utils;
use crate::io::gltf_decoder::{GltfDecoder, GltfSceneGraphMode};
use crate::io::gltf_test_helper::{GltfTestHelper, UseCase};
use crate::io::test_utils::{
    get_test_file_full_path, read_mesh_from_test_file, read_scene_from_test_file,
};
use crate::io::ImageFormat;
use crate::mesh::mesh_are_equivalent::MeshAreEquivalent;
use crate::mesh::mesh_utils::MeshUtils;
use crate::mesh::Mesh;
use crate::scene::{
    AnimationIndex, LightIndex, MeshGroupIndex, MeshIndex, MeshInstanceIndex, Scene,
    SceneNodeIndex, SceneUtils, SkinIndex, INVALID_LIGHT_INDEX,
};
use crate::texture::{TextureMapAxisWrappingMode, TextureMapType, TextureTransform, TextureUtils};
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::{FaceIndex, PointIndex};
use draco_core::core::decoder_buffer::DecoderBuffer;

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

fn decode_gltf_file(file_name: &str) -> Option<Box<Mesh>> {
    let path = get_test_file_full_path(file_name);
    let mut decoder = GltfDecoder::new();
    let maybe_geometry = decoder.decode_from_file(&path, None);
    if !maybe_geometry.is_ok() {
        return None;
    }
    Some(maybe_geometry.into_value())
}

fn decode_gltf_file_to_scene(file_name: &str) -> Option<Box<Scene>> {
    let path = get_test_file_full_path(file_name);
    let mut decoder = GltfDecoder::new();
    let maybe_scene = decoder.decode_from_file_to_scene(&path, None);
    if !maybe_scene.is_ok() {
        return None;
    }
    Some(maybe_scene.into_value())
}

fn compare_vector_array(a: &[Vector3f; 3], b: &[Vector3f; 3]) {
    for v in 0..3 {
        for c in 0..3 {
            let diff = (a[v][c] - b[v][c]).abs();
            assert!(diff < 1e-6, "v:{v} c:{c} diff:{diff}");
        }
    }
}

fn matrix_col_norm(matrix: &crate::mesh::mesh_utils::Matrix4d, col: usize) -> f64 {
    let mut sum = 0.0f64;
    for r in 0..3 {
        sum += matrix.m[r][col] * matrix.m[r][col];
    }
    sum.sqrt()
}

#[test]
fn gltf_decoder_test_sphere_gltf() {
    let mesh = decode_gltf_file("sphere.gltf").expect("mesh");
    assert_eq!(mesh.num_attributes(), 4);
    assert_eq!(mesh.num_points(), 231);
    assert_eq!(mesh.num_faces(), 224);
    assert_eq!(mesh.material_library().num_materials(), 1);
    let material = mesh.material_library().material(0).expect("material");
    assert_eq!(material.num_texture_maps(), 2);
}

#[test]
fn gltf_decoder_test_triangle_gltf() {
    let mesh = decode_gltf_file("one_face_123.gltf").expect("mesh");
    assert_eq!(mesh.num_attributes(), 1);
    assert_eq!(mesh.num_points(), 3);
    assert_eq!(mesh.num_faces(), 1);
    assert_eq!(mesh.material_library().num_materials(), 1);
    let material = mesh.material_library().material(0).expect("material");
    assert_eq!(material.num_texture_maps(), 0);

    let pos_attribute = mesh
        .get_named_attribute(GeometryAttributeType::Position)
        .expect("POSITION");
    let face = mesh.face(FaceIndex::from(0u32));
    let mut pos: [Vector3f; 3] = [Vector3f::new3(0.0, 0.0, 0.0); 3];
    for c in 0..3 {
        let mut value = [0.0f32; 3];
        pos_attribute.convert_value(pos_attribute.mapped_index(face[c]), 3, &mut value);
        pos[c] = Vector3f::new3(value[0], value[1], value[2]);
    }

    let mut pos_test: [Vector3f; 3] = [Vector3f::new3(0.0, 0.0, 0.0); 3];
    pos_test[0] = Vector3f::new3(1.0, 0.0999713, 0.0);
    pos_test[1] = Vector3f::new3(2.00006104, 0.01, 0.0);
    pos_test[2] = Vector3f::new3(3.0, 0.10998169, 0.0);
    compare_vector_array(&pos, &pos_test);
}

#[test]
fn gltf_decoder_test_triangle_gltf_cesium_rtc() {
    let mesh = decode_gltf_file("one_face_123_cesium_rtc.gltf").expect("mesh");
    assert_eq!(mesh.num_attributes(), 1);
    assert_eq!(mesh.num_points(), 3);
    assert_eq!(mesh.num_faces(), 1);
    assert_eq!(mesh.material_library().num_materials(), 1);
    let material = mesh.material_library().material(0).expect("material");
    assert_eq!(material.num_texture_maps(), 0);

    let pos_attribute = mesh
        .get_named_attribute(GeometryAttributeType::Position)
        .expect("POSITION");
    let face = mesh.face(FaceIndex::from(0u32));
    let mut pos: [Vector3f; 3] = [Vector3f::new3(0.0, 0.0, 0.0); 3];
    for c in 0..3 {
        let mut value = [0.0f32; 3];
        pos_attribute.convert_value(pos_attribute.mapped_index(face[c]), 3, &mut value);
        pos[c] = Vector3f::new3(value[0], value[1], value[2]);
    }

    let mut pos_test: [Vector3f; 3] = [Vector3f::new3(0.0, 0.0, 0.0); 3];
    pos_test[0] = Vector3f::new3(1.0, 0.0999713, 0.0);
    pos_test[1] = Vector3f::new3(2.00006104, 0.01, 0.0);
    pos_test[2] = Vector3f::new3(3.0, 0.10998169, 0.0);
    compare_vector_array(&pos, &pos_test);

    let scene = decode_gltf_file_to_scene("one_face_123_cesium_rtc.gltf").expect("scene");
    let expected_cesium_rtc = vec![-123.4, 234.5, 345_678.9];
    assert_eq!(scene.cesium_rtc(), &expected_cesium_rtc);
}

#[test]
fn gltf_decoder_test_mirrored_triangle_gltf() {
    let mesh = decode_gltf_file("one_face_123_mirror.gltf").expect("mesh");
    assert_eq!(mesh.num_attributes(), 1);
    assert_eq!(mesh.num_points(), 3);
    assert_eq!(mesh.num_faces(), 1);
    assert_eq!(mesh.material_library().num_materials(), 1);
    let material = mesh.material_library().material(0).expect("material");
    assert_eq!(material.num_texture_maps(), 0);

    let pos_attribute = mesh
        .get_named_attribute(GeometryAttributeType::Position)
        .expect("POSITION");
    let face = mesh.face(FaceIndex::from(0u32));
    let mut pos: [Vector3f; 3] = [Vector3f::new3(0.0, 0.0, 0.0); 3];
    for c in 0..3 {
        let mut value = [0.0f32; 3];
        pos_attribute.convert_value(pos_attribute.mapped_index(face[c]), 3, &mut value);
        pos[c] = Vector3f::new3(value[0], value[1], value[2]);
    }

    let mut pos_test: [Vector3f; 3] = [Vector3f::new3(0.0, 0.0, 0.0); 3];
    pos_test[0] = Vector3f::new3(-1.0, -0.0999713, 0.0);
    pos_test[1] = Vector3f::new3(-3.0, -0.10998169, 0.0);
    pos_test[2] = Vector3f::new3(-2.00006104, -0.01, 0.0);
    compare_vector_array(&pos, &pos_test);
}

#[test]
fn gltf_decoder_test_translate_triangle_gltf() {
    let mesh = decode_gltf_file("one_face_123_translated.gltf").expect("mesh");
    assert_eq!(mesh.num_attributes(), 1);
    assert_eq!(mesh.num_points(), 3);
    assert_eq!(mesh.num_faces(), 1);
    assert_eq!(mesh.material_library().num_materials(), 1);
    let material = mesh.material_library().material(0).expect("material");
    assert_eq!(material.num_texture_maps(), 0);

    let pos_attribute = mesh
        .get_named_attribute(GeometryAttributeType::Position)
        .expect("POSITION");
    let face = mesh.face(FaceIndex::from(0u32));
    let mut pos: [Vector3f; 3] = [Vector3f::new3(0.0, 0.0, 0.0); 3];
    for c in 0..3 {
        let mut value = [0.0f32; 3];
        pos_attribute.convert_value(pos_attribute.mapped_index(face[c]), 3, &mut value);
        pos[c] = Vector3f::new3(value[0], value[1], value[2]);
    }

    let translate = Vector3f::new3(-1.5, 5.0, 2.3);
    let mut pos_test: [Vector3f; 3] = [Vector3f::new3(0.0, 0.0, 0.0); 3];
    pos_test[0] = Vector3f::new3(1.0, 0.0999713, 0.0) + translate;
    pos_test[1] = Vector3f::new3(2.00006104, 0.01, 0.0) + translate;
    pos_test[2] = Vector3f::new3(3.0, 0.10998169, 0.0) + translate;
    compare_vector_array(&pos, &pos_test);
}

#[test]
fn gltf_decoder_test_milk_truck_gltf() {
    let mesh = decode_gltf_file("CesiumMilkTruck/glTF/CesiumMilkTruck.gltf").expect("mesh");
    assert_eq!(mesh.num_attributes(), 4);
    assert_eq!(mesh.num_points(), 3564);
    assert_eq!(mesh.num_faces(), 3624);
    assert_eq!(mesh.material_library().num_materials(), 4);
    assert_eq!(
        mesh.material_library()
            .material(0)
            .unwrap()
            .num_texture_maps(),
        1
    );
    assert_eq!(
        mesh.material_library()
            .material(1)
            .unwrap()
            .num_texture_maps(),
        0
    );
    assert_eq!(
        mesh.material_library()
            .material(2)
            .unwrap()
            .num_texture_maps(),
        0
    );
    assert_eq!(
        mesh.material_library()
            .material(3)
            .unwrap()
            .num_texture_maps(),
        1
    );
    assert_eq!(mesh.material_library().material(0).unwrap().name(), "truck");
    assert_eq!(mesh.material_library().material(1).unwrap().name(), "glass");
    assert_eq!(
        mesh.material_library().material(2).unwrap().name(),
        "window_trim"
    );
    assert_eq!(
        mesh.material_library().material(3).unwrap().name(),
        "wheels"
    );
}

#[test]
fn gltf_decoder_test_scene_milk_truck_gltf() {
    let scene =
        decode_gltf_file_to_scene("CesiumMilkTruck/glTF/CesiumMilkTruck.gltf").expect("scene");

    assert_eq!(scene.num_meshes(), 4);
    assert_eq!(scene.num_mesh_groups(), 2);
    assert_eq!(scene.num_nodes(), 5);
    assert_eq!(scene.num_root_nodes(), 1);
    assert_eq!(scene.num_lights(), 0);
    assert_eq!(scene.material_library().num_materials(), 4);
    assert_eq!(
        scene
            .material_library()
            .material(0)
            .unwrap()
            .num_texture_maps(),
        1
    );
    assert_eq!(
        scene
            .material_library()
            .material(1)
            .unwrap()
            .num_texture_maps(),
        0
    );
    assert_eq!(
        scene
            .material_library()
            .material(2)
            .unwrap()
            .num_texture_maps(),
        0
    );
    assert_eq!(
        scene
            .material_library()
            .material(3)
            .unwrap()
            .num_texture_maps(),
        1
    );
    assert_eq!(
        scene.material_library().material(0).unwrap().name(),
        "truck"
    );
    assert_eq!(
        scene.material_library().material(1).unwrap().name(),
        "glass"
    );
    assert_eq!(
        scene.material_library().material(2).unwrap().name(),
        "window_trim"
    );
    assert_eq!(
        scene.material_library().material(3).unwrap().name(),
        "wheels"
    );
    assert_eq!(scene.num_animations(), 1);
    assert_eq!(scene.num_skins(), 0);
    for i in 0..scene.num_animations() {
        let animation = scene.animation(AnimationIndex::from(i as u32));
        assert_eq!(animation.num_samplers(), 2);
        assert_eq!(animation.num_channels(), 2);
    }

    assert_eq!(
        scene.mesh_group(MeshGroupIndex::from(0)).name(),
        "Cesium_Milk_Truck"
    );
    assert_eq!(scene.mesh_group(MeshGroupIndex::from(1)).name(), "Wheels");

    for i in 0..scene.num_meshes() {
        let mesh = scene.mesh(MeshIndex::from(i as u32));
        assert_eq!(mesh.material_library().num_materials(), 0);
    }
}

#[test]
fn gltf_decoder_test_animated_bones_gltf() {
    let scene = decode_gltf_file_to_scene("CesiumMan/glTF/CesiumMan.gltf").expect("scene");
    assert_eq!(scene.num_meshes(), 1);
    assert_eq!(scene.num_mesh_groups(), 1);
    let mesh_group = scene.mesh_group(MeshGroupIndex::from(0));
    assert_eq!(mesh_group.num_mesh_instances(), 1);
    assert_eq!(mesh_group.mesh_instance(0).material_index, 0);
    assert_eq!(scene.num_nodes(), 22);
    assert_eq!(scene.num_root_nodes(), 1);
    assert_eq!(scene.material_library().num_materials(), 1);
    assert_eq!(
        scene
            .material_library()
            .material(0)
            .unwrap()
            .num_texture_maps(),
        1
    );
    assert_eq!(scene.num_animations(), 1);
    assert_eq!(scene.num_skins(), 1);
    for i in 0..scene.num_animations() {
        let animation = scene.animation(AnimationIndex::from(i as u32));
        assert_eq!(animation.num_samplers(), 57);
        assert_eq!(animation.num_channels(), 57);
    }

    for i in 0..scene.num_meshes() {
        let mesh = scene.mesh(MeshIndex::from(i as u32));
        assert_eq!(mesh.material_library().num_materials(), 0);
    }
}

#[test]
fn gltf_decoder_test_animated_bones_glb() {
    let scene = decode_gltf_file_to_scene("CesiumMan/glTF_Binary/CesiumMan.glb").expect("scene");
    assert_eq!(scene.num_meshes(), 1);
    assert_eq!(scene.num_mesh_groups(), 1);
    let mesh_group = scene.mesh_group(MeshGroupIndex::from(0));
    assert_eq!(mesh_group.num_mesh_instances(), 1);
    assert_eq!(mesh_group.mesh_instance(0).material_index, 0);
    assert_eq!(scene.num_nodes(), 22);
    assert_eq!(scene.num_root_nodes(), 1);
    assert_eq!(scene.material_library().num_materials(), 1);
    assert_eq!(
        scene
            .material_library()
            .material(0)
            .unwrap()
            .num_texture_maps(),
        1
    );
    assert_eq!(scene.num_animations(), 1);
    assert_eq!(scene.num_skins(), 1);
    for i in 0..scene.num_animations() {
        let animation = scene.animation(AnimationIndex::from(i as u32));
        assert_eq!(animation.num_samplers(), 57);
        assert_eq!(animation.num_channels(), 57);
    }

    for i in 0..scene.num_meshes() {
        let mesh = scene.mesh(MeshIndex::from(i as u32));
        assert_eq!(mesh.material_library().num_materials(), 0);
    }
}

#[test]
fn gltf_decoder_test_lantern_gltf() {
    let mesh = decode_gltf_file("Lantern/glTF/Lantern.gltf").expect("mesh");
    assert_eq!(mesh.num_attributes(), 4);
    assert_eq!(mesh.num_points(), 4145);
    assert_eq!(mesh.num_faces(), 5394);
    assert_eq!(mesh.material_library().num_materials(), 1);
    assert_eq!(
        mesh.material_library()
            .material(0)
            .unwrap()
            .num_texture_maps(),
        4
    );
}

#[test]
fn gltf_decoder_test_color_attribute_gltf() {
    let mesh = decode_gltf_file("test_pos_color.gltf").expect("mesh");
    assert_eq!(mesh.num_attributes(), 2);
    assert_eq!(mesh.num_points(), 114);
    assert_eq!(mesh.num_faces(), 224);
    assert_eq!(mesh.material_library().num_materials(), 1);
    assert_eq!(
        mesh.material_library()
            .material(0)
            .unwrap()
            .num_texture_maps(),
        0
    );
    let color_att = mesh
        .get_named_attribute(GeometryAttributeType::Color)
        .expect("COLOR");
    assert_eq!(
        color_att.data_type(),
        draco_core::core::draco_types::DataType::Uint8
    );
    assert!(color_att.normalized());
}

#[test]
fn gltf_decoder_test_color_attribute_gltf_scene() {
    let scene = decode_gltf_file_to_scene("test_pos_color.gltf").expect("scene");
    assert_eq!(scene.num_meshes(), 1);
    let mesh = scene.mesh(MeshIndex::from(0));
    let color_att = mesh
        .get_named_attribute(GeometryAttributeType::Color)
        .expect("COLOR");
    assert_eq!(
        color_att.data_type(),
        draco_core::core::draco_types::DataType::Uint8
    );
    assert!(color_att.normalized());
}

#[test]
fn gltf_decoder_test_two_tex_coord_attributes_gltf() {
    let mesh = decode_gltf_file("sphere_two_tex_coords.gltf").expect("mesh");
    assert_eq!(
        mesh.num_named_attributes(GeometryAttributeType::TexCoord),
        2
    );
}

#[test]
fn gltf_decoder_test_scene_with_tangents() {
    let scene = decode_gltf_file_to_scene("Lantern/glTF/Lantern.gltf").expect("scene");

    let mut num_tangent_attributes = 0;
    for i in 0..scene.num_meshes() {
        let mesh = scene.mesh(MeshIndex::from(i as u32));
        if mesh
            .get_named_attribute(GeometryAttributeType::Tangent)
            .is_some()
        {
            num_tangent_attributes += 1;
            assert!(!MeshUtils::has_auto_generated_tangents(mesh));
        }
    }
    assert!(num_tangent_attributes > 0);
}

#[test]
fn gltf_decoder_test_shared_images() {
    let mesh = decode_gltf_file("SphereAllSame/sphere_texture_all.gltf").expect("mesh");
    assert_eq!(mesh.material_library().num_materials(), 1);
    assert_eq!(
        mesh.material_library()
            .material(0)
            .unwrap()
            .num_texture_maps(),
        5
    );
    assert_eq!(mesh.material_library().texture_library().num_textures(), 4);
}

#[test]
fn gltf_decoder_test_texture_names_not_empty() {
    let mesh = decode_gltf_file("SphereAllSame/sphere_texture_all.gltf").expect("mesh");
    assert_eq!(mesh.material_library().num_materials(), 1);
    assert_eq!(
        mesh.material_library()
            .material(0)
            .unwrap()
            .num_texture_maps(),
        5
    );
    assert_eq!(mesh.material_library().texture_library().num_textures(), 4);
    let textures = vec![
        mesh.material_library()
            .texture_library()
            .texture(0)
            .unwrap(),
        mesh.material_library()
            .texture_library()
            .texture(1)
            .unwrap(),
        mesh.material_library()
            .texture_library()
            .texture(2)
            .unwrap(),
        mesh.material_library()
            .texture_library()
            .texture(3)
            .unwrap(),
    ];
    assert_eq!(
        TextureUtils::get_target_stem(textures[0]),
        "256x256_all_orange"
    );
    assert_eq!(
        TextureUtils::get_target_stem(textures[1]),
        "256x256_all_blue"
    );
    assert_eq!(
        TextureUtils::get_target_stem(textures[2]),
        "256x256_all_red"
    );
    assert_eq!(
        TextureUtils::get_target_stem(textures[3]),
        "256x256_all_green"
    );
    assert_eq!(
        TextureUtils::get_target_format(textures[0]),
        ImageFormat::Png
    );
    assert_eq!(
        TextureUtils::get_target_format(textures[1]),
        ImageFormat::Png
    );
    assert_eq!(
        TextureUtils::get_target_format(textures[2]),
        ImageFormat::Png
    );
    assert_eq!(
        TextureUtils::get_target_format(textures[3]),
        ImageFormat::Png
    );
}

#[test]
fn gltf_decoder_test_tex_coord1() {
    let mesh = decode_gltf_file("MultiUVTest/glTF/MultiUVTest.gltf").expect("mesh");
    assert_eq!(mesh.material_library().num_materials(), 1);
    assert_eq!(
        mesh.material_library()
            .material(0)
            .unwrap()
            .num_texture_maps(),
        2
    );
    assert_eq!(mesh.material_library().texture_library().num_textures(), 2);
    let textures = vec![
        mesh.material_library()
            .texture_library()
            .texture(0)
            .unwrap(),
        mesh.material_library()
            .texture_library()
            .texture(1)
            .unwrap(),
    ];
    assert_eq!(TextureUtils::get_target_stem(textures[0]), "uv0");
    assert_eq!(TextureUtils::get_target_stem(textures[1]), "uv1");
    assert_eq!(
        TextureUtils::get_target_format(textures[0]),
        ImageFormat::Png
    );
    assert_eq!(
        TextureUtils::get_target_format(textures[1]),
        ImageFormat::Png
    );
    assert_eq!(
        mesh.num_named_attributes(GeometryAttributeType::TexCoord),
        2
    );
    assert_eq!(
        mesh.num_named_attributes(GeometryAttributeType::Position),
        1
    );
    assert_eq!(mesh.num_named_attributes(GeometryAttributeType::Normal), 1);
    assert_eq!(mesh.num_named_attributes(GeometryAttributeType::Tangent), 1);
}

#[test]
fn gltf_decoder_test_simple_scene() {
    let scene = decode_gltf_file_to_scene("Box/glTF/Box.gltf").expect("scene");

    assert_eq!(scene.num_meshes(), 1);
    assert_eq!(scene.num_mesh_groups(), 1);
    let mesh_group = scene.mesh_group(MeshGroupIndex::from(0));
    assert_eq!(mesh_group.num_mesh_instances(), 1);
    assert_eq!(mesh_group.mesh_instance(0).material_index, 0);
    assert_eq!(scene.num_nodes(), 2);
    assert_eq!(scene.num_root_nodes(), 1);
    assert_eq!(scene.material_library().num_materials(), 1);
    assert_eq!(
        scene
            .material_library()
            .material(0)
            .unwrap()
            .num_texture_maps(),
        0
    );
    assert_eq!(scene.num_skins(), 0);
    assert_eq!(scene.num_animations(), 0);

    for i in 0..scene.num_meshes() {
        let mesh = scene.mesh(MeshIndex::from(i as u32));
        assert_eq!(mesh.material_library().num_materials(), 0);
    }

    assert!(scene.node(SceneNodeIndex::from(0)).name().is_empty());
    assert!(scene.node(SceneNodeIndex::from(1)).name().is_empty());
}

#[test]
fn gltf_decoder_test_lantern_scene() {
    let scene = decode_gltf_file_to_scene("Lantern/glTF/Lantern.gltf").expect("scene");

    assert_eq!(scene.num_meshes(), 3);
    assert_eq!(scene.num_mesh_groups(), 3);
    assert_eq!(scene.num_nodes(), 4);
    assert_eq!(scene.num_root_nodes(), 1);
    assert_eq!(scene.material_library().num_materials(), 1);
    assert_eq!(
        scene
            .material_library()
            .material(0)
            .unwrap()
            .num_texture_maps(),
        4
    );
    assert!(!scene.material_library().material(0).unwrap().double_sided());
    assert_eq!(scene.num_skins(), 0);
    assert_eq!(scene.num_animations(), 0);

    assert_eq!(scene.node(SceneNodeIndex::from(0)).name(), "Lantern");
    assert_eq!(
        scene.node(SceneNodeIndex::from(1)).name(),
        "LanternPole_Body"
    );
    assert_eq!(
        scene.node(SceneNodeIndex::from(2)).name(),
        "LanternPole_Chain"
    );
    assert_eq!(
        scene.node(SceneNodeIndex::from(3)).name(),
        "LanternPole_Lantern"
    );
}

#[test]
fn gltf_decoder_test_simple_triangle_mesh() {
    let mesh = decode_gltf_file("Triangle/glTF/Triangle.gltf").expect("mesh");
    assert_eq!(mesh.num_attributes(), 1);
    assert_eq!(mesh.num_points(), 3);
    assert_eq!(mesh.num_faces(), 1);
    assert_eq!(mesh.material_library().num_materials(), 0);
}

#[test]
fn gltf_decoder_test_simple_triangle_scene() {
    let scene = decode_gltf_file_to_scene("Triangle/glTF/Triangle.gltf").expect("scene");

    assert_eq!(scene.num_meshes(), 1);
    assert_eq!(scene.num_mesh_groups(), 1);
    let mesh_group = scene.mesh_group(MeshGroupIndex::from(0));
    assert_eq!(mesh_group.num_mesh_instances(), 1);
    assert_eq!(mesh_group.mesh_instance(0).material_index, 0);
    assert_eq!(scene.num_nodes(), 1);
    assert_eq!(scene.num_root_nodes(), 1);
    assert_eq!(scene.material_library().num_materials(), 1);
    assert_eq!(scene.num_skins(), 0);
    assert_eq!(scene.num_animations(), 0);
}

#[test]
fn gltf_decoder_test_three_meshes_one_no_material_scene() {
    let scene = decode_gltf_file_to_scene(
        "three_meshes_two_materials_one_no_material/three_meshes_two_materials_one_no_material.gltf",
    )
    .expect("scene");

    assert_eq!(scene.num_meshes(), 3);
    assert_eq!(scene.num_mesh_groups(), 3);
    assert_eq!(scene.num_nodes(), 4);
    assert_eq!(scene.num_root_nodes(), 1);
    assert_eq!(scene.material_library().num_materials(), 3);
    assert_eq!(scene.num_skins(), 0);
    assert_eq!(scene.num_animations(), 0);
}

#[test]
fn gltf_decoder_test_three_meshes_one_no_material_mesh() {
    let mesh = decode_gltf_file(
        "three_meshes_two_materials_one_no_material/three_meshes_two_materials_one_no_material.gltf",
    )
    .expect("mesh");

    assert_eq!(mesh.num_attributes(), 4);
    assert_eq!(mesh.num_points(), 72);
    assert_eq!(mesh.num_faces(), 36);
    assert_eq!(mesh.material_library().num_materials(), 3);
}

#[test]
fn gltf_decoder_test_double_sided_material() {
    let mesh = decode_gltf_file("TwoSidedPlane/glTF/TwoSidedPlane.gltf").expect("mesh");
    assert_eq!(mesh.material_library().num_materials(), 1);
    assert!(mesh.material_library().material(0).unwrap().double_sided());

    let scene = decode_gltf_file_to_scene("TwoSidedPlane/glTF/TwoSidedPlane.gltf").expect("scene");
    assert_eq!(scene.material_library().num_materials(), 1);
    assert!(scene.material_library().material(0).unwrap().double_sided());
}

#[test]
fn gltf_decoder_test_vertex_color_test() {
    let mesh = decode_gltf_file("VertexColorTest/glTF/VertexColorTest.gltf").expect("mesh");
    assert_eq!(mesh.material_library().num_materials(), 2);
    assert_eq!(mesh.num_named_attributes(GeometryAttributeType::Color), 1);

    let scene =
        decode_gltf_file_to_scene("VertexColorTest/glTF/VertexColorTest.gltf").expect("scene");
    assert_eq!(scene.material_library().num_materials(), 2);
    assert_eq!(scene.num_meshes(), 2);
    let second_mesh = scene.mesh(MeshIndex::from(1));
    assert_eq!(
        second_mesh.num_named_attributes(GeometryAttributeType::Color),
        1
    );
}

#[test]
fn gltf_decoder_test_morph_targets() {
    let path = get_test_file_full_path(
        "KhronosSampleModels/AnimatedMorphCube/glTF/AnimatedMorphCube.gltf",
    );
    let mut decoder = GltfDecoder::new();
    let maybe_scene = decoder.decode_from_file_to_scene(&path, None);
    assert!(!maybe_scene.is_ok());
    assert_eq!(maybe_scene.status().code(), StatusCode::UnsupportedFeature);
}

#[test]
fn gltf_decoder_test_sparse_accessors() {
    let path = get_test_file_full_path(
        "KhronosSampleModels/SimpleSparseAccessor/glTF/SimpleSparseAccessor.gltf",
    );
    let mut decoder = GltfDecoder::new();
    let maybe_scene = decoder.decode_from_file_to_scene(&path, None);
    assert!(!maybe_scene.is_ok());
    assert_eq!(maybe_scene.status().code(), StatusCode::UnsupportedFeature);
}

#[test]
fn gltf_decoder_test_pbr_specular_glossiness_extension() {
    let path = get_test_file_full_path(
        "KhronosSampleModels/SpecGlossVsMetalRough/glTF/SpecGlossVsMetalRough.gltf",
    );
    let mut decoder = GltfDecoder::new();
    let maybe_scene = decoder.decode_from_file_to_scene(&path, None);
    assert!(!maybe_scene.is_ok());
    assert_eq!(maybe_scene.status().code(), StatusCode::UnsupportedFeature);
}

#[test]
fn gltf_decoder_test_different_wrapping_modes() {
    let path = get_test_file_full_path(
        "KhronosSampleModels/TextureSettingsTest/glTF/TextureSettingsTest.gltf",
    );
    let mut decoder = GltfDecoder::new();
    let maybe_scene = decoder.decode_from_file_to_scene(&path, None);
    assert!(maybe_scene.is_ok());
    let scene = maybe_scene.into_value();
    assert_eq!(scene.material_library().texture_library().num_textures(), 3);
    assert_eq!(scene.material_library().num_materials(), 10);
    let material = scene.material_library().material(0).expect("material");
    assert_eq!(material.num_texture_maps(), 1);
    let wrapping = material
        .texture_map_by_index(0)
        .expect("texture map")
        .wrapping_mode();
    assert_eq!(wrapping.s, TextureMapAxisWrappingMode::Repeat);
    assert_eq!(wrapping.t, TextureMapAxisWrappingMode::MirroredRepeat);
}

#[test]
fn gltf_decoder_test_khr_materials_unlit_extension() {
    let scene_no_unlit = decode_gltf_file_to_scene("Box/glTF/Box.gltf").expect("scene");
    assert_eq!(scene_no_unlit.material_library().num_materials(), 1);
    assert!(!scene_no_unlit
        .material_library()
        .material(0)
        .unwrap()
        .unlit());

    let mesh = decode_gltf_file("KhronosSampleModels/UnlitTest/glTF/UnlitTest.gltf").expect("mesh");
    assert_eq!(mesh.material_library().num_materials(), 2);
    assert!(mesh.material_library().material(0).unwrap().unlit());
    assert!(mesh.material_library().material(1).unwrap().unlit());

    let scene = decode_gltf_file_to_scene("KhronosSampleModels/UnlitTest/glTF/UnlitTest.gltf")
        .expect("scene");
    assert_eq!(scene.material_library().num_materials(), 2);
    assert!(scene.material_library().material(0).unwrap().unlit());
    assert!(scene.material_library().material(1).unwrap().unlit());
}

#[test]
fn gltf_decoder_test_khr_materials_sheen_extension() {
    {
        let scene = decode_gltf_file_to_scene("Box/glTF/Box.gltf").expect("scene");
        assert_eq!(scene.material_library().num_materials(), 1);
        let material = scene.material_library().material(0).expect("material");
        assert!(!material.has_sheen());
        assert_eq!(material.sheen_color_factor(), Vector3f::new3(0.0, 0.0, 0.0));
        assert_eq!(material.sheen_roughness_factor(), 0.0f32);
        assert!(material
            .texture_map_by_type(TextureMapType::SheenColor)
            .is_none());
        assert!(material
            .texture_map_by_type(TextureMapType::SheenRoughness)
            .is_none());
    }

    {
        let mesh =
            decode_gltf_file("KhronosSampleModels/SheenCloth/glTF/SheenCloth.gltf").expect("mesh");
        let material = mesh.material_library().material(0).expect("material");
        assert!(material.has_sheen());
        assert_eq!(material.sheen_color_factor(), Vector3f::new3(1.0, 1.0, 1.0));
        assert_eq!(material.sheen_roughness_factor(), 1.0f32);
        assert!(material
            .texture_map_by_type(TextureMapType::SheenColor)
            .is_some());
        assert!(material
            .texture_map_by_type(TextureMapType::SheenRoughness)
            .is_some());
        let color_tex = material
            .texture_map_by_type(TextureMapType::SheenColor)
            .unwrap()
            .texture()
            .expect("texture");
        let rough_tex = material
            .texture_map_by_type(TextureMapType::SheenRoughness)
            .unwrap()
            .texture()
            .expect("texture");
        assert!(std::ptr::eq(color_tex, rough_tex));
    }

    {
        let scene =
            decode_gltf_file_to_scene("KhronosSampleModels/SheenCloth/glTF/SheenCloth.gltf")
                .expect("scene");
        assert_eq!(scene.material_library().num_materials(), 1);
        let material = scene.material_library().material(0).expect("material");
        assert!(material.has_sheen());
        assert_eq!(material.sheen_color_factor(), Vector3f::new3(1.0, 1.0, 1.0));
        assert_eq!(material.sheen_roughness_factor(), 1.0f32);
        assert!(material
            .texture_map_by_type(TextureMapType::SheenColor)
            .is_some());
        assert!(material
            .texture_map_by_type(TextureMapType::SheenRoughness)
            .is_some());
        let color_tex = material
            .texture_map_by_type(TextureMapType::SheenColor)
            .unwrap()
            .texture()
            .expect("texture");
        let rough_tex = material
            .texture_map_by_type(TextureMapType::SheenRoughness)
            .unwrap()
            .texture()
            .expect("texture");
        assert!(std::ptr::eq(color_tex, rough_tex));
    }
}

#[test]
fn gltf_decoder_test_pbr_next_extensions() {
    {
        let scene = decode_gltf_file_to_scene("Box/glTF/Box.gltf").expect("scene");
        assert_eq!(scene.material_library().num_materials(), 1);
        let material = scene.material_library().material(0).expect("material");
        assert!(!material.has_sheen());
        assert!(!material.has_transmission());
        assert!(!material.has_clearcoat());
        assert!(!material.has_volume());
        assert!(!material.has_ior());
        assert!(!material.has_specular());
    }

    {
        let mesh = decode_gltf_file("pbr_next/sphere/glTF/sphere.gltf").expect("mesh");
        let material = mesh.material_library().material(0).expect("material");
        assert!(material.has_sheen());
        assert!(material.has_transmission());
        assert!(material.has_clearcoat());
        assert!(material.has_volume());
        assert!(material.has_ior());
        assert!(material.has_specular());

        assert_eq!(
            material.sheen_color_factor(),
            Vector3f::new3(1.0f32, 0.329f32, 0.1f32)
        );
        assert_eq!(material.sheen_roughness_factor(), 0.8f32);
        assert_eq!(material.transmission_factor(), 0.75f32);
        assert_eq!(material.clearcoat_factor(), 0.95f32);
        assert_eq!(material.clearcoat_roughness_factor(), 0.03f32);
        assert_eq!(
            material.attenuation_color(),
            Vector3f::new3(0.921, 0.640, 0.064)
        );
        assert_eq!(material.attenuation_distance(), 0.155f32);
        assert_eq!(material.thickness_factor(), 2.27f32);
        assert_eq!(material.ior(), 1.55f32);
        assert_eq!(material.specular_factor(), 0.3f32);
        assert_eq!(
            material.specular_color_factor(),
            Vector3f::new3(0.212f32, 0.521f32, 0.051f32)
        );

        assert!(material
            .texture_map_by_type(TextureMapType::SheenColor)
            .is_some());
        assert!(material
            .texture_map_by_type(TextureMapType::SheenRoughness)
            .is_some());
        assert!(material
            .texture_map_by_type(TextureMapType::Transmission)
            .is_some());
        assert!(material
            .texture_map_by_type(TextureMapType::Clearcoat)
            .is_some());
        assert!(material
            .texture_map_by_type(TextureMapType::ClearcoatRoughness)
            .is_some());
        assert!(material
            .texture_map_by_type(TextureMapType::ClearcoatNormal)
            .is_some());
        assert!(material
            .texture_map_by_type(TextureMapType::Thickness)
            .is_some());
        assert!(material
            .texture_map_by_type(TextureMapType::Specular)
            .is_some());
        assert!(material
            .texture_map_by_type(TextureMapType::SpecularColor)
            .is_some());
    }
}

#[test]
fn gltf_decoder_test_texture_transform_test() {
    let filename = "KhronosSampleModels/TextureTransformTest/glTF/TextureTransformTest.gltf";
    let mesh = decode_gltf_file(filename).expect("mesh");
    assert_eq!(mesh.material_library().num_materials(), 9);
    let expected_default_transforms: HashSet<i32> = [4, 5, 6].into_iter().collect();
    for i in 0..9 {
        let expected_default = expected_default_transforms.contains(&i);
        let transform = mesh
            .material_library()
            .material(i)
            .unwrap()
            .texture_map_by_index(0)
            .unwrap()
            .texture_transform();
        assert_eq!(TextureTransform::is_default(&transform), expected_default);
    }

    let scene = decode_gltf_file_to_scene(filename).expect("scene");
    assert_eq!(scene.material_library().num_materials(), 9);
    for i in 0..6 {
        let transform = scene
            .material_library()
            .material(i)
            .unwrap()
            .texture_map_by_index(0)
            .unwrap()
            .texture_transform();
        assert!(!TextureTransform::is_default(&transform));
    }
    for i in 6..9 {
        let transform = scene
            .material_library()
            .material(i)
            .unwrap()
            .texture_map_by_index(0)
            .unwrap()
            .texture_transform();
        assert!(TextureTransform::is_default(&transform));
    }
}

#[test]
fn gltf_decoder_test_glb_texture_source() {
    let scene =
        decode_gltf_file_to_scene("KhronosSampleModels/Duck/glTF_Binary/Duck.glb").expect("scene");
    assert_eq!(scene.num_meshes(), 1);
    assert_eq!(scene.num_mesh_groups(), 1);
    let mesh_group = scene.mesh_group(MeshGroupIndex::from(0));
    assert_eq!(mesh_group.num_mesh_instances(), 1);
    assert_eq!(mesh_group.mesh_instance(0).material_index, 0);
    assert_eq!(scene.num_nodes(), 3);
    assert_eq!(scene.num_root_nodes(), 1);
    assert_eq!(scene.material_library().num_materials(), 1);
    assert_eq!(
        scene
            .material_library()
            .material(0)
            .unwrap()
            .num_texture_maps(),
        1
    );
    assert_eq!(scene.num_animations(), 0);
    assert_eq!(scene.num_skins(), 0);
    assert_eq!(scene.material_library().texture_library().num_textures(), 1);
    let texture = scene
        .material_library()
        .texture_library()
        .texture(0)
        .expect("texture");
    let source_image = texture.source_image();
    assert_eq!(source_image.encoded_data().len(), 16302);
    assert!(source_image.filename().is_empty());
    assert_eq!(source_image.mime_type(), "image/png");
}

#[test]
fn gltf_decoder_test_gltf_texture_source() {
    let scene =
        decode_gltf_file_to_scene("KhronosSampleModels/Duck/glTF/Duck.gltf").expect("scene");
    assert_eq!(scene.num_meshes(), 1);
    assert_eq!(scene.num_mesh_groups(), 1);
    let mesh_group = scene.mesh_group(MeshGroupIndex::from(0));
    assert_eq!(mesh_group.num_mesh_instances(), 1);
    assert_eq!(mesh_group.mesh_instance(0).material_index, 0);
    assert_eq!(scene.num_nodes(), 3);
    assert_eq!(scene.num_root_nodes(), 1);
    assert_eq!(scene.material_library().num_materials(), 1);
    assert_eq!(
        scene
            .material_library()
            .material(0)
            .unwrap()
            .num_texture_maps(),
        1
    );
    assert_eq!(scene.num_animations(), 0);
    assert_eq!(scene.num_skins(), 0);
    assert_eq!(scene.material_library().texture_library().num_textures(), 1);
    let texture = scene
        .material_library()
        .texture_library()
        .texture(0)
        .expect("texture");
    let source_image = texture.source_image();
    assert_eq!(source_image.encoded_data().len(), 0);
    assert!(!source_image.filename().is_empty());
    assert!(source_image.mime_type().is_empty());
}

#[test]
fn gltf_decoder_test_gltf_decode_with_draco() {
    let scene = decode_gltf_file_to_scene("Box/glTF_Binary/Box.glb").expect("scene");
    let scene_draco = decode_gltf_file_to_scene("Box/glTF_Binary/Box_Draco.glb").expect("scene");
    assert_eq!(scene.num_meshes(), scene_draco.num_meshes());
    assert_eq!(scene.num_mesh_groups(), scene_draco.num_mesh_groups());
    assert_eq!(scene.num_nodes(), scene_draco.num_nodes());
    assert_eq!(scene.num_root_nodes(), scene_draco.num_root_nodes());
    assert_eq!(
        scene.material_library().num_materials(),
        scene_draco.material_library().num_materials()
    );
    assert_eq!(scene.num_animations(), scene_draco.num_animations());
    assert_eq!(scene.num_skins(), scene_draco.num_skins());

    assert_eq!(scene.num_meshes(), 1);
    assert_eq!(
        scene.mesh(MeshIndex::from(0)).num_faces(),
        scene_draco.mesh(MeshIndex::from(0)).num_faces()
    );
}

#[test]
fn gltf_decoder_test_animation_names() {
    let scene =
        decode_gltf_file_to_scene("InterpolationTest/glTF/InterpolationTest.gltf").expect("scene");
    assert_eq!(scene.num_animations(), 9);
    let animation_names = vec![
        "Step Scale",
        "Linear Scale",
        "CubicSpline Scale",
        "Step Rotation",
        "CubicSpline Rotation",
        "Linear Rotation",
        "Step Translation",
        "CubicSpline Translation",
        "Linear Translation",
    ];
    for i in 0..scene.num_animations() {
        let anim = scene.animation(AnimationIndex::from(i as u32));
        assert_eq!(anim.name(), animation_names[i as usize]);
    }
}

#[test]
fn gltf_decoder_test_duplicate_primitives() {
    let scene = decode_gltf_file_to_scene("DuplicateMeshes/duplicate_meshes.gltf").expect("scene");
    assert_eq!(scene.num_meshes(), 1);
    assert_eq!(scene.num_mesh_groups(), 4);
    assert_eq!(scene.material_library().num_materials(), 2);
}

#[test]
fn gltf_decoder_test_simple_skin() {
    let scene = decode_gltf_file_to_scene("simple_skin.gltf").expect("scene");

    assert_eq!(scene.num_meshes(), 1);
    assert_eq!(scene.num_mesh_groups(), 1);
    assert_eq!(
        scene
            .mesh_group(MeshGroupIndex::from(0))
            .num_mesh_instances(),
        1
    );
    assert_eq!(scene.num_nodes(), 3);
    assert_eq!(scene.num_root_nodes(), 1);
    assert_eq!(scene.material_library().num_materials(), 1);
    assert_eq!(scene.num_animations(), 1);
    assert_eq!(scene.num_skins(), 1);

    let animation = scene.animation(AnimationIndex::from(0));
    assert_eq!(animation.num_samplers(), 1);
    assert_eq!(animation.num_channels(), 1);
    assert_eq!(animation.num_node_animation_data(), 2);

    let sampler = animation.sampler(0).expect("sampler");
    assert_eq!(sampler.input_index, 0);
    assert_eq!(sampler.interpolation_type, SamplerInterpolation::Linear);
    assert_eq!(sampler.output_index, 1);

    let channel = animation.channel(0).expect("channel");
    assert_eq!(channel.sampler_index, 0);
    assert_eq!(channel.target_index, 2);
    assert_eq!(channel.transformation_type, ChannelTransformation::Rotation);

    {
        let node_animation = animation.node_animation_data(0).expect("node data");
        assert_eq!(node_animation.component_size(), 4);
        assert_eq!(node_animation.num_components(), 1);
        assert_eq!(node_animation.count(), 12);
        assert_eq!(node_animation.data_type(), NodeAnimationDataType::Scalar);
        assert!(!node_animation.normalized());
        let expected = vec![
            0.0f32, 0.5f32, 1.0f32, 1.5f32, 2.0f32, 2.5f32, 3.0f32, 3.5f32, 4.0f32, 4.5f32, 5.0f32,
            5.5f32,
        ];
        assert_eq!(node_animation.data(), &expected);
    }

    {
        let node_animation = animation.node_animation_data(1).expect("node data");
        assert_eq!(node_animation.component_size(), 4);
        assert_eq!(node_animation.num_components(), 4);
        assert_eq!(node_animation.count(), 12);
        assert_eq!(node_animation.data_type(), NodeAnimationDataType::Vec4);
        assert!(!node_animation.normalized());
        let expected = vec![
            0.000f32, 0.000f32, 0.000f32, 1.000f32, 0.000f32, 0.000f32, 0.383f32, 0.924f32,
            0.000f32, 0.000f32, 0.707f32, 0.707f32, 0.000f32, 0.000f32, 0.707f32, 0.707f32,
            0.000f32, 0.000f32, 0.383f32, 0.924f32, 0.000f32, 0.000f32, 0.000f32, 1.000f32,
            0.000f32, 0.000f32, 0.000f32, 1.000f32, 0.000f32, 0.000f32, -0.383f32, 0.924f32,
            0.000f32, 0.000f32, -0.707f32, 0.707f32, 0.000f32, 0.000f32, -0.707f32, 0.707f32,
            0.000f32, 0.000f32, -0.383f32, 0.924f32, 0.000f32, 0.000f32, 0.000f32, 1.000f32,
        ];
        assert_eq!(node_animation.data(), &expected);
    }

    let skin = scene.skin(SkinIndex::from(0));
    assert_eq!(skin.num_joints(), 2);
    assert_eq!(skin.joint_root(), crate::scene::INVALID_SCENE_NODE_INDEX);
    assert_eq!(skin.joint(0), SceneNodeIndex::from(1));
    assert_eq!(skin.joint(1), SceneNodeIndex::from(2));

    let bind_matrices = skin.inverse_bind_matrices();
    assert_eq!(bind_matrices.data_type(), NodeAnimationDataType::Mat4);
    assert_eq!(bind_matrices.count(), 2);
    assert!(!bind_matrices.normalized());
    let expected = vec![
        1.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 1.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 1.0f32,
        0.0f32, -0.5f32, -1.0f32, 0.0f32, 1.0f32, 1.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 1.0f32,
        0.0f32, 0.0f32, 0.0f32, 0.0f32, 1.0f32, 0.0f32, -0.5f32, -1.0f32, 0.0f32, 1.0f32,
    ];
    assert_eq!(bind_matrices.data(), &expected);

    let mesh = scene.mesh(MeshIndex::from(0));
    assert_eq!(mesh.num_faces(), 8);
    assert_eq!(mesh.num_points(), 10);
    assert_eq!(mesh.num_attributes(), 3);

    let joints_att = mesh
        .get_named_attribute(GeometryAttributeType::Joints)
        .expect("JOINTS");
    assert_eq!(
        joints_att.data_type(),
        draco_core::core::draco_types::DataType::Uint16
    );
    assert_eq!(joints_att.num_components(), 4);
    assert_eq!(joints_att.size(), 1);
    let expected_joints: [u16; 40] = [
        0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1,
        0, 0, 0, 1, 0, 0, 0, 1, 0, 0,
    ];
    let mut joints = [0u16; 40];
    for pi in 0..mesh.num_points() {
        let mut value = [0u16; 4];
        joints_att.convert_value(joints_att.mapped_index(PointIndex::from(pi)), 4, &mut value);
        let offset = pi as usize * 4;
        joints[offset..offset + 4].copy_from_slice(&value);
    }
    assert_eq!(joints, expected_joints);

    let weights_att = mesh
        .get_named_attribute(GeometryAttributeType::Weights)
        .expect("WEIGHTS");
    assert_eq!(
        weights_att.data_type(),
        draco_core::core::draco_types::DataType::Float32
    );
    assert_eq!(weights_att.num_components(), 4);
    assert_eq!(weights_att.size(), 5);
    let expected_weights: [f32; 40] = [
        1.00, 0.00, 0.00, 0.00, 1.00, 0.00, 0.00, 0.00, 0.75, 0.25, 0.00, 0.00, 0.75, 0.25, 0.00,
        0.00, 0.50, 0.50, 0.00, 0.00, 0.50, 0.50, 0.00, 0.00, 0.25, 0.75, 0.00, 0.00, 0.25, 0.75,
        0.00, 0.00, 0.00, 1.00, 0.00, 0.00, 0.00, 1.00, 0.00, 0.00,
    ];
    let mut weights = [0.0f32; 40];
    for pi in 0..mesh.num_points() {
        let mut value = [0.0f32; 4];
        weights_att.convert_value(
            weights_att.mapped_index(PointIndex::from(pi)),
            4,
            &mut value,
        );
        let offset = pi as usize * 4;
        weights[offset..offset + 4].copy_from_slice(&value);
    }
    assert_eq!(weights, expected_weights);
}

#[test]
fn gltf_decoder_test_decode_mesh_with_implicit_primitive_indices() {
    let mesh = decode_gltf_file("Fox/glTF/Fox.gltf").expect("mesh");
    assert_eq!(mesh.num_faces(), 576);
}

#[test]
fn gltf_decoder_test_decode_scene_with_implicit_primitive_indices() {
    let scene = decode_gltf_file_to_scene("Fox/glTF/Fox.gltf").expect("scene");
    assert_eq!(scene.num_meshes(), 1);
    assert_eq!(scene.mesh(MeshIndex::from(0)).num_faces(), 576);
}

#[test]
fn gltf_decoder_test_decode_from_buffer_to_mesh() {
    let file_name = "KhronosSampleModels/Duck/glTF_Binary/Duck.glb";
    let file_path = get_test_file_full_path(file_name);
    let mut file_data: Vec<u8> = Vec::new();
    assert!(file_utils::read_file_to_buffer(&file_path, &mut file_data));
    let mut buffer = DecoderBuffer::new();
    buffer.init(&file_data);

    let mut decoder = GltfDecoder::new();
    let mesh = draco_assign_or_assert!(decoder.decode_from_buffer(&buffer));

    let expected_mesh = decode_gltf_file(file_name).expect("mesh");
    let eq = MeshAreEquivalent::new();
    assert!(eq.are_equivalent(mesh.as_ref(), expected_mesh.as_ref()));
}

#[test]
fn gltf_decoder_test_decode_graph() {
    let file_name = "CubeScaledInstances/glTF/cube_att.gltf";
    let file_path = get_test_file_full_path(file_name);

    let mut dec_tree = GltfDecoder::new();
    let scene_tree = draco_assign_or_assert!(dec_tree.decode_from_file_to_scene(&file_path, None));
    assert_eq!(scene_tree.num_nodes(), 9);
    let instances_tree = SceneUtils::compute_all_instances(&scene_tree);
    assert_eq!(instances_tree.size(), 4);

    let mut dec_graph = GltfDecoder::new();
    dec_graph.set_scene_graph_mode(GltfSceneGraphMode::Dag);
    let scene_graph =
        draco_assign_or_assert!(dec_graph.decode_from_file_to_scene(&file_path, None));
    assert_eq!(scene_graph.num_nodes(), 6);
    let instances_graph = SceneUtils::compute_all_instances(&scene_graph);
    assert_eq!(instances_graph.size(), 4);

    for mii in 1..4 {
        assert_eq!(
            instances_graph[MeshInstanceIndex::from((mii - 1) as u32)].scene_node_index,
            instances_graph[MeshInstanceIndex::from(mii as u32)].scene_node_index
        );
    }
}

#[test]
fn gltf_decoder_test_correct_volume_thickness_factor() {
    const DRAGON_SCALE: f32 = 0.25;
    const DRAGON_VOLUME_THICKNESS: f32 = 2.27;

    let scene = draco_assign_or_assert!(read_scene_from_test_file(
        "KhronosSampleModels/DragonAttenuation/glTF/DragonAttenuation.gltf",
    ));
    let instances = SceneUtils::compute_all_instances(&scene);
    assert_eq!(instances.size(), 2);
    let col_norm = matrix_col_norm(&instances[MeshInstanceIndex::from(1)].transform, 0);
    assert!((col_norm as f32 - DRAGON_SCALE).abs() < 1e-6);
    assert_eq!(
        scene
            .material_library()
            .material(1)
            .unwrap()
            .thickness_factor(),
        DRAGON_VOLUME_THICKNESS
    );

    let mesh = draco_assign_or_assert!(read_mesh_from_test_file(
        "KhronosSampleModels/DragonAttenuation/glTF/DragonAttenuation.gltf",
    ));
    assert_eq!(
        mesh.material_library()
            .material(1)
            .unwrap()
            .thickness_factor(),
        DRAGON_SCALE * DRAGON_VOLUME_THICKNESS
    );
}

#[test]
fn gltf_decoder_test_decode_lights_into_mesh() {
    let mesh = decode_gltf_file("sphere_lights.gltf").expect("mesh");
    assert_eq!(mesh.num_faces(), 224);
}

#[test]
fn gltf_decoder_test_decode_lights_into_scene() {
    let scene = decode_gltf_file_to_scene("sphere_lights.gltf").expect("scene");
    assert_eq!(scene.num_lights(), 4);

    let light = scene.light(LightIndex::from(0));
    assert_eq!(light.name(), "Blue Lightsaber");
    assert_eq!(*light.color(), Vector3f::new3(0.72, 0.71, 1.00));
    assert_eq!(light.intensity(), 3.0);
    assert_eq!(light.light_type(), crate::scene::LightType::Spot);
    assert_eq!(light.range(), 100.0);
    assert_eq!(light.inner_cone_angle(), 0.2);
    assert_eq!(light.outer_cone_angle(), 0.8);

    let light = scene.light(LightIndex::from(1));
    assert_eq!(light.name(), "The Star of Earendil");
    assert_eq!(*light.color(), Vector3f::new3(0.90, 0.97, 1.0));
    assert_eq!(light.intensity(), 5.0);
    assert_eq!(light.light_type(), crate::scene::LightType::Point);
    assert_eq!(light.range(), 1000.0);
    assert_eq!(light.inner_cone_angle(), 0.0);
    assert!((light.outer_cone_angle() - (DRACO_PI / 4.0)).abs() < 1e-8);

    let light = scene.light(LightIndex::from(2));
    assert_eq!(light.name(), "Arc Reactor");
    assert_eq!(*light.color(), Vector3f::new3(0.9, 0.9, 0.9));
    assert_eq!(light.intensity(), 1.0);
    assert_eq!(light.light_type(), crate::scene::LightType::Directional);
    assert_eq!(light.range(), 200.0);

    let light = scene.light(LightIndex::from(3));
    assert_eq!(light.name(), "");
    assert_eq!(*light.color(), Vector3f::new3(1.0, 1.0, 1.0));
    assert_eq!(light.intensity(), 1.0);
    assert_eq!(light.light_type(), crate::scene::LightType::Spot);
    assert_eq!(light.range(), f32::MAX as f64);
    assert_eq!(light.inner_cone_angle(), 0.0);
    assert!((light.outer_cone_angle() - (DRACO_PI / 4.0)).abs() < 1e-8);

    assert_eq!(
        scene.node(SceneNodeIndex::from(0)).light_index(),
        INVALID_LIGHT_INDEX
    );
    assert_eq!(
        scene.node(SceneNodeIndex::from(1)).light_index(),
        LightIndex::from(0)
    );
    assert_eq!(
        scene.node(SceneNodeIndex::from(2)).light_index(),
        LightIndex::from(2)
    );
    assert_eq!(
        scene.node(SceneNodeIndex::from(3)).light_index(),
        LightIndex::from(3)
    );
    assert_eq!(
        scene.node(SceneNodeIndex::from(4)).light_index(),
        LightIndex::from(1)
    );
}

#[test]
fn gltf_decoder_test_materials_variants() {
    let mut decoder = GltfDecoder::new();
    let scene = draco_assign_or_assert!(decoder.decode_from_file_to_scene(
        &get_test_file_full_path(
            "KhronosSampleModels/DragonAttenuation/glTF/DragonAttenuation.gltf",
        ),
        None
    ));
    let library = scene.material_library();
    assert_eq!(library.num_materials_variants(), 2);
    assert_eq!(library.materials_variant_name(0), "Attenuation");
    assert_eq!(library.materials_variant_name(1), "Surface Color");

    let cloth_group = scene.mesh_group(MeshGroupIndex::from(0));
    assert_eq!(cloth_group.name(), "Cloth Backdrop");
    assert_eq!(cloth_group.num_mesh_instances(), 1);
    let cloth_mappings = &cloth_group.mesh_instance(0).materials_variants_mappings;
    assert_eq!(cloth_mappings.len(), 0);

    let dragon_group = scene.mesh_group(MeshGroupIndex::from(1));
    assert_eq!(dragon_group.name(), "Dragon");
    assert_eq!(dragon_group.num_mesh_instances(), 1);
    let dragon_mappings = &dragon_group.mesh_instance(0).materials_variants_mappings;
    assert_eq!(dragon_mappings.len(), 2);
    assert_eq!(dragon_mappings[0].material, 1);
    assert_eq!(dragon_mappings[1].material, 2);
    assert_eq!(dragon_mappings[0].variants.len(), 1);
    assert_eq!(dragon_mappings[1].variants.len(), 1);
    assert_eq!(dragon_mappings[0].variants[0], 0);
    assert_eq!(dragon_mappings[1].variants[0], 1);
}

#[test]
fn gltf_decoder_test_decode_mesh_with_mesh_features_with_structural_metadata() {
    let path = get_test_file_full_path("BoxMeta/glTF/BoxMeta.gltf");
    let mut use_case = UseCase::default();
    use_case.has_mesh_features = true;
    use_case.has_structural_metadata = true;

    let mut decoder = GltfDecoder::new();
    let mesh = draco_assign_or_assert!(decoder.decode_from_file(&path, None));
    GltfTestHelper::check_box_meta_mesh_features_mesh(mesh.as_ref(), &use_case);
    GltfTestHelper::check_box_meta_structural_metadata_mesh(mesh.as_ref(), &use_case);
}

#[test]
fn gltf_decoder_test_decode_mesh_with_structural_metadata_with_empty_string_buffer() {
    let path = get_test_file_full_path("ZeroLengthBufferView/ZeroLengthBufferView.gltf");
    let mut decoder = GltfDecoder::new();
    let mesh = draco_assign_or_assert!(decoder.decode_from_file(&path, None));
    let metadata = mesh.structural_metadata();
    assert_eq!(metadata.num_property_tables(), 1);
    let table = metadata.property_table(0);
    assert_eq!(table.count(), 1);
    assert_eq!(table.num_properties(), 1);
    assert_eq!(table.property(0).data().data.len(), 0);
}

#[test]
fn gltf_decoder_test_decode_mesh_with_mesh_features_with_draco_compression() {
    let path = get_test_file_full_path("BoxMetaDraco/glTF/BoxMetaDraco.gltf");
    let mut use_case = UseCase::default();
    use_case.has_draco_compression = true;
    use_case.has_mesh_features = true;

    let mut decoder = GltfDecoder::new();
    let mesh = draco_assign_or_assert!(decoder.decode_from_file(&path, None));
    GltfTestHelper::check_box_meta_mesh_features_mesh(mesh.as_ref(), &use_case);
}

#[test]
fn gltf_decoder_test_decode_scene_with_mesh_features_with_structural_metadata() {
    let path = get_test_file_full_path("BoxMeta/glTF/BoxMeta.gltf");
    let mut use_case = UseCase::default();
    use_case.has_mesh_features = true;
    use_case.has_structural_metadata = true;

    let mut decoder = GltfDecoder::new();
    let scene = draco_assign_or_assert!(decoder.decode_from_file_to_scene(&path, None));
    GltfTestHelper::check_box_meta_mesh_features_scene(scene.as_ref(), &use_case);
    GltfTestHelper::check_box_meta_structural_metadata_scene(scene.as_ref(), &use_case);
}

#[test]
fn gltf_decoder_test_decode_scene_with_mesh_features_with_draco_compression() {
    let path = get_test_file_full_path("BoxMetaDraco/glTF/BoxMetaDraco.gltf");
    let mut use_case = UseCase::default();
    use_case.has_draco_compression = true;
    use_case.has_mesh_features = true;

    let mut decoder = GltfDecoder::new();
    let scene = draco_assign_or_assert!(decoder.decode_from_file_to_scene(&path, None));
    GltfTestHelper::check_box_meta_mesh_features_scene(scene.as_ref(), &use_case);
}

#[test]
fn gltf_decoder_test_decode_point_cloud_to_mesh() {
    let path = get_test_file_full_path("SphereTwoMaterials/sphere_two_materials_point_cloud.gltf");
    let mut decoder = GltfDecoder::new();
    let mesh = draco_assign_or_assert!(decoder.decode_from_file(&path, None));

    assert_eq!(mesh.num_faces(), 0);
    assert_eq!(mesh.num_points(), 462);

    assert_eq!(mesh.num_named_attributes(GeometryAttributeType::Normal), 1);
    assert_eq!(
        mesh.num_named_attributes(GeometryAttributeType::TexCoord),
        1
    );
    assert_eq!(mesh.num_named_attributes(GeometryAttributeType::Tangent), 1);
    assert_eq!(
        mesh.num_named_attributes(GeometryAttributeType::Material),
        1
    );

    assert!(
        mesh.get_named_attribute(GeometryAttributeType::Normal)
            .unwrap()
            .size()
            < 462
    );

    assert_eq!(
        mesh.get_named_attribute(GeometryAttributeType::Material)
            .unwrap()
            .size(),
        2
    );
}

#[test]
fn gltf_decoder_test_decode_mesh_and_point_cloud_to_mesh() {
    let path = get_test_file_full_path(
        "SphereTwoMaterials/sphere_two_materials_mesh_and_point_cloud.gltf",
    );
    let mut decoder = GltfDecoder::new();
    assert!(!decoder.decode_from_file(&path, None).is_ok());
}

#[test]
fn gltf_decoder_test_decode_point_cloud_to_scene() {
    let path = get_test_file_full_path("SphereTwoMaterials/sphere_two_materials_point_cloud.gltf");
    let mut decoder = GltfDecoder::new();
    let scene = draco_assign_or_assert!(decoder.decode_from_file_to_scene(&path, None));

    assert_eq!(scene.num_meshes(), 2);

    for mi in 0..scene.num_meshes() {
        let mesh = scene.mesh(MeshIndex::from(mi as u32));
        assert_eq!(mesh.num_faces(), 0);
        assert_eq!(mesh.num_points(), 231);
        assert_eq!(mesh.num_named_attributes(GeometryAttributeType::Normal), 1);
        assert_eq!(
            mesh.num_named_attributes(GeometryAttributeType::TexCoord),
            1
        );
        assert_eq!(mesh.num_named_attributes(GeometryAttributeType::Tangent), 1);
        assert_eq!(
            mesh.num_named_attributes(GeometryAttributeType::Material),
            0
        );
    }

    let instances = SceneUtils::compute_all_instances(&scene);
    assert_eq!(instances.size(), 2);
    assert_eq!(
        SceneUtils::get_mesh_instance_material_index(
            &scene,
            &instances[MeshInstanceIndex::from(0)]
        ),
        0
    );
    assert_eq!(
        SceneUtils::get_mesh_instance_material_index(
            &scene,
            &instances[MeshInstanceIndex::from(1)]
        ),
        1
    );
}

#[test]
fn gltf_decoder_test_decode_mesh_and_point_cloud_to_scene() {
    let path = get_test_file_full_path(
        "SphereTwoMaterials/sphere_two_materials_mesh_and_point_cloud.gltf",
    );
    let mut decoder = GltfDecoder::new();
    let scene = draco_assign_or_assert!(decoder.decode_from_file_to_scene(&path, None));

    assert_eq!(scene.num_meshes(), 2);

    for mi in 0..scene.num_meshes() {
        let mesh = scene.mesh(MeshIndex::from(mi as u32));
        assert_eq!(mesh.num_faces(), if mi == 0 { 224 } else { 0 });
        assert_eq!(mesh.num_points(), 231);
        assert_eq!(mesh.num_named_attributes(GeometryAttributeType::Normal), 1);
        assert_eq!(
            mesh.num_named_attributes(GeometryAttributeType::TexCoord),
            1
        );
        assert_eq!(mesh.num_named_attributes(GeometryAttributeType::Tangent), 1);
    }
}

#[test]
fn gltf_decoder_test_load_unsupported_texcoord_attributes() {
    let scene =
        draco_assign_or_assert!(read_scene_from_test_file("UnusedTexCoords/TexCoord2.gltf"));
    assert_eq!(
        scene
            .mesh(MeshIndex::from(0))
            .num_named_attributes(GeometryAttributeType::TexCoord),
        2
    );
}

#[test]
fn gltf_decoder_test_inverted_materials() {
    let mesh = draco_assign_or_assert!(read_mesh_from_test_file(
        "two_objects_inverse_materials.gltf"
    ));
    assert_eq!(mesh.material_library().num_materials(), 2);
    assert_eq!(mesh.material_library().material(0).unwrap().name(), "Red");
    assert_eq!(mesh.material_library().material(1).unwrap().name(), "Green");

    let mut num_material_faces = [0i32; 2];
    let mat_att = mesh
        .get_named_attribute(GeometryAttributeType::Material)
        .expect("MATERIAL");
    for i in 0..mesh.num_faces() {
        let f = mesh.face(FaceIndex::from(i as u32));
        let mut value = [0u32; 1];
        mat_att.convert_value(mat_att.mapped_index(f[0]), 1, &mut value);
        let mat_index = value[0] as usize;
        assert!(mat_index == 0 || mat_index == 1);
        num_material_faces[mat_index] += 1;
    }
    assert_eq!(num_material_faces[0], 12);
}

#[test]
fn gltf_decoder_test_point_cloud_to_mesh_with_deduplication_disabled() {
    let path = get_test_file_full_path("SphereTwoMaterials/sphere_two_materials_point_cloud.gltf");
    let mut decoder = GltfDecoder::new();
    decoder.set_deduplicate_vertices(false);
    let mesh = draco_assign_or_assert!(decoder.decode_from_file(&path, None));

    assert_eq!(mesh.num_faces(), 0);
    assert_eq!(mesh.num_points(), 462);

    assert_eq!(
        mesh.get_named_attribute(GeometryAttributeType::Normal)
            .unwrap()
            .size(),
        462
    );
}
