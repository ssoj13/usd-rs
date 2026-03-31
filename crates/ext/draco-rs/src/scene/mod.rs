//! Scene module.
//!
//! What: Scene graph types, nodes, mesh groups, and utilities.
//! Why: glTF IO and transcoder tooling require full scene representation.
//! How: Mirrors Draco C++ scene headers and behavior.
//! Where used: glTF decoding/encoding and scene tools.

mod instance_array;
mod light;
mod mesh_group;
mod scene;
mod scene_are_equivalent;
mod scene_indices;
mod scene_node;
mod scene_utils;
mod trs_matrix;

pub use instance_array::{Instance, InstanceArray};
pub use light::{Light, LightType};
pub use mesh_group::{MaterialsVariantsMapping, MeshGroup, MeshInstance};
pub use scene::Scene;
pub use scene_are_equivalent::SceneAreEquivalent;
pub use scene_indices::*;
pub use scene_node::SceneNode;
pub use scene_utils::{CleanupOptions, SceneMeshInstance, SceneUtils};
pub use trs_matrix::{Quaterniond, TrsMatrix, Vector3d};
