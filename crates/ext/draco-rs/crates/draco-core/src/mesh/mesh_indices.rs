//! Mesh index types (transcoder-related).
//! Reference: `_ref/draco/src/draco/mesh/mesh_indices.h`.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshFeaturesIndex {
    value: u32,
}

impl MeshFeaturesIndex {
    pub const fn new(value: u32) -> Self {
        Self { value }
    }

    pub fn value(self) -> u32 {
        self.value
    }

    pub fn value_usize(self) -> usize {
        self.value as usize
    }
}

impl From<u32> for MeshFeaturesIndex {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

pub const INVALID_MESH_FEATURES_INDEX: MeshFeaturesIndex = MeshFeaturesIndex::new(u32::MAX);
