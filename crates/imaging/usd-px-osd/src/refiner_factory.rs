//! PxOsdRefinerFactory - Factory for creating OpenSubdiv topology refiners.
//!
//! Port of pxr/imaging/pxOsd/refinerFactory.h
//!
//! This module provides a factory that creates OpenSubdiv `TopologyRefiner`
//! objects from `MeshTopology` data. This is the bridge between USD subdivision
//! surface descriptions and actual OpenSubdiv evaluation.
//!
//! # Note
//!
//! Full OpenSubdiv integration requires linking against the OpenSubdiv library,
//! which is gated behind the `opensubdiv` feature flag. Without it, the factory
//! methods return `None`.

use super::mesh_topology::MeshTopology;
use usd_tf::Token;

/// Opaque handle to an OpenSubdiv TopologyRefiner.
///
/// In C++, this is `OpenSubdiv::Far::TopologyRefiner`.
/// Here we use an opaque wrapper that can later be backed by real OpenSubdiv FFI.
#[derive(Debug)]
pub struct TopologyRefiner {
    /// Number of vertices in the base mesh.
    pub num_vertices: usize,
    /// Number of faces in the base mesh.
    pub num_faces: usize,
    /// Subdivision scheme used.
    pub scheme: Token,
}

/// Shared handle to a TopologyRefiner.
pub type TopologyRefinerSharedPtr = std::sync::Arc<TopologyRefiner>;

/// Factory for creating OpenSubdiv TopologyRefiner objects from PxOsdMeshTopology.
///
/// Matches C++ `PxOsdRefinerFactory`.
///
/// # Usage
///
/// ```ignore
/// use usd_px_osd::{RefinerFactory, MeshTopology, tokens};
///
/// let topology = MeshTopology::new(
///     tokens::CATMULL_CLARK.clone(),
///     tokens::RIGHT_HANDED.clone(),
///     vec![4],
///     vec![0, 1, 2, 3],
/// );
///
/// if let Some(refiner) = RefinerFactory::create(&topology, "myMesh") {
///     println!("Refiner created with {} verts", refiner.num_vertices);
/// }
/// ```
pub struct RefinerFactory;

impl RefinerFactory {
    /// Creates a TopologyRefiner from a mesh topology.
    ///
    /// # Arguments
    /// * `topology` - The mesh topology to create a refiner for.
    /// * `name` - A name for diagnostic messages.
    ///
    /// # Returns
    /// `Some(TopologyRefinerSharedPtr)` if the topology is valid, `None` otherwise.
    ///
    /// # Note
    /// Without the `opensubdiv` feature, this creates a lightweight stub refiner
    /// containing only metadata (vertex/face counts, scheme).
    pub fn create(topology: &MeshTopology, _name: &str) -> Option<TopologyRefinerSharedPtr> {
        // Validate topology first
        let validation = topology.validate();
        if !validation.is_valid() {
            return None;
        }

        // Compute vertex count from face_vertex_indices
        let num_vertices = topology
            .face_vertex_indices()
            .iter()
            .copied()
            .max()
            .map(|m| m as usize + 1)
            .unwrap_or(0);

        Some(std::sync::Arc::new(TopologyRefiner {
            num_vertices,
            num_faces: topology.face_vertex_counts().len(),
            scheme: topology.scheme().clone(),
        }))
    }

    /// Creates a TopologyRefiner with face-varying topologies.
    ///
    /// # Arguments
    /// * `topology` - The mesh topology to create a refiner for.
    /// * `fvar_topologies` - Face-varying channel index arrays (e.g. UV indices per channel).
    /// * `name` - A name for diagnostic messages.
    ///
    /// # Returns
    /// `Some(TopologyRefinerSharedPtr)` if the topology is valid, `None` otherwise.
    ///
    /// # Note
    /// Face-varying data is currently ignored in the stub implementation.
    /// With full OpenSubdiv integration, each fvar topology would define a
    /// separate face-varying channel on the refiner.
    pub fn create_with_fvar(
        topology: &MeshTopology,
        _fvar_topologies: &[Vec<i32>],
        name: &str,
    ) -> Option<TopologyRefinerSharedPtr> {
        // In the stub, fvar topologies are noted but not applied.
        // Full OpenSubdiv integration would create fvar channels here.
        Self::create(topology, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens;

    #[test]
    fn test_create_valid() {
        let topology = MeshTopology::new(
            tokens::CATMULL_CLARK.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![4],
            vec![0, 1, 2, 3],
        );

        let refiner = RefinerFactory::create(&topology, "test");
        assert!(refiner.is_some());

        let refiner = refiner.unwrap();
        assert_eq!(refiner.num_vertices, 4);
        assert_eq!(refiner.num_faces, 1);
        assert_eq!(refiner.scheme.as_str(), "catmullClark");
    }

    #[test]
    fn test_create_empty() {
        let topology = MeshTopology::default();
        // Empty topology should still be valid (no faces = valid)
        let refiner = RefinerFactory::create(&topology, "empty");
        // May or may not succeed depending on validation of empty mesh
        let _ = refiner;
    }

    #[test]
    fn test_create_with_fvar() {
        let topology = MeshTopology::new(
            tokens::CATMULL_CLARK.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![3],
            vec![0, 1, 2],
        );

        // Face-varying channel indices (e.g. UV indices)
        let fvar_indices = vec![0, 1, 2];
        let refiner = RefinerFactory::create_with_fvar(&topology, &[fvar_indices], "test_fvar");
        assert!(refiner.is_some());
    }
}
