//! Subdivision tags for non-hierarchical subdivision surfaces.
//!
//! This module provides the `SubdivTags` struct which contains all the
//! subdivision parameters for a mesh, including creases, corners, and
//! interpolation rules.

use std::hash::{Hash, Hasher};
use usd_tf::Token;

use crate::enums::{
    CreasingMethod, FVarLinearInterpolation, TriangleSubdivision, VtxBoundaryInterpolation,
};

/// Tags for non-hierarchical subdivision surfaces.
///
/// Contains all subdivision parameters including:
/// - Interpolation rules for vertices and face-varying data
/// - Crease method for edge sharpness
/// - Triangle subdivision method
/// - Edge creases (indices, lengths, weights)
/// - Corner sharpness values
#[derive(Debug, Clone, PartialEq)]
pub struct SubdivTags {
    /// Vertex boundary interpolation rule.
    /// Valid values: "none", "edgeOnly", "edgeAndCorner"
    vertex_interpolation_rule: Token,

    /// Face-varying boundary interpolation rule.
    /// Valid values: "none", "all", "boundaries", "cornersOnly", "cornersPlus1", "cornersPlus2"
    face_varying_interpolation_rule: Token,

    /// Crease sharpness computation method.
    /// Valid values: "uniform", "chaikin"
    crease_method: Token,

    /// Triangle subdivision method.
    /// Valid values: "catmullClark", "smooth", ""
    triangle_subdivision: Token,

    /// Indices of vertices that form crease edges.
    /// Organized as consecutive pairs defining edges.
    /// Size must equal sum of crease_lengths.
    crease_indices: Vec<i32>,

    /// Length of each crease loop (minimum 2).
    /// Each entry defines how many vertices form one crease.
    crease_lengths: Vec<i32>,

    /// Sharpness weights for creases.
    /// Size must be either:
    /// - Number of crease edges (sum of lengths - number of creases)
    /// - Number of creases (one weight per crease)
    crease_weights: Vec<f32>,

    /// Indices of corner vertices.
    /// Must reference valid vertices in the mesh.
    corner_indices: Vec<i32>,

    /// Sharpness weights for corners.
    /// Size must equal corner_indices.len()
    corner_weights: Vec<f32>,
}

impl SubdivTags {
    /// Create new subdivision tags with all parameters.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        vertex_interpolation_rule: Token,
        face_varying_interpolation_rule: Token,
        crease_method: Token,
        triangle_subdivision: Token,
        crease_indices: Vec<i32>,
        crease_lengths: Vec<i32>,
        crease_weights: Vec<f32>,
        corner_indices: Vec<i32>,
        corner_weights: Vec<f32>,
    ) -> Self {
        Self {
            vertex_interpolation_rule,
            face_varying_interpolation_rule,
            crease_method,
            triangle_subdivision,
            crease_indices,
            crease_lengths,
            crease_weights,
            corner_indices,
            corner_weights,
        }
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Get the vertex boundary interpolation rule.
    pub fn vertex_interpolation_rule(&self) -> &Token {
        &self.vertex_interpolation_rule
    }

    /// Set the vertex boundary interpolation rule.
    pub fn set_vertex_interpolation_rule(&mut self, rule: Token) {
        self.vertex_interpolation_rule = rule;
    }

    /// Get the face-varying boundary interpolation rule.
    pub fn face_varying_interpolation_rule(&self) -> &Token {
        &self.face_varying_interpolation_rule
    }

    /// Set the face-varying boundary interpolation rule.
    pub fn set_face_varying_interpolation_rule(&mut self, rule: Token) {
        self.face_varying_interpolation_rule = rule;
    }

    /// Get the crease method.
    pub fn crease_method(&self) -> &Token {
        &self.crease_method
    }

    /// Set the crease method.
    pub fn set_crease_method(&mut self, method: Token) {
        self.crease_method = method;
    }

    /// Get the triangle subdivision method.
    pub fn triangle_subdivision(&self) -> &Token {
        &self.triangle_subdivision
    }

    /// Set the triangle subdivision method.
    pub fn set_triangle_subdivision(&mut self, method: Token) {
        self.triangle_subdivision = method;
    }

    // ========================================================================
    // Crease Accessors
    // ========================================================================

    /// Get edge crease indices.
    pub fn crease_indices(&self) -> &[i32] {
        &self.crease_indices
    }

    /// Set edge crease indices.
    pub fn set_crease_indices(&mut self, indices: Vec<i32>) {
        self.crease_indices = indices;
    }

    /// Get edge crease loop lengths.
    pub fn crease_lengths(&self) -> &[i32] {
        &self.crease_lengths
    }

    /// Set edge crease loop lengths.
    pub fn set_crease_lengths(&mut self, lengths: Vec<i32>) {
        self.crease_lengths = lengths;
    }

    /// Get edge crease weights (sharpness values).
    pub fn crease_weights(&self) -> &[f32] {
        &self.crease_weights
    }

    /// Set edge crease weights (sharpness values).
    pub fn set_crease_weights(&mut self, weights: Vec<f32>) {
        self.crease_weights = weights;
    }

    // ========================================================================
    // Corner Accessors
    // ========================================================================

    /// Get corner vertex indices.
    pub fn corner_indices(&self) -> &[i32] {
        &self.corner_indices
    }

    /// Set corner vertex indices.
    pub fn set_corner_indices(&mut self, indices: Vec<i32>) {
        self.corner_indices = indices;
    }

    /// Get corner weights (sharpness values).
    pub fn corner_weights(&self) -> &[f32] {
        &self.corner_weights
    }

    /// Set corner weights (sharpness values).
    pub fn set_corner_weights(&mut self, weights: Vec<f32>) {
        self.corner_weights = weights;
    }

    // ========================================================================
    // Typed enum accessors (convenience wrappers over raw token strings)
    // ========================================================================

    /// Return vertex boundary interpolation as a typed enum.
    ///
    /// Parses the raw token; falls back to `EdgeAndCorner` on unknown values.
    pub fn get_vtx_boundary_interpolation(&self) -> VtxBoundaryInterpolation {
        self.vertex_interpolation_rule
            .as_str()
            .parse()
            .unwrap_or_default()
    }

    /// Return face-varying linear interpolation as a typed enum.
    ///
    /// Parses the raw token; falls back to `All` on unknown values.
    pub fn get_fvar_linear_interpolation(&self) -> FVarLinearInterpolation {
        self.face_varying_interpolation_rule
            .as_str()
            .parse()
            .unwrap_or_default()
    }

    /// Return crease method as a typed enum.
    ///
    /// Parses the raw token; falls back to `Uniform` on unknown/empty values.
    pub fn get_crease_method(&self) -> CreasingMethod {
        self.crease_method.as_str().parse().unwrap_or_default()
    }

    /// Return triangle subdivision method as a typed enum.
    ///
    /// Parses the raw token; falls back to `CatmullClark` on unknown/empty values.
    pub fn get_triangle_subdivision(&self) -> TriangleSubdivision {
        self.triangle_subdivision
            .as_str()
            .parse()
            .unwrap_or_default()
    }

    // ========================================================================
    // Hash computation
    // ========================================================================

    /// Compute a hash value for instancing.
    pub fn compute_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        self.vertex_interpolation_rule.as_str().hash(&mut hasher);
        self.face_varying_interpolation_rule
            .as_str()
            .hash(&mut hasher);
        self.crease_method.as_str().hash(&mut hasher);
        self.triangle_subdivision.as_str().hash(&mut hasher);

        self.corner_indices.hash(&mut hasher);
        // Hash corner weights as bytes
        for &w in &self.corner_weights {
            w.to_bits().hash(&mut hasher);
        }

        self.crease_indices.hash(&mut hasher);
        self.crease_lengths.hash(&mut hasher);
        // Hash crease weights as bytes
        for &w in &self.crease_weights {
            w.to_bits().hash(&mut hasher);
        }

        hasher.finish()
    }
}

impl Default for SubdivTags {
    fn default() -> Self {
        Self {
            vertex_interpolation_rule: Token::default(),
            face_varying_interpolation_rule: Token::default(),
            crease_method: Token::default(),
            triangle_subdivision: Token::default(),
            crease_indices: Vec::new(),
            crease_lengths: Vec::new(),
            crease_weights: Vec::new(),
            corner_indices: Vec::new(),
            corner_weights: Vec::new(),
        }
    }
}

impl std::fmt::Display for SubdivTags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "({}, {}, {}, {}, {:?}, {:?}, {:?}, {:?}, {:?})",
            self.vertex_interpolation_rule.as_str(),
            self.face_varying_interpolation_rule.as_str(),
            self.crease_method.as_str(),
            self.triangle_subdivision.as_str(),
            self.crease_indices,
            self.crease_lengths,
            self.crease_weights,
            self.corner_indices,
            self.corner_weights
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens;

    #[test]
    fn test_default() {
        let tags = SubdivTags::default();
        assert!(tags.crease_indices().is_empty());
        assert!(tags.corner_indices().is_empty());
    }

    #[test]
    fn test_new() {
        let tags = SubdivTags::new(
            tokens::EDGE_AND_CORNER.clone(),
            tokens::BOUNDARIES.clone(),
            tokens::UNIFORM.clone(),
            tokens::SMOOTH.clone(),
            vec![0, 1, 1, 2],
            vec![2, 2],
            vec![1.0, 0.5],
            vec![0],
            vec![2.0],
        );

        assert_eq!(tags.vertex_interpolation_rule().as_str(), "edgeAndCorner");
        assert_eq!(
            tags.face_varying_interpolation_rule().as_str(),
            "boundaries"
        );
        assert_eq!(tags.crease_method().as_str(), "uniform");
        assert_eq!(tags.triangle_subdivision().as_str(), "smooth");
        assert_eq!(tags.crease_indices().len(), 4);
        assert_eq!(tags.crease_lengths().len(), 2);
        assert_eq!(tags.crease_weights().len(), 2);
        assert_eq!(tags.corner_indices().len(), 1);
        assert_eq!(tags.corner_weights().len(), 1);
    }

    #[test]
    fn test_setters() {
        let mut tags = SubdivTags::default();

        tags.set_vertex_interpolation_rule(tokens::EDGE_ONLY.clone());
        tags.set_crease_indices(vec![0, 1, 2, 3]);
        tags.set_corner_weights(vec![1.0, 2.0]);

        assert_eq!(tags.vertex_interpolation_rule().as_str(), "edgeOnly");
        assert_eq!(tags.crease_indices().len(), 4);
        assert_eq!(tags.corner_weights().len(), 2);
    }

    #[test]
    fn test_equality() {
        let tags1 = SubdivTags::new(
            tokens::EDGE_AND_CORNER.clone(),
            tokens::BOUNDARIES.clone(),
            tokens::UNIFORM.clone(),
            tokens::SMOOTH.clone(),
            vec![0, 1],
            vec![2],
            vec![1.0],
            vec![0],
            vec![2.0],
        );

        let tags2 = tags1.clone();
        assert_eq!(tags1, tags2);

        let tags3 = SubdivTags::new(
            tokens::EDGE_ONLY.clone(),
            tokens::BOUNDARIES.clone(),
            tokens::UNIFORM.clone(),
            tokens::SMOOTH.clone(),
            vec![0, 1],
            vec![2],
            vec![1.0],
            vec![0],
            vec![2.0],
        );

        assert_ne!(tags1, tags3);
    }

    #[test]
    fn test_compute_hash() {
        let tags1 = SubdivTags::new(
            tokens::EDGE_AND_CORNER.clone(),
            tokens::BOUNDARIES.clone(),
            tokens::UNIFORM.clone(),
            tokens::SMOOTH.clone(),
            vec![0, 1],
            vec![2],
            vec![1.0],
            vec![0],
            vec![2.0],
        );

        let tags2 = tags1.clone();
        assert_eq!(tags1.compute_hash(), tags2.compute_hash());

        let tags3 = SubdivTags::new(
            tokens::EDGE_ONLY.clone(),
            tokens::BOUNDARIES.clone(),
            tokens::UNIFORM.clone(),
            tokens::SMOOTH.clone(),
            vec![0, 1],
            vec![2],
            vec![1.0],
            vec![0],
            vec![2.0],
        );

        // Different tags should produce different hashes (highly likely)
        assert_ne!(tags1.compute_hash(), tags3.compute_hash());
    }

    #[test]
    fn test_display() {
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

        let display = format!("{}", tags);
        assert!(display.contains("edgeAndCorner"));
        assert!(display.contains("boundaries"));
        assert!(display.contains("uniform"));
        assert!(display.contains("smooth"));
    }

    #[test]
    fn test_typed_getters() {
        use crate::enums::{
            CreasingMethod, FVarLinearInterpolation, TriangleSubdivision, VtxBoundaryInterpolation,
        };

        let tags = SubdivTags::new(
            tokens::EDGE_AND_CORNER.clone(),
            tokens::BOUNDARIES.clone(),
            tokens::CHAIKIN.clone(),
            tokens::SMOOTH.clone(),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        );

        assert_eq!(
            tags.get_vtx_boundary_interpolation(),
            VtxBoundaryInterpolation::EdgeAndCorner
        );
        assert_eq!(
            tags.get_fvar_linear_interpolation(),
            FVarLinearInterpolation::Boundaries
        );
        assert_eq!(tags.get_crease_method(), CreasingMethod::Chaikin);
        assert_eq!(tags.get_triangle_subdivision(), TriangleSubdivision::Smooth);
    }

    #[test]
    fn test_typed_getters_defaults() {
        use crate::enums::{
            CreasingMethod, FVarLinearInterpolation, TriangleSubdivision, VtxBoundaryInterpolation,
        };

        // Empty tokens fall back to enum defaults
        let tags = SubdivTags::default();
        assert_eq!(
            tags.get_vtx_boundary_interpolation(),
            VtxBoundaryInterpolation::EdgeAndCorner
        );
        assert_eq!(
            tags.get_fvar_linear_interpolation(),
            FVarLinearInterpolation::All
        );
        assert_eq!(tags.get_crease_method(), CreasingMethod::Uniform);
        assert_eq!(
            tags.get_triangle_subdivision(),
            TriangleSubdivision::CatmullClark
        );
    }
}
