//! Scene index types.
//!
//! What: Strongly-typed indices for scene elements.
//! Why: Mirrors Draco index types for meshes, nodes, animations, skins, lights.
//! How: Lightweight wrappers around u32 with DracoIndex support.
//! Where used: Scene graph structures and glTF IO.

use draco_core::core::draco_index_type::DracoIndex;

macro_rules! define_scene_index {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name {
            value: u32,
        }

        impl $name {
            pub const fn new(value: u32) -> Self {
                Self { value }
            }

            pub fn value(self) -> u32 {
                self.value
            }
        }

        impl From<u32> for $name {
            fn from(value: u32) -> Self {
                Self::new(value)
            }
        }

        impl DracoIndex for $name {
            fn to_usize(self) -> usize {
                self.value as usize
            }
        }
    };
}

// Index of a mesh in a scene.
define_scene_index!(MeshIndex);
// Index of a mesh instance in a scene.
define_scene_index!(MeshInstanceIndex);
// Index of a mesh group in a scene.
define_scene_index!(MeshGroupIndex);
// Index of a node in a scene.
define_scene_index!(SceneNodeIndex);
// Index of an animation in a scene.
define_scene_index!(AnimationIndex);
// Index of a skin in a scene.
define_scene_index!(SkinIndex);
// Index of a light in a scene.
define_scene_index!(LightIndex);
// Index of a mesh group GPU instancing in a scene.
define_scene_index!(InstanceArrayIndex);

pub const INVALID_MESH_INDEX: MeshIndex = MeshIndex::new(u32::MAX);
pub const INVALID_MESH_INSTANCE_INDEX: MeshInstanceIndex = MeshInstanceIndex::new(u32::MAX);
pub const INVALID_MESH_GROUP_INDEX: MeshGroupIndex = MeshGroupIndex::new(u32::MAX);
pub const INVALID_SCENE_NODE_INDEX: SceneNodeIndex = SceneNodeIndex::new(u32::MAX);
pub const INVALID_ANIMATION_INDEX: AnimationIndex = AnimationIndex::new(u32::MAX);
pub const INVALID_SKIN_INDEX: SkinIndex = SkinIndex::new(u32::MAX);
pub const INVALID_LIGHT_INDEX: LightIndex = LightIndex::new(u32::MAX);
pub const INVALID_INSTANCE_ARRAY_INDEX: InstanceArrayIndex = InstanceArrayIndex::new(u32::MAX);
