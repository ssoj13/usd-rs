//! Skin definition.
//!
//! What: Stores glTF skin joints and inverse bind matrices.
//! Why: Required to support skeletal animation in scenes.
//! How: Mirrors Draco `Skin` with explicit copy helper.
//! Where used: scene graphs and glTF IO.

use crate::animation::node_animation_data::NodeAnimationData;
use crate::scene::SceneNodeIndex;

/// Skin data for skeletal animation.
#[derive(Clone, Debug, Default)]
pub struct Skin {
    inverse_bind_matrices: NodeAnimationData,
    joints: Vec<SceneNodeIndex>,
    joint_root_index: SceneNodeIndex,
}

impl Skin {
    pub fn new() -> Self {
        Self {
            inverse_bind_matrices: NodeAnimationData::default(),
            joints: Vec::new(),
            joint_root_index: SceneNodeIndex::from(u32::MAX),
        }
    }

    pub fn copy_from(&mut self, src: &Skin) {
        self.inverse_bind_matrices
            .copy_from(&src.inverse_bind_matrices);
        self.joints = src.joints.clone();
        self.joint_root_index = src.joint_root_index;
    }

    pub fn inverse_bind_matrices(&self) -> &NodeAnimationData {
        &self.inverse_bind_matrices
    }

    pub fn inverse_bind_matrices_mut(&mut self) -> &mut NodeAnimationData {
        &mut self.inverse_bind_matrices
    }

    pub fn add_joint(&mut self, index: SceneNodeIndex) -> i32 {
        self.joints.push(index);
        (self.joints.len() - 1) as i32
    }

    pub fn num_joints(&self) -> i32 {
        self.joints.len() as i32
    }

    pub fn joint(&self, index: i32) -> SceneNodeIndex {
        self.joints[index as usize]
    }

    pub fn joint_mut(&mut self, index: i32) -> &mut SceneNodeIndex {
        &mut self.joints[index as usize]
    }

    pub fn joints(&self) -> &Vec<SceneNodeIndex> {
        &self.joints
    }

    pub fn set_joint_root(&mut self, index: SceneNodeIndex) {
        self.joint_root_index = index;
    }

    pub fn joint_root(&self) -> SceneNodeIndex {
        self.joint_root_index
    }
}
