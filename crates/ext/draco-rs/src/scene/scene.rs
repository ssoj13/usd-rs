//! Scene container.
//!
//! What: Holds meshes, mesh groups, nodes, animations, skins, lights, and metadata.
//! Why: Represents glTF scenes and related extensions.
//! How: Owns all scene resources and provides removal/update helpers.
//! Where used: glTF IO and transcoder tools.

use draco_core::core::draco_index_type_vector::IndexTypeVector;
use draco_core::core::status::{ok_status, Status, StatusCode};
use draco_core::material::material_library::MaterialLibrary;
use draco_core::mesh::mesh_indices::MeshFeaturesIndex;
use draco_core::mesh::Mesh;
use draco_core::metadata::metadata::Metadata;
use draco_core::metadata::structural_metadata::StructuralMetadata;
use draco_core::texture::texture_library::TextureLibrary;

use crate::animation::Animation;
use crate::animation::Skin;
use crate::scene::instance_array::InstanceArray;
use crate::scene::light::Light;
use crate::scene::mesh_group::MeshGroup;
use crate::scene::scene_indices::{
    AnimationIndex, InstanceArrayIndex, LightIndex, MeshGroupIndex, MeshIndex, SceneNodeIndex,
    SkinIndex, INVALID_MESH_GROUP_INDEX,
};
use crate::scene::scene_node::SceneNode;

/// Scene container for meshes, nodes, and related assets.
#[derive(Default)]
pub struct Scene {
    meshes: IndexTypeVector<MeshIndex, Box<Mesh>>,
    mesh_groups: IndexTypeVector<MeshGroupIndex, Box<MeshGroup>>,
    nodes: IndexTypeVector<SceneNodeIndex, Box<SceneNode>>,
    root_node_indices: Vec<SceneNodeIndex>,
    animations: IndexTypeVector<AnimationIndex, Box<Animation>>,
    skins: IndexTypeVector<SkinIndex, Box<Skin>>,
    lights: IndexTypeVector<LightIndex, Box<Light>>,
    instance_arrays: IndexTypeVector<InstanceArrayIndex, Box<InstanceArray>>,
    material_library: MaterialLibrary,
    non_material_texture_library: TextureLibrary,
    structural_metadata: StructuralMetadata,
    metadata: Box<Metadata>,
    cesium_rtc: Vec<f64>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            meshes: IndexTypeVector::new(),
            mesh_groups: IndexTypeVector::new(),
            nodes: IndexTypeVector::new(),
            root_node_indices: Vec::new(),
            animations: IndexTypeVector::new(),
            skins: IndexTypeVector::new(),
            lights: IndexTypeVector::new(),
            instance_arrays: IndexTypeVector::new(),
            material_library: MaterialLibrary::new(),
            non_material_texture_library: TextureLibrary::new(),
            structural_metadata: StructuralMetadata::new(),
            metadata: Box::new(Metadata::new()),
            cesium_rtc: Vec::new(),
        }
    }

    pub fn copy_from(&mut self, src: &Scene) {
        self.meshes = IndexTypeVector::new();
        for i in 0..src.meshes.size() {
            let mut mesh = Box::new(Mesh::new());
            mesh.copy_from(&src.meshes[MeshIndex::from(i as u32)]);
            self.meshes.push_back(mesh);
        }

        self.mesh_groups = IndexTypeVector::new();
        for i in 0..src.mesh_groups.size() {
            let mut group = Box::new(MeshGroup::new());
            group.copy_from(&src.mesh_groups[MeshGroupIndex::from(i as u32)]);
            self.mesh_groups.push_back(group);
        }

        self.nodes = IndexTypeVector::new();
        for i in 0..src.nodes.size() {
            let mut node = Box::new(SceneNode::new());
            node.copy_from(&src.nodes[SceneNodeIndex::from(i as u32)]);
            self.nodes.push_back(node);
        }

        self.root_node_indices = src.root_node_indices.clone();

        self.animations = IndexTypeVector::new();
        for i in 0..src.animations.size() {
            let mut anim = Box::new(Animation::new());
            anim.copy_from(&src.animations[AnimationIndex::from(i as u32)]);
            self.animations.push_back(anim);
        }

        self.skins = IndexTypeVector::new();
        for i in 0..src.skins.size() {
            let mut skin = Box::new(Skin::new());
            skin.copy_from(&src.skins[SkinIndex::from(i as u32)]);
            self.skins.push_back(skin);
        }

        self.lights = IndexTypeVector::new();
        for i in 0..src.lights.size() {
            let mut light = Box::new(Light::new());
            light.copy_from(&src.lights[LightIndex::from(i as u32)]);
            self.lights.push_back(light);
        }

        self.instance_arrays = IndexTypeVector::new();
        for i in 0..src.instance_arrays.size() {
            let mut array = Box::new(InstanceArray::new());
            array.copy_from(&src.instance_arrays[InstanceArrayIndex::from(i as u32)]);
            self.instance_arrays.push_back(array);
        }

        self.material_library.copy_from(&src.material_library);

        // Copy non-material textures and update mesh feature texture pointers.
        self.non_material_texture_library
            .copy_from(&src.non_material_texture_library);
        if self.non_material_texture_library.num_textures() != 0 {
            let texture_to_index_map = src
                .non_material_texture_library
                .compute_texture_to_index_map();
            let texture_library_ptr: *mut TextureLibrary = &mut self.non_material_texture_library;
            let num_meshes = self.num_meshes();
            for i in 0..num_meshes {
                let mesh = self.mesh_mut(MeshIndex::from(i as u32));
                for j in 0..mesh.num_mesh_features() {
                    let idx = MeshFeaturesIndex::from(j as u32);
                    let features = mesh.mesh_features_mut(idx);
                    // Safety: texture_library_ptr points to self.non_material_texture_library.
                    let texture_library = unsafe { &mut *texture_library_ptr };
                    Mesh::update_mesh_features_texture_pointer(
                        &texture_to_index_map,
                        texture_library,
                        features,
                    );
                }
            }
        }

        self.structural_metadata.copy_from(&src.structural_metadata);
        self.metadata = Box::new((*src.metadata).clone());
        self.cesium_rtc = src.cesium_rtc.clone();
    }

    pub fn add_mesh(&mut self, mesh: Box<Mesh>) -> MeshIndex {
        self.meshes.push_back(mesh);
        MeshIndex::from((self.meshes.size() - 1) as u32)
    }

    pub fn remove_mesh(&mut self, index: MeshIndex) -> Status {
        let idx = index.value() as usize;
        if idx >= self.meshes.size() {
            return Status::new(StatusCode::DracoError, "Mesh index out of range.");
        }
        let _ = self.meshes.erase(idx);

        for mgi in 0..self.num_mesh_groups() {
            let mesh_group = self.mesh_group_mut(MeshGroupIndex::from(mgi as u32));
            mesh_group.remove_mesh_instances(index);
            for i in 0..mesh_group.num_mesh_instances() {
                let instance = mesh_group.mesh_instance_mut(i);
                if instance.mesh_index.value() > index.value()
                    && instance.mesh_index.value() != u32::MAX
                {
                    instance.mesh_index = MeshIndex::from(instance.mesh_index.value() - 1);
                }
            }
        }
        ok_status()
    }

    pub fn num_meshes(&self) -> i32 {
        self.meshes.size() as i32
    }

    pub fn mesh(&self, index: MeshIndex) -> &Mesh {
        &self.meshes[index]
    }

    pub fn mesh_mut(&mut self, index: MeshIndex) -> &mut Mesh {
        &mut self.meshes[index]
    }

    pub fn add_mesh_group(&mut self) -> MeshGroupIndex {
        self.mesh_groups.push_back(Box::new(MeshGroup::new()));
        MeshGroupIndex::from((self.mesh_groups.size() - 1) as u32)
    }

    pub fn remove_mesh_group(&mut self, index: MeshGroupIndex) -> Status {
        let idx = index.value() as usize;
        if idx >= self.mesh_groups.size() {
            return Status::new(StatusCode::DracoError, "Mesh group index out of range.");
        }
        let _ = self.mesh_groups.erase(idx);

        for sni in 0..self.num_nodes() {
            let node = self.node_mut(SceneNodeIndex::from(sni as u32));
            let mgi = node.mesh_group_index();
            if mgi == index {
                node.set_mesh_group_index(INVALID_MESH_GROUP_INDEX);
            } else if mgi.value() > index.value() && mgi != INVALID_MESH_GROUP_INDEX {
                node.set_mesh_group_index(MeshGroupIndex::from(mgi.value() - 1));
            }
        }
        ok_status()
    }

    pub fn remove_material(&mut self, index: i32) -> Status {
        if index < 0 || index >= self.material_library.num_materials() as i32 {
            return Status::new(StatusCode::DracoError, "Material index is out of range.");
        }

        for mgi in 0..self.num_mesh_groups() {
            let mesh_group = self.mesh_group(MeshGroupIndex::from(mgi as u32));
            for i in 0..mesh_group.num_mesh_instances() {
                let instance = mesh_group.mesh_instance(i);
                if instance.material_index == index {
                    return Status::new(StatusCode::DracoError, "Removed material has references.");
                }
            }
        }

        let _ = self.material_library.remove_material(index);

        for mgi in 0..self.num_mesh_groups() {
            let mesh_group = self.mesh_group_mut(MeshGroupIndex::from(mgi as u32));
            for i in 0..mesh_group.num_mesh_instances() {
                let instance = mesh_group.mesh_instance_mut(i);
                if instance.material_index > index {
                    instance.material_index -= 1;
                }
            }
        }
        ok_status()
    }

    pub fn num_mesh_groups(&self) -> i32 {
        self.mesh_groups.size() as i32
    }

    pub fn mesh_group(&self, index: MeshGroupIndex) -> &MeshGroup {
        &self.mesh_groups[index]
    }

    pub fn mesh_group_mut(&mut self, index: MeshGroupIndex) -> &mut MeshGroup {
        &mut self.mesh_groups[index]
    }

    pub fn add_node(&mut self) -> SceneNodeIndex {
        self.nodes.push_back(Box::new(SceneNode::new()));
        SceneNodeIndex::from((self.nodes.size() - 1) as u32)
    }

    pub fn num_nodes(&self) -> i32 {
        self.nodes.size() as i32
    }

    pub fn node(&self, index: SceneNodeIndex) -> &SceneNode {
        &self.nodes[index]
    }

    pub fn node_mut(&mut self, index: SceneNodeIndex) -> &mut SceneNode {
        &mut self.nodes[index]
    }

    pub fn resize_nodes(&mut self, num_nodes: i32) {
        let mut current = self.nodes.size() as i32;
        while current < num_nodes {
            self.nodes.push_back(Box::new(SceneNode::new()));
            current += 1;
        }
        while current > num_nodes {
            let _ = self.nodes.erase((current - 1) as usize);
            current -= 1;
        }
    }

    pub fn num_root_nodes(&self) -> i32 {
        self.root_node_indices.len() as i32
    }

    pub fn root_node_index(&self, index: i32) -> SceneNodeIndex {
        self.root_node_indices[index as usize]
    }

    pub fn root_node_indices(&self) -> &Vec<SceneNodeIndex> {
        &self.root_node_indices
    }

    pub fn add_root_node_index(&mut self, index: SceneNodeIndex) {
        self.root_node_indices.push(index);
    }

    pub fn set_root_node_index(&mut self, index: i32, node_index: SceneNodeIndex) {
        self.root_node_indices[index as usize] = node_index;
    }

    pub fn remove_all_root_node_indices(&mut self) {
        self.root_node_indices.clear();
    }

    pub fn material_library(&self) -> &MaterialLibrary {
        &self.material_library
    }

    pub fn material_library_mut(&mut self) -> &mut MaterialLibrary {
        &mut self.material_library
    }

    pub fn non_material_texture_library(&self) -> &TextureLibrary {
        &self.non_material_texture_library
    }

    pub fn non_material_texture_library_mut(&mut self) -> &mut TextureLibrary {
        &mut self.non_material_texture_library
    }

    /// Returns both texture libraries for move operations (e.g. move_non_material_textures).
    /// SAFETY: material_library and non_material_texture_library are disjoint fields.
    #[inline]
    pub fn texture_libraries_for_move_mut(&mut self) -> (&mut TextureLibrary, &mut TextureLibrary) {
        let base = self as *mut Scene;
        unsafe {
            (
                (*base).material_library.texture_library_mut(),
                &mut (*base).non_material_texture_library,
            )
        }
    }

    pub fn structural_metadata(&self) -> &StructuralMetadata {
        &self.structural_metadata
    }

    pub fn structural_metadata_mut(&mut self) -> &mut StructuralMetadata {
        &mut self.structural_metadata
    }

    pub fn add_animation(&mut self) -> AnimationIndex {
        self.animations.push_back(Box::new(Animation::new()));
        AnimationIndex::from((self.animations.size() - 1) as u32)
    }

    pub fn num_animations(&self) -> i32 {
        self.animations.size() as i32
    }

    pub fn animation(&self, index: AnimationIndex) -> &Animation {
        &self.animations[index]
    }

    pub fn animation_mut(&mut self, index: AnimationIndex) -> &mut Animation {
        &mut self.animations[index]
    }

    pub fn add_skin(&mut self) -> SkinIndex {
        self.skins.push_back(Box::new(Skin::new()));
        SkinIndex::from((self.skins.size() - 1) as u32)
    }

    pub fn num_skins(&self) -> i32 {
        self.skins.size() as i32
    }

    pub fn skin(&self, index: SkinIndex) -> &Skin {
        &self.skins[index]
    }

    pub fn skin_mut(&mut self, index: SkinIndex) -> &mut Skin {
        &mut self.skins[index]
    }

    pub fn add_light(&mut self) -> LightIndex {
        self.lights.push_back(Box::new(Light::new()));
        LightIndex::from((self.lights.size() - 1) as u32)
    }

    pub fn num_lights(&self) -> i32 {
        self.lights.size() as i32
    }

    pub fn light(&self, index: LightIndex) -> &Light {
        &self.lights[index]
    }

    pub fn light_mut(&mut self, index: LightIndex) -> &mut Light {
        &mut self.lights[index]
    }

    pub fn add_instance_array(&mut self) -> InstanceArrayIndex {
        self.instance_arrays
            .push_back(Box::new(InstanceArray::new()));
        InstanceArrayIndex::from((self.instance_arrays.size() - 1) as u32)
    }

    pub fn num_instance_arrays(&self) -> i32 {
        self.instance_arrays.size() as i32
    }

    pub fn instance_array(&self, index: InstanceArrayIndex) -> &InstanceArray {
        &self.instance_arrays[index]
    }

    pub fn instance_array_mut(&mut self, index: InstanceArrayIndex) -> &mut InstanceArray {
        &mut self.instance_arrays[index]
    }

    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    pub fn metadata_mut(&mut self) -> &mut Metadata {
        &mut self.metadata
    }

    pub fn cesium_rtc(&self) -> &Vec<f64> {
        &self.cesium_rtc
    }

    pub fn set_cesium_rtc(&mut self, values: Vec<f64>) {
        self.cesium_rtc = values;
    }
}
