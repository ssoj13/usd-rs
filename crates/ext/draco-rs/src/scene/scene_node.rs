//! Scene node.
//!
//! What: Node in a scene hierarchy with TRS transform and references.
//! Why: glTF nodes reference mesh groups, skins, lights, and instancing.
//! How: Stores parent/child indices with explicit setters.
//! Where used: Scene graphs and glTF IO.

use crate::scene::scene_indices::{
    InstanceArrayIndex, LightIndex, MeshGroupIndex, SceneNodeIndex, SkinIndex,
};
use crate::scene::trs_matrix::TrsMatrix;

/// Scene node describing hierarchy and transforms.
#[derive(Clone, Debug, Default)]
pub struct SceneNode {
    name: String,
    trs_matrix: TrsMatrix,
    mesh_group_index: MeshGroupIndex,
    skin_index: SkinIndex,
    parents: Vec<SceneNodeIndex>,
    children: Vec<SceneNodeIndex>,
    light_index: LightIndex,
    instance_array_index: InstanceArrayIndex,
}

impl SceneNode {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            trs_matrix: TrsMatrix::default(),
            mesh_group_index: MeshGroupIndex::from(u32::MAX),
            skin_index: SkinIndex::from(u32::MAX),
            parents: Vec::new(),
            children: Vec::new(),
            light_index: LightIndex::from(u32::MAX),
            instance_array_index: InstanceArrayIndex::from(u32::MAX),
        }
    }

    pub fn copy_from(&mut self, src: &SceneNode) {
        self.name = src.name.clone();
        self.trs_matrix.copy_from(&src.trs_matrix);
        self.mesh_group_index = src.mesh_group_index;
        self.skin_index = src.skin_index;
        self.parents = src.parents.clone();
        self.children = src.children.clone();
        self.light_index = src.light_index;
        self.instance_array_index = src.instance_array_index;
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_trs_matrix(&mut self, trs: &TrsMatrix) {
        self.trs_matrix.copy_from(trs);
    }

    pub fn trs_matrix(&self) -> &TrsMatrix {
        &self.trs_matrix
    }

    pub fn set_mesh_group_index(&mut self, index: MeshGroupIndex) {
        self.mesh_group_index = index;
    }

    pub fn mesh_group_index(&self) -> MeshGroupIndex {
        self.mesh_group_index
    }

    pub fn set_skin_index(&mut self, index: SkinIndex) {
        self.skin_index = index;
    }

    pub fn skin_index(&self) -> SkinIndex {
        self.skin_index
    }

    pub fn set_light_index(&mut self, index: LightIndex) {
        self.light_index = index;
    }

    pub fn light_index(&self) -> LightIndex {
        self.light_index
    }

    pub fn set_instance_array_index(&mut self, index: InstanceArrayIndex) {
        self.instance_array_index = index;
    }

    pub fn instance_array_index(&self) -> InstanceArrayIndex {
        self.instance_array_index
    }

    pub fn parent(&self, index: i32) -> SceneNodeIndex {
        self.parents[index as usize]
    }

    pub fn parents(&self) -> &Vec<SceneNodeIndex> {
        &self.parents
    }

    pub fn add_parent_index(&mut self, index: SceneNodeIndex) {
        self.parents.push(index);
    }

    pub fn num_parents(&self) -> i32 {
        self.parents.len() as i32
    }

    pub fn remove_all_parents(&mut self) {
        self.parents.clear();
    }

    pub fn child(&self, index: i32) -> SceneNodeIndex {
        self.children[index as usize]
    }

    pub fn children(&self) -> &Vec<SceneNodeIndex> {
        &self.children
    }

    pub fn add_child_index(&mut self, index: SceneNodeIndex) {
        self.children.push(index);
    }

    pub fn num_children(&self) -> i32 {
        self.children.len() as i32
    }

    pub fn remove_all_children(&mut self) {
        self.children.clear();
    }
}
