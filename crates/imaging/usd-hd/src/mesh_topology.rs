//! HdMeshTopology - Mesh topology for Hydra.
//!
//! Corresponds to pxr/imaging/hd/meshTopology.h.
//! Wraps pxOsd::MeshTopology with refinements, geom subsets, invisible components.

use super::geom_subset_struct::HdGeomSubsets;
use super::topology::{HdTopology, HdTopologyId};
use std::hash::{Hash, Hasher};
use usd_px_osd::{MeshTopology, SubdivTags};
use usd_tf::Token;

/// Mesh topology for Hydra meshes.
///
/// Holds raw topology and can compute derivative data.
/// Corresponds to C++ `HdMeshTopology`.
#[derive(Clone, Debug)]
pub struct HdMeshTopology {
    /// Underlying pxOsd topology.
    topology: MeshTopology,

    /// Geometry subsets (face sets, etc.)
    geom_subsets: HdGeomSubsets,

    /// Invisible point indices.
    invisible_points: Vec<i32>,

    /// Invisible face indices.
    invisible_faces: Vec<i32>,

    /// Subdivision refinement level.
    refine_level: i32,

    /// Cached point count.
    num_points: i32,
}

impl HdMeshTopology {
    /// Create from pxOsd topology.
    pub fn new(topology: MeshTopology, refine_level: i32) -> Self {
        let num_points = Self::compute_num_points(topology.face_vertex_indices());
        Self {
            topology,
            geom_subsets: Vec::new(),
            invisible_points: Vec::new(),
            invisible_faces: Vec::new(),
            refine_level,
            num_points,
        }
    }

    /// Create from scheme, orientation, face counts and indices.
    pub fn from_faces(
        scheme: Token,
        orientation: Token,
        face_vertex_counts: Vec<i32>,
        face_vertex_indices: Vec<i32>,
        refine_level: i32,
    ) -> Self {
        let topology =
            MeshTopology::new(scheme, orientation, face_vertex_counts, face_vertex_indices);
        Self::new(topology, refine_level)
    }

    /// Create with holes.
    pub fn from_faces_with_holes(
        scheme: Token,
        orientation: Token,
        face_vertex_counts: Vec<i32>,
        face_vertex_indices: Vec<i32>,
        hole_indices: Vec<i32>,
        refine_level: i32,
    ) -> Self {
        let topology = MeshTopology::with_holes(
            scheme,
            orientation,
            face_vertex_counts,
            face_vertex_indices,
            hole_indices,
        );
        Self::new(topology, refine_level)
    }

    /// Get underlying pxOsd topology.
    pub fn get_px_osd_mesh_topology(&self) -> &MeshTopology {
        &self.topology
    }

    /// Number of faces.
    pub fn get_num_faces(&self) -> i32 {
        self.topology.face_vertex_counts().len() as i32
    }

    /// Number of face-varying elements (sum of face vertex counts).
    pub fn get_num_face_varyings(&self) -> i32 {
        self.topology.face_vertex_indices().len() as i32
    }

    /// Number of points (max index + 1 in vert indices).
    pub fn get_num_points(&self) -> i32 {
        self.num_points
    }

    /// Compute point count from vertex indices.
    pub fn compute_num_points(verts: &[i32]) -> i32 {
        verts.iter().copied().max().map(|m| m + 1).unwrap_or(0)
    }

    /// Subdivision scheme.
    pub fn get_scheme(&self) -> &Token {
        self.topology.scheme()
    }

    /// Refinement level.
    pub fn get_refine_level(&self) -> i32 {
        self.refine_level
    }

    /// Face vertex counts.
    pub fn get_face_vertex_counts(&self) -> &[i32] {
        self.topology.face_vertex_counts()
    }

    /// Face vertex indices.
    pub fn get_face_vertex_indices(&self) -> &[i32] {
        self.topology.face_vertex_indices()
    }

    /// Orientation token.
    pub fn get_orientation(&self) -> &Token {
        self.topology.orientation()
    }

    /// Hole face indices.
    pub fn get_hole_indices(&self) -> &[i32] {
        self.topology.hole_indices()
    }

    /// Set geometry subsets.
    pub fn set_geom_subsets(&mut self, geom_subsets: HdGeomSubsets) {
        self.geom_subsets = geom_subsets;
    }

    /// Get geometry subsets.
    pub fn get_geom_subsets(&self) -> &HdGeomSubsets {
        &self.geom_subsets
    }

    /// Set invisible points.
    pub fn set_invisible_points(&mut self, invisible_points: Vec<i32>) {
        self.invisible_points = invisible_points;
    }

    /// Get invisible points.
    pub fn get_invisible_points(&self) -> &[i32] {
        &self.invisible_points
    }

    /// Set invisible faces.
    pub fn set_invisible_faces(&mut self, invisible_faces: Vec<i32>) {
        self.invisible_faces = invisible_faces;
    }

    /// Get invisible faces.
    pub fn get_invisible_faces(&self) -> &[i32] {
        &self.invisible_faces
    }

    /// Set subdivision tags.
    pub fn set_subdiv_tags(&mut self, tags: SubdivTags) {
        self.topology = self.topology.clone().with_subdiv_tags(tags);
    }

    /// Get subdivision tags.
    pub fn get_subdiv_tags(&self) -> &SubdivTags {
        self.topology.subdiv_tags()
    }

    /// Compute hash for instancing. Corresponds to C++ HdMeshTopology::ComputeHash.
    pub fn compute_hash(&self) -> HdTopologyId {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.topology.face_vertex_counts().hash(&mut hasher);
        self.topology.face_vertex_indices().hash(&mut hasher);
        self.topology.hole_indices().hash(&mut hasher);
        self.refine_level.hash(&mut hasher);
        self.invisible_points.hash(&mut hasher);
        self.invisible_faces.hash(&mut hasher);
        // Geom subsets - use len and a simple identity to avoid requiring HdGeomSubset: Hash
        self.geom_subsets.len().hash(&mut hasher);
        for gs in &self.geom_subsets {
            gs.id.hash(&mut hasher);
            gs.indices.len().hash(&mut hasher);
        }
        hasher.finish()
    }
}

impl HdTopology for HdMeshTopology {
    fn compute_hash(&self) -> HdTopologyId {
        HdMeshTopology::compute_hash(self)
    }
}

impl PartialEq for HdMeshTopology {
    fn eq(&self, other: &Self) -> bool {
        self.topology == other.topology
            && self.geom_subsets == other.geom_subsets
            && self.invisible_points == other.invisible_points
            && self.invisible_faces == other.invisible_faces
            && self.refine_level == other.refine_level
    }
}
