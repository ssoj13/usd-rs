//! Node animation data.
//!
//! What: Stores per-node animation keyframes (scalar/vec/matrix data).
//! Why: glTF animation samplers and skins reference these data blocks.
//! How: Mirrors Draco C++ `NodeAnimationData` with explicit copy and hashing.
//! Where used: animation samplers, skins, and glTF IO helpers.

use std::hash::{Hash, Hasher};

use draco_core::core::hash_utils::{fingerprint_string, hash_combine_u64, hash_combine_with};

/// Animation data type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeAnimationDataType {
    Scalar,
    Vec3,
    Vec4,
    Mat4,
}

impl Default for NodeAnimationDataType {
    fn default() -> Self {
        NodeAnimationDataType::Scalar
    }
}

impl NodeAnimationDataType {
    pub fn as_string(self) -> &'static str {
        match self {
            NodeAnimationDataType::Scalar => "SCALAR",
            NodeAnimationDataType::Vec3 => "VEC3",
            NodeAnimationDataType::Mat4 => "MAT4",
            NodeAnimationDataType::Vec4 => "VEC4",
        }
    }
}

/// Stores node animation values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeAnimationData {
    data_type: NodeAnimationDataType,
    count: i32,
    normalized: bool,
    data: Vec<f32>,
}

impl NodeAnimationData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn copy_from(&mut self, src: &NodeAnimationData) {
        self.data_type = src.data_type;
        self.count = src.count;
        self.normalized = src.normalized;
        self.data = src.data.clone();
    }

    pub fn data_type(&self) -> NodeAnimationDataType {
        self.data_type
    }

    pub fn count(&self) -> i32 {
        self.count
    }

    pub fn normalized(&self) -> bool {
        self.normalized
    }

    pub fn data(&self) -> &Vec<f32> {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut Vec<f32> {
        &mut self.data
    }

    pub fn set_type(&mut self, data_type: NodeAnimationDataType) {
        self.data_type = data_type;
    }

    pub fn set_count(&mut self, count: i32) {
        self.count = count;
    }

    pub fn set_normalized(&mut self, normalized: bool) {
        self.normalized = normalized;
    }

    pub fn component_size(&self) -> i32 {
        std::mem::size_of::<f32>() as i32
    }

    pub fn num_components(&self) -> i32 {
        match self.data_type {
            NodeAnimationDataType::Scalar => 1,
            NodeAnimationDataType::Vec3 => 3,
            NodeAnimationDataType::Mat4 => 16,
            NodeAnimationDataType::Vec4 => 4,
        }
    }

    pub fn type_as_string(&self) -> &'static str {
        self.data_type.as_string()
    }
}

/// Hash wrapper for NodeAnimationData.
#[derive(Clone, Copy)]
pub struct NodeAnimationDataHash {
    node_animation_data: *const NodeAnimationData,
    hash: u64,
}

impl NodeAnimationDataHash {
    pub fn new(nad: &NodeAnimationData) -> Self {
        let hash = Self::hash_node_animation_data(nad);
        Self {
            node_animation_data: nad as *const NodeAnimationData,
            hash,
        }
    }

    pub fn node_animation_data(&self) -> &NodeAnimationData {
        unsafe { &*self.node_animation_data }
    }

    fn hash_node_animation_data(nad: &NodeAnimationData) -> u64 {
        let mut hash = 79u64;
        hash = hash_combine_with(&(nad.data_type() as i32), hash);
        hash = hash_combine_with(&nad.count(), hash);
        hash = hash_combine_with(&nad.normalized(), hash);
        let bytes = unsafe {
            std::slice::from_raw_parts(
                nad.data().as_ptr() as *const u8,
                nad.data().len() * std::mem::size_of::<f32>(),
            )
        };
        let data_hash = fingerprint_string(bytes);
        hash_combine_u64(data_hash, hash)
    }
}

impl PartialEq for NodeAnimationDataHash {
    fn eq(&self, other: &Self) -> bool {
        self.node_animation_data() == other.node_animation_data()
    }
}

impl Eq for NodeAnimationDataHash {}

impl Hash for NodeAnimationDataHash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}
