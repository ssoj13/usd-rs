//! Rust port of Draco mesh module.
//! Reference: `_ref/draco/src/draco/mesh`.

pub mod corner_table;
pub mod corner_table_iterators;
pub mod mesh;
pub mod mesh_are_equivalent;
pub mod mesh_attribute_corner_table;
pub mod mesh_cleanup;
pub mod mesh_connected_components;
pub mod mesh_features;
pub mod mesh_indices;
pub mod mesh_misc_functions;
pub mod mesh_splitter;
pub mod mesh_stripifier;
pub mod mesh_utils;
pub mod triangle_soup_mesh_builder;
pub mod valence_cache;

pub use corner_table::{CornerTable, FaceType, INVALID_FACE};
pub use mesh::{Face, Mesh, MeshAttributeElementType, MeshHasher};
pub use mesh_are_equivalent::MeshAreEquivalent;
pub use mesh_attribute_corner_table::MeshAttributeCornerTable;
pub use mesh_cleanup::{MeshCleanup, MeshCleanupOptions};
pub use mesh_connected_components::{ConnectedComponent, MeshConnectedComponents};
pub use mesh_features::MeshFeatures;
pub use mesh_indices::{MeshFeaturesIndex, INVALID_MESH_FEATURES_INDEX};
pub use mesh_misc_functions::{
    compute_interpolated_attribute_value_on_mesh_face, create_corner_table_from_all_attributes,
    create_corner_table_from_attribute, create_corner_table_from_position_attribute,
    is_corner_opposite_to_attribute_seam, InterpolatedScalar,
};
pub use mesh_splitter::{MeshSplitter, MeshVector};
pub use mesh_stripifier::MeshStripifier;
pub use mesh_utils::{Matrix3d, Matrix4d, MeshUtils};
pub use triangle_soup_mesh_builder::TriangleSoupMeshBuilder;

#[cfg(test)]
mod tests;
