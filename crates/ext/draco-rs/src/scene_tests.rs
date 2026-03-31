//! Scene tests ported from Draco C++ reference.
//!
//! What: Covers `scene_test.cc` and `scene_are_equivalent_test.cc` parity.
//! Why: Validates scene copy/removal behavior and equivalence checks.
//! Where used: `cargo test -p draco-rs scene_`.

use std::env;
use std::path::PathBuf;

use crate::io::scene_io;
use crate::mesh::MeshAreEquivalent;
use crate::scene::{
    Instance, MeshGroupIndex, MeshIndex, Quaterniond, Scene, SceneAreEquivalent, SceneNodeIndex,
    Vector3d, INVALID_INSTANCE_ARRAY_INDEX,
};
use draco_core::core::status::{ok_status, Status};
use draco_core::metadata::metadata::MetadataString;
use draco_core::metadata::structural_metadata_schema::StructuralMetadataSchema;

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

fn read_scene(file_name: &str) -> Box<Scene> {
    let path = test_data_dir()
        .join(file_name)
        .to_string_lossy()
        .into_owned();
    draco_assign_or_assert!(scene_io::read_scene_from_file(&path))
}

fn add_gpu_instancing_to_milk_truck(scene: &mut Scene) -> Status {
    let mut instance_0 = Instance::default();
    instance_0.trs.set_translation(Vector3d::new(1.0, 2.0, 3.0));
    instance_0
        .trs
        .set_rotation(Quaterniond::new(4.0, 5.0, 6.0, 7.0));
    instance_0.trs.set_scale(Vector3d::new(8.0, 9.0, 10.0));

    let mut instance_1 = Instance::default();
    instance_1.trs.set_translation(Vector3d::new(1.1, 2.1, 3.1));
    instance_1
        .trs
        .set_rotation(Quaterniond::new(4.1, 5.1, 6.1, 7.1));
    instance_1.trs.set_scale(Vector3d::new(8.1, 9.1, 10.1));

    let index = scene.add_instance_array();
    {
        let gpu_instancing = scene.instance_array_mut(index);
        let status = gpu_instancing.add_instance(&instance_0);
        if !status.is_ok() {
            return status;
        }
        let status = gpu_instancing.add_instance(&instance_1);
        if !status.is_ok() {
            return status;
        }
    }

    scene
        .node_mut(SceneNodeIndex::from(2))
        .set_instance_array_index(index);
    scene
        .node_mut(SceneNodeIndex::from(4))
        .set_instance_array_index(index);

    ok_status()
}

fn check_mesh_materials(scene: &Scene, expected_material_indices: &[i32]) {
    assert_eq!(scene.num_meshes() as usize, expected_material_indices.len());
    let mut scene_material_indices: Vec<i32> = Vec::new();
    for i in 0..scene.num_mesh_groups() {
        let group = scene.mesh_group(MeshGroupIndex::from(i as u32));
        for mi in 0..group.num_mesh_instances() {
            scene_material_indices.push(group.mesh_instance(mi).material_index);
        }
    }
    assert_eq!(scene_material_indices, expected_material_indices);
}

#[test]
fn scene_are_equivalent_identical_scenes() {
    let file_name = "CesiumMilkTruck/glTF/CesiumMilkTruck.gltf";
    let mut scene = read_scene(file_name);

    scene
        .mesh_mut(MeshIndex::from(2))
        .add_mesh_features(Box::new(draco_core::mesh::MeshFeatures::new()));

    let equiv = SceneAreEquivalent::new();
    assert!(equiv.are_equivalent(&scene, &scene));
}

#[test]
fn scene_are_equivalent_different_scenes() {
    let file_name0 = "CesiumMilkTruck/glTF/CesiumMilkTruck.gltf";
    let file_name1 = "Lantern/glTF/Lantern.gltf";
    let scene0 = read_scene(file_name0);
    let scene1 = read_scene(file_name1);

    let equiv = SceneAreEquivalent::new();
    assert!(!equiv.are_equivalent(&scene0, &scene1));
}

#[test]
fn scene_are_equivalent_mesh_features() {
    let file_name = "CesiumMilkTruck/glTF/CesiumMilkTruck.gltf";
    let mut scene0 = read_scene(file_name);
    let mut scene1 = read_scene(file_name);

    scene0
        .mesh_mut(MeshIndex::from(0))
        .add_mesh_features(Box::new(draco_core::mesh::MeshFeatures::new()));
    scene1
        .mesh_mut(MeshIndex::from(0))
        .add_mesh_features(Box::new(draco_core::mesh::MeshFeatures::new()));

    let equiv = SceneAreEquivalent::new();
    assert!(equiv.are_equivalent(&scene0, &scene1));

    scene0
        .mesh_mut(MeshIndex::from(0))
        .mesh_features_mut(draco_core::mesh::MeshFeaturesIndex::from(0))
        .set_feature_count(5);
    scene1
        .mesh_mut(MeshIndex::from(0))
        .mesh_features_mut(draco_core::mesh::MeshFeaturesIndex::from(0))
        .set_feature_count(6);
    assert!(!equiv.are_equivalent(&scene0, &scene1));

    scene0
        .mesh_mut(MeshIndex::from(0))
        .mesh_features_mut(draco_core::mesh::MeshFeaturesIndex::from(0))
        .set_feature_count(1);
    scene1
        .mesh_mut(MeshIndex::from(0))
        .mesh_features_mut(draco_core::mesh::MeshFeaturesIndex::from(0))
        .set_feature_count(1);
    assert!(equiv.are_equivalent(&scene0, &scene1));
}

#[test]
fn scene_copy() {
    let mut src_scene = read_scene("CesiumMilkTruck/glTF/CesiumMilkTruck.gltf");

    draco_assert_ok!(add_gpu_instancing_to_milk_truck(src_scene.as_mut()));
    assert_eq!(src_scene.num_instance_arrays(), 1);
    assert_eq!(src_scene.num_nodes(), 5);
    assert_eq!(
        src_scene
            .node(SceneNodeIndex::from(0))
            .instance_array_index(),
        INVALID_INSTANCE_ARRAY_INDEX
    );
    assert_eq!(
        src_scene
            .node(SceneNodeIndex::from(1))
            .instance_array_index(),
        INVALID_INSTANCE_ARRAY_INDEX
    );
    assert_eq!(
        src_scene
            .node(SceneNodeIndex::from(2))
            .instance_array_index(),
        crate::scene::InstanceArrayIndex::from(0)
    );
    assert_eq!(
        src_scene
            .node(SceneNodeIndex::from(3))
            .instance_array_index(),
        INVALID_INSTANCE_ARRAY_INDEX
    );
    assert_eq!(
        src_scene
            .node(SceneNodeIndex::from(4))
            .instance_array_index(),
        crate::scene::InstanceArrayIndex::from(0)
    );

    let mut dst_scene = Scene::new();
    dst_scene.copy_from(&src_scene);

    assert_eq!(src_scene.num_meshes(), dst_scene.num_meshes());
    assert_eq!(src_scene.num_mesh_groups(), dst_scene.num_mesh_groups());
    assert_eq!(src_scene.num_nodes(), dst_scene.num_nodes());
    assert_eq!(src_scene.num_animations(), dst_scene.num_animations());
    assert_eq!(src_scene.num_skins(), dst_scene.num_skins());
    assert_eq!(src_scene.num_lights(), dst_scene.num_lights());
    assert_eq!(
        src_scene.num_instance_arrays(),
        dst_scene.num_instance_arrays()
    );

    let mesh_eq = MeshAreEquivalent::new();
    for i in 0..src_scene.num_meshes() {
        let idx = MeshIndex::from(i as u32);
        assert!(mesh_eq.are_equivalent(src_scene.mesh(idx), dst_scene.mesh(idx)));
    }

    for i in 0..src_scene.num_mesh_groups() {
        let idx = MeshGroupIndex::from(i as u32);
        let src_group = src_scene.mesh_group(idx);
        let dst_group = dst_scene.mesh_group(idx);
        assert_eq!(
            src_group.num_mesh_instances(),
            dst_group.num_mesh_instances()
        );
        for j in 0..src_group.num_mesh_instances() {
            let src_inst = src_group.mesh_instance(j);
            let dst_inst = dst_group.mesh_instance(j);
            assert_eq!(src_inst.mesh_index, dst_inst.mesh_index);
            assert_eq!(src_inst.material_index, dst_inst.material_index);
            assert_eq!(
                src_inst.materials_variants_mappings.len(),
                dst_inst.materials_variants_mappings.len()
            );
        }
    }

    for i in 0..src_scene.num_nodes() {
        let idx = SceneNodeIndex::from(i as u32);
        let src_node = src_scene.node(idx);
        let dst_node = dst_scene.node(idx);
        assert_eq!(src_node.num_parents(), dst_node.num_parents());
        for j in 0..src_node.num_parents() {
            assert_eq!(src_node.parent(j), dst_node.parent(j));
        }
        assert_eq!(src_node.num_children(), dst_node.num_children());
        for j in 0..src_node.num_children() {
            assert_eq!(src_node.child(j), dst_node.child(j));
        }
        assert_eq!(src_node.mesh_group_index(), dst_node.mesh_group_index());
        assert_eq!(src_node.skin_index(), dst_node.skin_index());
        assert_eq!(src_node.light_index(), dst_node.light_index());
        assert_eq!(
            src_node.instance_array_index(),
            dst_node.instance_array_index()
        );
    }
}

#[test]
fn scene_remove_mesh() {
    let src_scene = read_scene("CesiumMilkTruck/glTF/CesiumMilkTruck.gltf");

    let mut dst_scene = Scene::new();
    dst_scene.copy_from(&src_scene);
    assert_eq!(dst_scene.num_meshes(), 4);

    let eq = MeshAreEquivalent::new();
    assert!(eq.are_equivalent(
        dst_scene.mesh(MeshIndex::from(0)),
        src_scene.mesh(MeshIndex::from(0))
    ));
    assert!(eq.are_equivalent(
        dst_scene.mesh(MeshIndex::from(1)),
        src_scene.mesh(MeshIndex::from(1))
    ));
    assert!(eq.are_equivalent(
        dst_scene.mesh(MeshIndex::from(2)),
        src_scene.mesh(MeshIndex::from(2))
    ));
    assert!(eq.are_equivalent(
        dst_scene.mesh(MeshIndex::from(3)),
        src_scene.mesh(MeshIndex::from(3))
    ));

    draco_assert_ok!(dst_scene.remove_mesh(MeshIndex::from(2)));
    assert_eq!(dst_scene.num_meshes(), 3);
    assert!(eq.are_equivalent(
        dst_scene.mesh(MeshIndex::from(0)),
        src_scene.mesh(MeshIndex::from(0))
    ));
    assert!(eq.are_equivalent(
        dst_scene.mesh(MeshIndex::from(1)),
        src_scene.mesh(MeshIndex::from(1))
    ));
    assert!(eq.are_equivalent(
        dst_scene.mesh(MeshIndex::from(2)),
        src_scene.mesh(MeshIndex::from(3))
    ));

    draco_assert_ok!(dst_scene.remove_mesh(MeshIndex::from(1)));
    assert_eq!(dst_scene.num_meshes(), 2);
    assert!(eq.are_equivalent(
        dst_scene.mesh(MeshIndex::from(0)),
        src_scene.mesh(MeshIndex::from(0))
    ));
    assert!(eq.are_equivalent(
        dst_scene.mesh(MeshIndex::from(1)),
        src_scene.mesh(MeshIndex::from(3))
    ));
}

#[test]
fn scene_remove_mesh_group() {
    let src_scene = read_scene("CesiumMilkTruck/glTF/CesiumMilkTruck.gltf");

    let mut dst_scene = Scene::new();
    dst_scene.copy_from(&src_scene);
    assert_eq!(dst_scene.num_mesh_groups(), 2);
    assert_eq!(dst_scene.num_nodes(), 5);
    assert_eq!(
        dst_scene.node(SceneNodeIndex::from(0)).mesh_group_index(),
        MeshGroupIndex::from(0)
    );
    assert_eq!(
        dst_scene.node(SceneNodeIndex::from(2)).mesh_group_index(),
        MeshGroupIndex::from(1)
    );
    assert_eq!(
        dst_scene.node(SceneNodeIndex::from(4)).mesh_group_index(),
        MeshGroupIndex::from(1)
    );

    draco_assert_ok!(dst_scene.remove_mesh_group(MeshGroupIndex::from(0)));
    assert_eq!(dst_scene.num_mesh_groups(), 1);
    assert_eq!(dst_scene.num_nodes(), 5);
    assert_eq!(
        dst_scene.node(SceneNodeIndex::from(0)).mesh_group_index(),
        crate::scene::INVALID_MESH_GROUP_INDEX
    );
    assert_eq!(
        dst_scene.node(SceneNodeIndex::from(2)).mesh_group_index(),
        MeshGroupIndex::from(0)
    );
    assert_eq!(
        dst_scene.node(SceneNodeIndex::from(4)).mesh_group_index(),
        MeshGroupIndex::from(0)
    );

    draco_assert_ok!(dst_scene.remove_mesh_group(MeshGroupIndex::from(0)));
    assert_eq!(dst_scene.num_mesh_groups(), 0);
    assert_eq!(
        dst_scene.node(SceneNodeIndex::from(0)).mesh_group_index(),
        crate::scene::INVALID_MESH_GROUP_INDEX
    );
    assert_eq!(
        dst_scene.node(SceneNodeIndex::from(2)).mesh_group_index(),
        crate::scene::INVALID_MESH_GROUP_INDEX
    );
    assert_eq!(
        dst_scene.node(SceneNodeIndex::from(4)).mesh_group_index(),
        crate::scene::INVALID_MESH_GROUP_INDEX
    );
}

#[test]
fn scene_remove_material() {
    let src_scene = read_scene("CesiumMilkTruck/glTF/CesiumMilkTruck.gltf");
    assert_eq!(src_scene.material_library().num_materials(), 4);
    check_mesh_materials(&src_scene, &[0, 1, 2, 3]);

    let mut dst_scene = Scene::new();
    dst_scene.copy_from(&src_scene);

    let status = dst_scene.remove_material(2);
    assert!(!status.is_ok());

    dst_scene.copy_from(&src_scene);

    draco_assert_ok!(dst_scene.remove_mesh(MeshIndex::from(2)));
    assert_eq!(dst_scene.material_library().num_materials(), 4);
    check_mesh_materials(&dst_scene, &[0, 1, 3]);

    draco_assert_ok!(dst_scene.remove_material(2));
    assert_eq!(dst_scene.material_library().num_materials(), 3);
    check_mesh_materials(&dst_scene, &[0, 1, 2]);

    let status = dst_scene.remove_material(-1);
    assert!(!status.is_ok());
    let status = dst_scene.remove_material(3);
    assert!(!status.is_ok());
}

#[test]
fn scene_copy_with_structural_metadata() {
    let mut scene = read_scene("CesiumMilkTruck/glTF/CesiumMilkTruck.gltf");

    let mut schema = StructuralMetadataSchema::new();
    schema.json.set_string("Data");
    scene.structural_metadata_mut().set_schema(schema);

    let mut copy = Scene::new();
    copy.copy_from(&scene);

    assert_eq!(copy.structural_metadata().schema().json.string(), "Data");
}

#[test]
fn scene_copy_with_metadata() {
    let mut scene = read_scene("CesiumMilkTruck/glTF/CesiumMilkTruck.gltf");

    scene
        .metadata_mut()
        .add_entry_string("test_name", "test_value");
    scene.metadata_mut().add_entry_int("test_int", 101);

    let mut copy = Scene::new();
    copy.copy_from(&scene);

    let mut string_val = MetadataString::default();
    let mut int_val = 0;
    assert!(copy
        .metadata()
        .get_entry_string("test_name", &mut string_val));
    assert!(copy.metadata().get_entry_int("test_int", &mut int_val));
    assert_eq!(string_val.as_bytes(), b"test_value");
    assert_eq!(int_val, 101);
}
