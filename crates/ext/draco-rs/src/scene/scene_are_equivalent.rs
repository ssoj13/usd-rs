//! Scene equivalence checker.
//!
//! What: Compares two scenes for equivalence up to mesh vertex permutation.
//! Why: Mirrors Draco `SceneAreEquivalent` used in transcoder tests.
//! How: Checks mesh/scene sizes, mesh equivalence, node transforms, and indices.
//! Where used: Scene validation tests and parity checks.

use draco_core::material::material_utils::MaterialUtils;
use draco_core::mesh::MeshAreEquivalent;
use draco_core::metadata::metadata::Metadata;
use draco_core::texture::texture_library::TextureLibrary;

use crate::animation::{Animation, AnimationChannel, AnimationSampler, NodeAnimationData, Skin};
use crate::scene::scene_indices::{MeshIndex, SceneNodeIndex};
use crate::scene::{
    InstanceArray, Light, MaterialsVariantsMapping, MeshGroup, MeshInstance, Scene, SceneNode,
};

/// Compares two scenes for equivalence up to permutation of mesh vertices.
#[derive(Default)]
pub struct SceneAreEquivalent;

impl SceneAreEquivalent {
    pub fn new() -> Self {
        Self
    }

    /// Returns true if both scenes are equivalent up to mesh vertex permutation.
    pub fn are_equivalent(&self, scene0: &Scene, scene1: &Scene) -> bool {
        if scene0.num_animations() != scene1.num_animations() {
            return false;
        }
        if scene0.num_mesh_groups() != scene1.num_mesh_groups() {
            return false;
        }
        if scene0.num_skins() != scene1.num_skins() {
            return false;
        }
        if scene0.num_lights() != scene1.num_lights() {
            return false;
        }
        if scene0.num_instance_arrays() != scene1.num_instance_arrays() {
            return false;
        }

        if scene0.num_meshes() != scene1.num_meshes() {
            return false;
        }
        let mesh_eq = MeshAreEquivalent::new();
        for i in 0..scene0.num_meshes() {
            let idx = MeshIndex::from(i as u32);
            if !mesh_eq.are_equivalent(scene0.mesh(idx), scene1.mesh(idx)) {
                return false;
            }
        }

        if scene0.num_nodes() != scene1.num_nodes() {
            return false;
        }
        for i in 0..scene0.num_nodes() {
            let idx = SceneNodeIndex::from(i as u32);
            if !Self::nodes_are_equivalent(scene0.node(idx), scene1.node(idx)) {
                return false;
            }
        }

        if scene0.num_root_nodes() != scene1.num_root_nodes() {
            return false;
        }
        for i in 0..scene0.num_root_nodes() {
            if scene0.root_node_index(i) != scene1.root_node_index(i) {
                return false;
            }
        }

        for i in 0..scene0.num_mesh_groups() {
            let idx = crate::scene::scene_indices::MeshGroupIndex::from(i as u32);
            if !Self::mesh_groups_are_equivalent(scene0.mesh_group(idx), scene1.mesh_group(idx)) {
                return false;
            }
        }

        for i in 0..scene0.num_animations() {
            let idx = crate::scene::scene_indices::AnimationIndex::from(i as u32);
            if !Self::animations_are_equivalent(scene0.animation(idx), scene1.animation(idx)) {
                return false;
            }
        }

        for i in 0..scene0.num_skins() {
            let idx = crate::scene::scene_indices::SkinIndex::from(i as u32);
            if !Self::skins_are_equivalent(scene0.skin(idx), scene1.skin(idx)) {
                return false;
            }
        }

        for i in 0..scene0.num_lights() {
            let idx = crate::scene::scene_indices::LightIndex::from(i as u32);
            if !Self::lights_are_equivalent(scene0.light(idx), scene1.light(idx)) {
                return false;
            }
        }

        for i in 0..scene0.num_instance_arrays() {
            let idx = crate::scene::scene_indices::InstanceArrayIndex::from(i as u32);
            if !Self::instance_arrays_are_equivalent(
                scene0.instance_array(idx),
                scene1.instance_array(idx),
            ) {
                return false;
            }
        }

        if !Self::material_libraries_are_equivalent(
            scene0.material_library(),
            scene1.material_library(),
        ) {
            return false;
        }

        if !Self::texture_libraries_are_equivalent(
            scene0.non_material_texture_library(),
            scene1.non_material_texture_library(),
        ) {
            return false;
        }

        if scene0.structural_metadata() != scene1.structural_metadata() {
            return false;
        }

        if !Self::metadata_are_equivalent(scene0.metadata(), scene1.metadata()) {
            return false;
        }

        if scene0.cesium_rtc() != scene1.cesium_rtc() {
            return false;
        }

        true
    }

    fn nodes_are_equivalent(node0: &SceneNode, node1: &SceneNode) -> bool {
        if node0.name() != node1.name() {
            return false;
        }
        if node0.mesh_group_index() != node1.mesh_group_index() {
            return false;
        }
        if node0.skin_index() != node1.skin_index() {
            return false;
        }
        if node0.light_index() != node1.light_index() {
            return false;
        }
        if node0.instance_array_index() != node1.instance_array_index() {
            return false;
        }

        if node0.trs_matrix().compute_transformation_matrix()
            != node1.trs_matrix().compute_transformation_matrix()
        {
            return false;
        }

        if node0.num_children() != node1.num_children() {
            return false;
        }
        for i in 0..node0.num_children() {
            if node0.child(i) != node1.child(i) {
                return false;
            }
        }

        if node0.num_parents() != node1.num_parents() {
            return false;
        }
        for i in 0..node0.num_parents() {
            if node0.parent(i) != node1.parent(i) {
                return false;
            }
        }
        true
    }

    fn mesh_groups_are_equivalent(group0: &MeshGroup, group1: &MeshGroup) -> bool {
        if group0.name() != group1.name() {
            return false;
        }
        if group0.num_mesh_instances() != group1.num_mesh_instances() {
            return false;
        }
        for i in 0..group0.num_mesh_instances() {
            if !Self::mesh_instances_are_equivalent(
                group0.mesh_instance(i),
                group1.mesh_instance(i),
            ) {
                return false;
            }
        }
        true
    }

    fn mesh_instances_are_equivalent(instance0: &MeshInstance, instance1: &MeshInstance) -> bool {
        if instance0.mesh_index != instance1.mesh_index {
            return false;
        }
        if instance0.material_index != instance1.material_index {
            return false;
        }
        if instance0.materials_variants_mappings.len()
            != instance1.materials_variants_mappings.len()
        {
            return false;
        }
        for (lhs, rhs) in instance0
            .materials_variants_mappings
            .iter()
            .zip(instance1.materials_variants_mappings.iter())
        {
            if !Self::variants_mapping_are_equivalent(lhs, rhs) {
                return false;
            }
        }
        true
    }

    fn variants_mapping_are_equivalent(
        mapping0: &MaterialsVariantsMapping,
        mapping1: &MaterialsVariantsMapping,
    ) -> bool {
        mapping0 == mapping1
    }

    fn animations_are_equivalent(anim0: &Animation, anim1: &Animation) -> bool {
        if anim0.name() != anim1.name() {
            return false;
        }
        if anim0.num_channels() != anim1.num_channels() {
            return false;
        }
        if anim0.num_samplers() != anim1.num_samplers() {
            return false;
        }
        if anim0.num_node_animation_data() != anim1.num_node_animation_data() {
            return false;
        }

        for i in 0..anim0.num_channels() {
            let channel0 = match anim0.channel(i) {
                Some(channel) => channel,
                None => return false,
            };
            let channel1 = match anim1.channel(i) {
                Some(channel) => channel,
                None => return false,
            };
            if !Self::channels_are_equivalent(channel0, channel1) {
                return false;
            }
        }

        for i in 0..anim0.num_samplers() {
            let sampler0 = match anim0.sampler(i) {
                Some(sampler) => sampler,
                None => return false,
            };
            let sampler1 = match anim1.sampler(i) {
                Some(sampler) => sampler,
                None => return false,
            };
            if !Self::samplers_are_equivalent(sampler0, sampler1) {
                return false;
            }
        }

        for i in 0..anim0.num_node_animation_data() {
            let data0 = match anim0.node_animation_data(i) {
                Some(data) => data,
                None => return false,
            };
            let data1 = match anim1.node_animation_data(i) {
                Some(data) => data,
                None => return false,
            };
            if !Self::node_animation_data_are_equivalent(data0, data1) {
                return false;
            }
        }

        true
    }

    fn channels_are_equivalent(channel0: &AnimationChannel, channel1: &AnimationChannel) -> bool {
        channel0.target_index == channel1.target_index
            && channel0.transformation_type == channel1.transformation_type
            && channel0.sampler_index == channel1.sampler_index
    }

    fn samplers_are_equivalent(sampler0: &AnimationSampler, sampler1: &AnimationSampler) -> bool {
        sampler0.input_index == sampler1.input_index
            && sampler0.interpolation_type == sampler1.interpolation_type
            && sampler0.output_index == sampler1.output_index
    }

    fn node_animation_data_are_equivalent(
        data0: &NodeAnimationData,
        data1: &NodeAnimationData,
    ) -> bool {
        data0 == data1
    }

    fn skins_are_equivalent(skin0: &Skin, skin1: &Skin) -> bool {
        if !Self::node_animation_data_are_equivalent(
            skin0.inverse_bind_matrices(),
            skin1.inverse_bind_matrices(),
        ) {
            return false;
        }
        if skin0.joint_root() != skin1.joint_root() {
            return false;
        }
        if skin0.num_joints() != skin1.num_joints() {
            return false;
        }
        for i in 0..skin0.num_joints() {
            if skin0.joint(i) != skin1.joint(i) {
                return false;
            }
        }
        true
    }

    fn lights_are_equivalent(light0: &Light, light1: &Light) -> bool {
        light0.name() == light1.name()
            && light0.color() == light1.color()
            && light0.intensity() == light1.intensity()
            && light0.light_type() == light1.light_type()
            && light0.range() == light1.range()
            && light0.inner_cone_angle() == light1.inner_cone_angle()
            && light0.outer_cone_angle() == light1.outer_cone_angle()
    }

    fn instance_arrays_are_equivalent(array0: &InstanceArray, array1: &InstanceArray) -> bool {
        if array0.num_instances() != array1.num_instances() {
            return false;
        }
        for i in 0..array0.num_instances() {
            let inst0 = array0.instance(i);
            let inst1 = array1.instance(i);
            if inst0.trs.compute_transformation_matrix()
                != inst1.trs.compute_transformation_matrix()
            {
                return false;
            }
        }
        true
    }

    fn material_libraries_are_equivalent(
        lib0: &draco_core::material::material_library::MaterialLibrary,
        lib1: &draco_core::material::material_library::MaterialLibrary,
    ) -> bool {
        if lib0.num_materials() != lib1.num_materials() {
            return false;
        }
        for i in 0..lib0.num_materials() {
            let material0 = match lib0.material(i as i32) {
                Some(material) => material,
                None => return false,
            };
            let material1 = match lib1.material(i as i32) {
                Some(material) => material,
                None => return false,
            };
            if !MaterialUtils::are_materials_equivalent(material0, material1) {
                return false;
            }
        }

        if lib0.num_materials_variants() != lib1.num_materials_variants() {
            return false;
        }
        for i in 0..lib0.num_materials_variants() {
            if lib0.materials_variant_name(i) != lib1.materials_variant_name(i) {
                return false;
            }
        }

        if !Self::texture_libraries_are_equivalent(lib0.texture_library(), lib1.texture_library()) {
            return false;
        }

        true
    }

    fn texture_libraries_are_equivalent(lib0: &TextureLibrary, lib1: &TextureLibrary) -> bool {
        if lib0.num_textures() != lib1.num_textures() {
            return false;
        }
        for i in 0..lib0.num_textures() {
            let tex0 = match lib0.texture(i as i32) {
                Some(tex) => tex,
                None => return false,
            };
            let tex1 = match lib1.texture(i as i32) {
                Some(tex) => tex,
                None => return false,
            };
            if tex0 != tex1 {
                return false;
            }
        }
        true
    }

    fn metadata_are_equivalent(meta0: &Metadata, meta1: &Metadata) -> bool {
        if meta0.num_entries() != meta1.num_entries() {
            return false;
        }
        if meta0.entries().len() != meta1.entries().len() {
            return false;
        }
        for (key, entry) in meta0.entries() {
            let other_entry = match meta1.entries().get(key) {
                Some(entry) => entry,
                None => return false,
            };
            if entry.data() != other_entry.data() {
                return false;
            }
        }

        if meta0.sub_metadatas().len() != meta1.sub_metadatas().len() {
            return false;
        }
        for (key, sub) in meta0.sub_metadatas() {
            let other_sub = match meta1.sub_metadatas().get(key) {
                Some(entry) => entry,
                None => return false,
            };
            if !Self::metadata_are_equivalent(sub.as_ref(), other_sub) {
                return false;
            }
        }
        true
    }
}
