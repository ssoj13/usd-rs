//! Mesh topology for subdivision surfaces.
//!
//! This module provides the `MeshTopology` struct which represents the complete
//! topology of a subdivision surface mesh, including connectivity, subdivision
//! scheme, orientation, holes, and subdivision tags.

use super::{mesh_topology_validation::MeshTopologyValidation, subdiv_tags::SubdivTags, tokens};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use usd_tf::Token;

/// Topology data for subdivision surface meshes.
///
/// This struct is immutable once constructed (except via assignment/move).
/// Use the `with_*` builder methods to create modified copies.
///
/// # Example
/// ```ignore
/// let topology = MeshTopology::new(
///     tokens::CATMULL_CLARK.clone(),
///     tokens::RIGHT_HANDED.clone(),
///     vec![4, 4, 4, 4, 4, 4], // 6 quads
///     vec![0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 4, 5, 9, 8, 5, 6, 10, 9, 6, 7, 11, 10],
/// );
///
/// // Create a variant with different scheme
/// let loop_topology = topology.with_scheme(tokens::LOOP.clone());
/// ```
#[derive(Debug)]
pub struct MeshTopology {
    /// Subdivision scheme (catmullClark, loop, bilinear, none).
    scheme: Token,

    /// Face winding order (rightHanded, leftHanded).
    orientation: Token,

    /// Number of vertices per face.
    /// Each element defines how many vertices form one face.
    face_vertex_counts: Vec<i32>,

    /// Vertex indices for all faces.
    /// Size must equal sum of face_vertex_counts.
    face_vertex_indices: Vec<i32>,

    /// Indices of faces that should be treated as holes.
    hole_indices: Vec<i32>,

    /// Subdivision tags (creases, corners, interpolation rules).
    subdiv_tags: SubdivTags,

    /// Cached validation status.
    /// True if topology has been successfully validated.
    /// Uses AtomicBool for thread-safe access (enables HdMeshTopology: HdTopology).
    validated: AtomicBool,
}

impl Clone for MeshTopology {
    fn clone(&self) -> Self {
        Self {
            scheme: self.scheme.clone(),
            orientation: self.orientation.clone(),
            face_vertex_counts: self.face_vertex_counts.clone(),
            face_vertex_indices: self.face_vertex_indices.clone(),
            hole_indices: self.hole_indices.clone(),
            subdiv_tags: self.subdiv_tags.clone(),
            validated: AtomicBool::new(self.validated.load(Ordering::Relaxed)),
        }
    }
}

impl MeshTopology {
    /// Create a new mesh topology without holes or subdiv tags.
    pub fn new(
        scheme: Token,
        orientation: Token,
        face_vertex_counts: Vec<i32>,
        face_vertex_indices: Vec<i32>,
    ) -> Self {
        Self {
            scheme,
            orientation,
            face_vertex_counts,
            face_vertex_indices,
            hole_indices: Vec::new(),
            subdiv_tags: SubdivTags::default(),
            validated: AtomicBool::new(false),
        }
    }

    /// Create a new mesh topology with holes.
    pub fn with_holes(
        scheme: Token,
        orientation: Token,
        face_vertex_counts: Vec<i32>,
        face_vertex_indices: Vec<i32>,
        hole_indices: Vec<i32>,
    ) -> Self {
        Self {
            scheme,
            orientation,
            face_vertex_counts,
            face_vertex_indices,
            hole_indices,
            subdiv_tags: SubdivTags::default(),
            validated: AtomicBool::new(false),
        }
    }

    /// Create a new mesh topology with holes and subdiv tags.
    pub fn with_holes_and_tags(
        scheme: Token,
        orientation: Token,
        face_vertex_counts: Vec<i32>,
        face_vertex_indices: Vec<i32>,
        hole_indices: Vec<i32>,
        subdiv_tags: SubdivTags,
    ) -> Self {
        Self {
            scheme,
            orientation,
            face_vertex_counts,
            face_vertex_indices,
            hole_indices,
            subdiv_tags,
            validated: AtomicBool::new(false),
        }
    }

    /// Create a new mesh topology with subdiv tags but no holes.
    pub fn with_tags(
        scheme: Token,
        orientation: Token,
        face_vertex_counts: Vec<i32>,
        face_vertex_indices: Vec<i32>,
        subdiv_tags: SubdivTags,
    ) -> Self {
        Self {
            scheme,
            orientation,
            face_vertex_counts,
            face_vertex_indices,
            hole_indices: Vec::new(),
            subdiv_tags,
            validated: AtomicBool::new(false),
        }
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Get the subdivision scheme.
    pub fn scheme(&self) -> &Token {
        &self.scheme
    }

    /// Get face vertex counts.
    pub fn face_vertex_counts(&self) -> &[i32] {
        &self.face_vertex_counts
    }

    /// Get face vertex indices.
    pub fn face_vertex_indices(&self) -> &[i32] {
        &self.face_vertex_indices
    }

    /// Get orientation.
    pub fn orientation(&self) -> &Token {
        &self.orientation
    }

    /// Get hole face indices.
    pub fn hole_indices(&self) -> &[i32] {
        &self.hole_indices
    }

    /// Get subdivision tags.
    pub fn subdiv_tags(&self) -> &SubdivTags {
        &self.subdiv_tags
    }

    // ========================================================================
    // Builder methods (return modified copies)
    // ========================================================================

    /// Return a copy with a different subdivision scheme.
    ///
    /// Valid schemes: catmullClark, loop, bilinear, none.
    pub fn with_scheme(&self, scheme: Token) -> Self {
        let mut copy = self.clone();
        copy.scheme = scheme;
        copy.validated = AtomicBool::new(false);
        copy
    }

    /// Return a copy with a different orientation.
    ///
    /// Valid orientations: rightHanded, leftHanded.
    pub fn with_orientation(&self, orientation: Token) -> Self {
        let mut copy = self.clone();
        copy.orientation = orientation;
        copy.validated = AtomicBool::new(false);
        copy
    }

    /// Return a copy with different subdivision tags.
    pub fn with_subdiv_tags(&self, subdiv_tags: SubdivTags) -> Self {
        let mut copy = self.clone();
        copy.subdiv_tags = subdiv_tags;
        copy.validated = AtomicBool::new(false);
        copy
    }

    /// Return a copy with different hole indices.
    pub fn with_hole_indices(&self, hole_indices: Vec<i32>) -> Self {
        let mut copy = self.clone();
        copy.hole_indices = hole_indices;
        copy.validated = AtomicBool::new(false);
        copy
    }

    // ========================================================================
    // Hash and validation
    // ========================================================================

    /// Compute a hash value for instancing.
    pub fn compute_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        let tags_hash = self.subdiv_tags.compute_hash();
        tags_hash.hash(&mut hasher);

        self.scheme.as_str().hash(&mut hasher);
        self.orientation.as_str().hash(&mut hasher);

        self.face_vertex_counts.hash(&mut hasher);
        self.face_vertex_indices.hash(&mut hasher);
        self.hole_indices.hash(&mut hasher);

        hasher.finish()
    }

    /// Validate the topology.
    ///
    /// Returns a validation object which is empty if the topology is valid.
    /// The validation result is cached if the topology is valid.
    ///
    /// # Example
    /// ```ignore
    /// let validation = topology.validate();
    /// if !validation.is_valid() {
    ///     for error in validation.errors() {
    ///         eprintln!("Error: {}", error);
    ///     }
    /// }
    /// ```
    pub fn validate(&self) -> MeshTopologyValidation {
        // Return cached success if already validated
        if self.validated.load(Ordering::Acquire) {
            return MeshTopologyValidation::new();
        }

        let mut validation = MeshTopologyValidation::new();

        // Validate tokens
        validation.validate_scheme(&self.scheme);
        validation.validate_orientation(&self.orientation);
        validation.validate_triangle_subdivision(self.subdiv_tags.triangle_subdivision());
        validation.validate_vertex_interpolation(self.subdiv_tags.vertex_interpolation_rule());
        validation.validate_face_varying_interpolation(
            self.subdiv_tags.face_varying_interpolation_rule(),
        );
        validation.validate_crease_method(self.subdiv_tags.crease_method());

        // Validate creases and corners
        validation.validate_creases_and_corners(
            self.subdiv_tags.crease_indices(),
            self.subdiv_tags.crease_lengths(),
            self.subdiv_tags.crease_weights(),
            self.subdiv_tags.corner_indices(),
            self.subdiv_tags.corner_weights(),
            &self.face_vertex_indices,
        );

        // Validate holes
        validation.validate_holes(&self.hole_indices, self.face_vertex_counts.len());

        // Validate face data
        validation.validate_face_vertex_counts(&self.face_vertex_counts);
        validation
            .validate_face_vertex_indices(&self.face_vertex_indices, &self.face_vertex_counts);

        // Cache success
        if validation.is_valid() {
            self.validated.store(true, Ordering::Release);
        }

        validation
    }
}

impl Default for MeshTopology {
    /// Create an empty topology with bilinear scheme and right-handed orientation.
    fn default() -> Self {
        Self {
            scheme: tokens::BILINEAR.clone(),
            orientation: tokens::RIGHT_HANDED.clone(),
            face_vertex_counts: Vec::new(),
            face_vertex_indices: Vec::new(),
            hole_indices: Vec::new(),
            subdiv_tags: SubdivTags::default(),
            validated: AtomicBool::new(false),
        }
    }
}

impl PartialEq for MeshTopology {
    fn eq(&self, other: &Self) -> bool {
        self.scheme == other.scheme
            && self.orientation == other.orientation
            && self.face_vertex_counts == other.face_vertex_counts
            && self.face_vertex_indices == other.face_vertex_indices
            && self.subdiv_tags == other.subdiv_tags
            && self.hole_indices == other.hole_indices
    }
}

impl Eq for MeshTopology {}

impl std::fmt::Display for MeshTopology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "({}, {}, {:?}, {:?}, {:?})",
            self.orientation.as_str(),
            self.scheme.as_str(),
            self.face_vertex_counts,
            self.face_vertex_indices,
            self.hole_indices
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let topology = MeshTopology::default();
        assert_eq!(topology.scheme().as_str(), "bilinear");
        assert_eq!(topology.orientation().as_str(), "rightHanded");
        assert!(topology.face_vertex_counts().is_empty());
        assert!(topology.face_vertex_indices().is_empty());
        assert!(topology.hole_indices().is_empty());
    }

    #[test]
    fn test_new() {
        let topology = MeshTopology::new(
            tokens::CATMULL_CLARK.clone(),
            tokens::LEFT_HANDED.clone(),
            vec![4, 4],
            vec![0, 1, 2, 3, 4, 5, 6, 7],
        );

        assert_eq!(topology.scheme().as_str(), "catmullClark");
        assert_eq!(topology.orientation().as_str(), "leftHanded");
        assert_eq!(topology.face_vertex_counts().len(), 2);
        assert_eq!(topology.face_vertex_indices().len(), 8);
    }

    #[test]
    fn test_with_holes() {
        let topology = MeshTopology::with_holes(
            tokens::LOOP.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![3, 3],
            vec![0, 1, 2, 3, 4, 5],
            vec![1],
        );

        assert_eq!(topology.scheme().as_str(), "loop");
        assert_eq!(topology.hole_indices().len(), 1);
        assert_eq!(topology.hole_indices()[0], 1);
    }

    #[test]
    fn test_with_tags() {
        let tags = SubdivTags::new(
            tokens::EDGE_AND_CORNER.clone(),
            tokens::BOUNDARIES.clone(),
            tokens::UNIFORM.clone(),
            tokens::SMOOTH.clone(),
            vec![0, 1],
            vec![2],
            vec![1.0],
            vec![],
            vec![],
        );

        let topology = MeshTopology::with_tags(
            tokens::CATMULL_CLARK.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![4],
            vec![0, 1, 2, 3],
            tags,
        );

        assert_eq!(
            topology.subdiv_tags().vertex_interpolation_rule().as_str(),
            "edgeAndCorner"
        );
    }

    #[test]
    fn test_builder_methods() {
        let topology = MeshTopology::new(
            tokens::BILINEAR.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![4],
            vec![0, 1, 2, 3],
        );

        // Test with_scheme
        let new_topology = topology.with_scheme(tokens::CATMULL_CLARK.clone());
        assert_eq!(new_topology.scheme().as_str(), "catmullClark");
        assert_eq!(topology.scheme().as_str(), "bilinear"); // Original unchanged

        // Test with_orientation
        let new_topology = topology.with_orientation(tokens::LEFT_HANDED.clone());
        assert_eq!(new_topology.orientation().as_str(), "leftHanded");
        assert_eq!(topology.orientation().as_str(), "rightHanded");

        // Test with_hole_indices
        let new_topology = topology.with_hole_indices(vec![0]);
        assert_eq!(new_topology.hole_indices().len(), 1);
        assert!(topology.hole_indices().is_empty());
    }

    #[test]
    fn test_equality() {
        let topology1 = MeshTopology::new(
            tokens::CATMULL_CLARK.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![4],
            vec![0, 1, 2, 3],
        );

        let topology2 = topology1.clone();
        assert_eq!(topology1, topology2);

        let topology3 = topology1.with_scheme(tokens::LOOP.clone());
        assert_ne!(topology1, topology3);
    }

    #[test]
    fn test_compute_hash() {
        let topology1 = MeshTopology::new(
            tokens::CATMULL_CLARK.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![4],
            vec![0, 1, 2, 3],
        );

        let topology2 = topology1.clone();
        assert_eq!(topology1.compute_hash(), topology2.compute_hash());

        let topology3 = topology1.with_scheme(tokens::LOOP.clone());
        // Different topologies should have different hashes (highly likely)
        assert_ne!(topology1.compute_hash(), topology3.compute_hash());
    }

    #[test]
    fn test_validate_simple_cube() {
        // Simple cube: 6 quads, 8 vertices
        let topology = MeshTopology::new(
            tokens::CATMULL_CLARK.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![4, 4, 4, 4, 4, 4],
            vec![
                0, 1, 5, 4, // Front
                1, 2, 6, 5, // Right
                2, 3, 7, 6, // Back
                3, 0, 4, 7, // Left
                4, 5, 6, 7, // Top
                0, 3, 2, 1, // Bottom
            ],
        );

        let validation = topology.validate();
        assert!(validation.is_valid(), "Cube topology should be valid");
    }

    #[test]
    fn test_validate_invalid_scheme() {
        let topology = MeshTopology::new(
            Token::new("invalid_scheme"),
            tokens::RIGHT_HANDED.clone(),
            vec![4],
            vec![0, 1, 2, 3],
        );

        let validation = topology.validate();
        assert!(!validation.is_valid());
        assert!(!validation.errors().is_empty());
    }

    #[test]
    fn test_validate_invalid_face_counts() {
        let topology = MeshTopology::new(
            tokens::CATMULL_CLARK.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![2], // Invalid: less than 3
            vec![0, 1],
        );

        let validation = topology.validate();
        assert!(!validation.is_valid());
    }

    #[test]
    fn test_validate_mismatched_indices_size() {
        let topology = MeshTopology::new(
            tokens::CATMULL_CLARK.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![4],       // Expects 4 indices
            vec![0, 1, 2], // Only 3 indices
        );

        let validation = topology.validate();
        assert!(!validation.is_valid());
    }

    #[test]
    fn test_validate_negative_indices() {
        let topology = MeshTopology::new(
            tokens::CATMULL_CLARK.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![4],
            vec![0, -1, 2, 3], // Negative index
        );

        let validation = topology.validate();
        assert!(!validation.is_valid());
    }

    #[test]
    fn test_validate_invalid_holes() {
        let topology = MeshTopology::with_holes(
            tokens::CATMULL_CLARK.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![4],
            vec![0, 1, 2, 3],
            vec![5], // Hole index >= face count
        );

        let validation = topology.validate();
        assert!(!validation.is_valid());
    }

    #[test]
    fn test_validate_caching() {
        let topology = MeshTopology::new(
            tokens::CATMULL_CLARK.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![4],
            vec![0, 1, 2, 3],
        );

        // First validation
        let validation1 = topology.validate();
        assert!(validation1.is_valid());

        // Second validation should use cache
        let validation2 = topology.validate();
        assert!(validation2.is_valid());
    }

    #[test]
    fn test_display() {
        let topology = MeshTopology::new(
            tokens::CATMULL_CLARK.clone(),
            tokens::RIGHT_HANDED.clone(),
            vec![4],
            vec![0, 1, 2, 3],
        );

        let display = format!("{}", topology);
        assert!(display.contains("rightHanded"));
        assert!(display.contains("catmullClark"));
    }
}
