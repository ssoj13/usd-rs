//! Mesh topology validation utilities.
//!
//! This module provides validation for OpenSubdiv mesh topology, checking
//! for common errors in subdivision surface specifications.

use super::tokens;
use usd_tf::Token;

/// Validation error codes for mesh topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValidationCode {
    /// Invalid subdivision scheme token.
    InvalidScheme,
    /// Invalid orientation token.
    InvalidOrientation,
    /// Invalid triangle subdivision token.
    InvalidTriangleSubdivision,
    /// Invalid vertex interpolation rule token.
    InvalidVertexInterpolationRule,
    /// Invalid face-varying interpolation rule token.
    InvalidFaceVaryingInterpolationRule,
    /// Invalid crease method token.
    InvalidCreaseMethod,
    /// Crease length element is less than 2.
    InvalidCreaseLengthElement,
    /// Crease indices size doesn't match sum of lengths array.
    InvalidCreaseIndicesSize,
    /// Crease index element not found in face vertex indices.
    InvalidCreaseIndicesElement,
    /// Crease weights size is invalid (must be per-edge or per-crease).
    InvalidCreaseWeightsSize,
    /// Crease weights contain negative values.
    NegativeCreaseWeights,
    /// Corner index element not found in face vertex indices.
    InvalidCornerIndicesElement,
    /// Corner weights contain negative values.
    NegativeCornerWeights,
    /// Corner weights size doesn't match corner indices size.
    InvalidCornerWeightsSize,
    /// Hole index is negative or exceeds maximum face index.
    InvalidHoleIndicesElement,
    /// Face vertex count is less than 3.
    InvalidFaceVertexCountsElement,
    /// Face vertex index is negative.
    InvalidFaceVertexIndicesElement,
    /// Face vertex indices size doesn't match sum of face vertex counts.
    InvalidFaceVertexIndicesSize,
}

impl std::fmt::Display for ValidationCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidScheme => write!(f, "InvalidScheme"),
            Self::InvalidOrientation => write!(f, "InvalidOrientation"),
            Self::InvalidTriangleSubdivision => write!(f, "InvalidTriangleSubdivision"),
            Self::InvalidVertexInterpolationRule => write!(f, "InvalidVertexInterpolationRule"),
            Self::InvalidFaceVaryingInterpolationRule => {
                write!(f, "InvalidFaceVaryingInterpolationRule")
            }
            Self::InvalidCreaseMethod => write!(f, "InvalidCreaseMethod"),
            Self::InvalidCreaseLengthElement => write!(f, "InvalidCreaseLengthElement"),
            Self::InvalidCreaseIndicesSize => write!(f, "InvalidCreaseIndicesSize"),
            Self::InvalidCreaseIndicesElement => write!(f, "InvalidCreaseIndicesElement"),
            Self::InvalidCreaseWeightsSize => write!(f, "InvalidCreaseWeightsSize"),
            Self::NegativeCreaseWeights => write!(f, "NegativeCreaseWeights"),
            Self::InvalidCornerIndicesElement => write!(f, "InvalidCornerIndicesElement"),
            Self::NegativeCornerWeights => write!(f, "NegativeCornerWeights"),
            Self::InvalidCornerWeightsSize => write!(f, "InvalidCornerWeightsSize"),
            Self::InvalidHoleIndicesElement => write!(f, "InvalidHoleIndicesElement"),
            Self::InvalidFaceVertexCountsElement => write!(f, "InvalidFaceVertexCountsElement"),
            Self::InvalidFaceVertexIndicesElement => write!(f, "InvalidFaceVertexIndicesElement"),
            Self::InvalidFaceVertexIndicesSize => write!(f, "InvalidFaceVertexIndicesSize"),
        }
    }
}

/// A validation error with code and descriptive message.
#[derive(Debug, Clone)]
pub struct Invalidation {
    /// Error code identifying the type of validation failure.
    pub code: ValidationCode,
    /// Human-readable description of the error.
    pub message: String,
}

impl Invalidation {
    /// Create a new validation error.
    pub fn new(code: ValidationCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Invalidation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// Result of mesh topology validation.
///
/// An empty validation result indicates valid topology.
/// Non-empty result contains a list of validation errors.
///
/// # Example
/// ```ignore
/// if !validation.is_valid() {
///     for error in validation.errors() {
///         eprintln!("Validation error: {}", error);
///     }
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct MeshTopologyValidation {
    errors: Vec<Invalidation>,
}

impl MeshTopologyValidation {
    /// Create a new empty (valid) validation result.
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Add a validation error.
    pub(crate) fn add_error(&mut self, error: Invalidation) {
        self.errors.push(error);
    }

    /// Check if the topology is valid (no errors).
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get the list of validation errors.
    pub fn errors(&self) -> &[Invalidation] {
        &self.errors
    }

    /// Get an iterator over validation errors.
    pub fn iter(&self) -> std::slice::Iter<'_, Invalidation> {
        self.errors.iter()
    }

    // ========================================================================
    // Validation helper methods
    // ========================================================================

    /// Validate a token against a list of valid options.
    /// Matches C++ _ValidateToken: reports error if token is not in the valid list.
    pub(crate) fn validate_token(
        &mut self,
        code: ValidationCode,
        name: &str,
        token: &Token,
        valid_tokens: &[&Token],
    ) {
        if !valid_tokens.contains(&token) {
            self.add_error(Invalidation::new(
                code,
                format!("'{}' is not a valid '{}' token.", token.as_str(), name),
            ));
        }
    }

    /// Validate subdivision scheme.
    pub(crate) fn validate_scheme(&mut self, scheme: &Token) {
        let valid_schemes = [&*tokens::CATMULL_CLARK, &*tokens::LOOP, &*tokens::BILINEAR];
        self.validate_token(
            ValidationCode::InvalidScheme,
            "scheme",
            scheme,
            &valid_schemes,
        );
    }

    /// Validate orientation.
    pub(crate) fn validate_orientation(&mut self, orientation: &Token) {
        let valid_orientations = [&*tokens::RIGHT_HANDED, &*tokens::LEFT_HANDED];
        self.validate_token(
            ValidationCode::InvalidOrientation,
            "orientation",
            orientation,
            &valid_orientations,
        );
    }

    /// Validate triangle subdivision method.
    pub(crate) fn validate_triangle_subdivision(&mut self, method: &Token) {
        let valid_methods = [&*tokens::CATMULL_CLARK, &*tokens::SMOOTH, &Token::default()];
        self.validate_token(
            ValidationCode::InvalidTriangleSubdivision,
            "triangle subdivision",
            method,
            &valid_methods,
        );
    }

    /// Validate vertex interpolation rule.
    pub(crate) fn validate_vertex_interpolation(&mut self, rule: &Token) {
        let valid_rules = [
            &*tokens::NONE,
            &*tokens::EDGE_AND_CORNER,
            &*tokens::EDGE_ONLY,
            &Token::default(),
        ];
        self.validate_token(
            ValidationCode::InvalidVertexInterpolationRule,
            "vertex interpolation rule",
            rule,
            &valid_rules,
        );
    }

    /// Validate face-varying interpolation rule.
    pub(crate) fn validate_face_varying_interpolation(&mut self, rule: &Token) {
        let valid_rules = [
            &*tokens::NONE,
            &*tokens::ALL,
            &*tokens::BOUNDARIES,
            &*tokens::CORNERS_ONLY,
            &*tokens::CORNERS_PLUS1,
            &*tokens::CORNERS_PLUS2,
            &Token::default(),
        ];
        self.validate_token(
            ValidationCode::InvalidFaceVaryingInterpolationRule,
            "face varying interpolation rule",
            rule,
            &valid_rules,
        );
    }

    /// Validate crease method.
    pub(crate) fn validate_crease_method(&mut self, method: &Token) {
        let valid_methods = [&*tokens::UNIFORM, &*tokens::CHAIKIN, &Token::default()];
        self.validate_token(
            ValidationCode::InvalidCreaseMethod,
            "crease method",
            method,
            &valid_methods,
        );
    }

    /// Validate creases and corners against face vertex indices.
    pub(crate) fn validate_creases_and_corners(
        &mut self,
        crease_indices: &[i32],
        crease_lengths: &[i32],
        crease_weights: &[f32],
        corner_indices: &[i32],
        corner_weights: &[f32],
        face_vertex_indices: &[i32],
    ) {
        // Validate crease lengths (must be >= 2)
        if crease_lengths.iter().any(|&len| len < 2) {
            self.add_error(Invalidation::new(
                ValidationCode::InvalidCreaseLengthElement,
                "Crease lengths must be greater than or equal to 2.",
            ));
        }

        // Validate crease indices size
        let total_crease_indices: i32 = crease_lengths.iter().sum();
        let total_creases = crease_lengths.len();
        let total_crease_edges = total_crease_indices as usize - total_creases;

        if crease_indices.len() != total_crease_indices as usize {
            self.add_error(Invalidation::new(
                ValidationCode::InvalidCreaseIndicesSize,
                format!(
                    "Crease indices size '{}' doesn't match expected '{}'.",
                    crease_indices.len(),
                    total_crease_indices
                ),
            ));
        }

        // Validate crease weights size
        if crease_weights.len() != total_crease_edges && crease_weights.len() != total_creases {
            self.add_error(Invalidation::new(
                ValidationCode::InvalidCreaseWeightsSize,
                format!(
                    "Crease weights size '{}' doesn't match either per edge '{}' or per crease '{}' sizes.",
                    crease_weights.len(),
                    total_crease_edges,
                    total_creases
                ),
            ));
        }

        // Validate corner weights size
        if corner_indices.len() != corner_weights.len() {
            self.add_error(Invalidation::new(
                ValidationCode::InvalidCornerWeightsSize,
                format!(
                    "Corner weights size '{}' doesn't match expected '{}'.",
                    corner_indices.len(),
                    corner_weights.len()
                ),
            ));
        }

        // Check for negative weights
        if crease_weights.iter().any(|&w| w < 0.0) {
            self.add_error(Invalidation::new(
                ValidationCode::NegativeCreaseWeights,
                "Negative crease weights.",
            ));
        }

        if corner_weights.iter().any(|&w| w < 0.0) {
            self.add_error(Invalidation::new(
                ValidationCode::NegativeCornerWeights,
                "Negative corner weights.",
            ));
        }

        // Validate that crease and corner indices exist in face vertex indices
        let mut sorted_face_indices = face_vertex_indices.to_vec();
        sorted_face_indices.sort_unstable();

        for &idx in corner_indices {
            if sorted_face_indices.binary_search(&idx).is_err() {
                self.add_error(Invalidation::new(
                    ValidationCode::InvalidCornerIndicesElement,
                    "Corner index element missing from face vertex indices array.",
                ));
                break;
            }
        }

        for &idx in crease_indices {
            if sorted_face_indices.binary_search(&idx).is_err() {
                self.add_error(Invalidation::new(
                    ValidationCode::InvalidCreaseIndicesElement,
                    "Crease index element missing from face vertex indices array.",
                ));
                break;
            }
        }
    }

    /// Validate hole indices.
    pub(crate) fn validate_holes(&mut self, hole_indices: &[i32], face_count: usize) {
        if hole_indices.is_empty() {
            return;
        }

        if let Some(&min_idx) = hole_indices.iter().min() {
            if min_idx < 0 {
                self.add_error(Invalidation::new(
                    ValidationCode::InvalidHoleIndicesElement,
                    "Hole indices cannot be negative.",
                ));
            }
        }

        if let Some(&max_idx) = hole_indices.iter().max() {
            if max_idx >= face_count as i32 {
                self.add_error(Invalidation::new(
                    ValidationCode::InvalidHoleIndicesElement,
                    format!(
                        "Hole indices must be less than face count '{}'.",
                        face_count
                    ),
                ));
            }
        }
    }

    /// Validate face vertex counts.
    pub(crate) fn validate_face_vertex_counts(&mut self, face_vertex_counts: &[i32]) {
        if face_vertex_counts.iter().any(|&count| count <= 2) {
            self.add_error(Invalidation::new(
                ValidationCode::InvalidFaceVertexCountsElement,
                "Face vertex counts must be greater than 2.",
            ));
        }
    }

    /// Validate face vertex indices.
    pub(crate) fn validate_face_vertex_indices(
        &mut self,
        face_vertex_indices: &[i32],
        face_vertex_counts: &[i32],
    ) {
        // Check for negative indices
        if face_vertex_indices.iter().any(|&idx| idx < 0) {
            self.add_error(Invalidation::new(
                ValidationCode::InvalidFaceVertexIndicesElement,
                "Face vertex indices element must be greater than 0.",
            ));
        }

        // Check size matches sum of counts
        let expected_size: i32 = face_vertex_counts.iter().sum();
        if face_vertex_indices.len() != expected_size as usize {
            self.add_error(Invalidation::new(
                ValidationCode::InvalidFaceVertexIndicesSize,
                format!(
                    "Face vertex indices size '{}' does not match expected size '{}'.",
                    face_vertex_indices.len(),
                    expected_size
                ),
            ));
        }
    }
}

// Convert to bool for easy validity checking
impl From<&MeshTopologyValidation> for bool {
    fn from(validation: &MeshTopologyValidation) -> bool {
        validation.is_valid()
    }
}

// Iterator support
impl<'a> IntoIterator for &'a MeshTopologyValidation {
    type Item = &'a Invalidation;
    type IntoIter = std::slice::Iter<'a, Invalidation>;

    fn into_iter(self) -> Self::IntoIter {
        self.errors.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_code_display() {
        assert_eq!(ValidationCode::InvalidScheme.to_string(), "InvalidScheme");
        assert_eq!(
            ValidationCode::NegativeCreaseWeights.to_string(),
            "NegativeCreaseWeights"
        );
    }

    #[test]
    fn test_invalidation() {
        let error = Invalidation::new(ValidationCode::InvalidScheme, "Invalid scheme token");
        assert_eq!(error.code, ValidationCode::InvalidScheme);
        assert_eq!(error.message, "Invalid scheme token");

        let display = format!("{}", error);
        assert!(display.contains("InvalidScheme"));
        assert!(display.contains("Invalid scheme token"));
    }

    #[test]
    fn test_empty_validation() {
        let validation = MeshTopologyValidation::new();
        assert!(validation.is_valid());
        assert!(validation.errors().is_empty());
        assert!(bool::from(&validation));
    }

    #[test]
    fn test_validation_with_errors() {
        let mut validation = MeshTopologyValidation::new();
        validation.add_error(Invalidation::new(
            ValidationCode::InvalidScheme,
            "Bad scheme",
        ));

        assert!(!validation.is_valid());
        assert_eq!(validation.errors().len(), 1);
        assert!(!bool::from(&validation));
    }

    #[test]
    fn test_validate_scheme() {
        let mut validation = MeshTopologyValidation::new();

        // Valid schemes
        validation.validate_scheme(&tokens::CATMULL_CLARK);
        validation.validate_scheme(&tokens::LOOP);
        validation.validate_scheme(&tokens::BILINEAR);
        assert!(validation.is_valid());

        // Invalid scheme
        validation.validate_scheme(&Token::new("invalid"));
        assert!(!validation.is_valid());
    }

    #[test]
    fn test_validate_crease_lengths() {
        let mut validation = MeshTopologyValidation::new();
        let face_vertex_indices = vec![0, 1, 2, 3];

        // Invalid crease length < 2
        validation.validate_creases_and_corners(
            &[0, 1],
            &[1], // Length < 2
            &[1.0],
            &[],
            &[],
            &face_vertex_indices,
        );
        assert!(!validation.is_valid());
    }

    #[test]
    fn test_validate_crease_indices_size() {
        let mut validation = MeshTopologyValidation::new();
        let face_vertex_indices = vec![0, 1, 2, 3];

        // Crease indices size mismatch
        validation.validate_creases_and_corners(
            &[0, 1], // Size 2
            &[3],    // Expected size 3
            &[],
            &[],
            &[],
            &face_vertex_indices,
        );
        assert!(!validation.is_valid());
    }

    #[test]
    fn test_validate_negative_weights() {
        let mut validation = MeshTopologyValidation::new();
        let face_vertex_indices = vec![0, 1, 2, 3];

        // Negative crease weights
        validation.validate_creases_and_corners(
            &[0, 1],
            &[2],
            &[-1.0], // Negative weight
            &[],
            &[],
            &face_vertex_indices,
        );
        assert!(!validation.is_valid());
    }

    #[test]
    fn test_validate_holes() {
        let mut validation = MeshTopologyValidation::new();

        // Valid holes
        validation.validate_holes(&[0, 1, 2], 10);
        assert!(validation.is_valid());

        // Negative hole index
        let mut validation = MeshTopologyValidation::new();
        validation.validate_holes(&[-1], 10);
        assert!(!validation.is_valid());

        // Hole index >= face count
        let mut validation = MeshTopologyValidation::new();
        validation.validate_holes(&[10], 10);
        assert!(!validation.is_valid());
    }

    #[test]
    fn test_validate_face_vertex_counts() {
        let mut validation = MeshTopologyValidation::new();

        // Valid counts
        validation.validate_face_vertex_counts(&[3, 4, 5]);
        assert!(validation.is_valid());

        // Invalid count <= 2
        let mut validation = MeshTopologyValidation::new();
        validation.validate_face_vertex_counts(&[3, 2, 4]);
        assert!(!validation.is_valid());
    }

    #[test]
    fn test_validate_face_vertex_indices() {
        let mut validation = MeshTopologyValidation::new();

        // Valid indices
        validation.validate_face_vertex_indices(&[0, 1, 2, 3, 4, 5], &[3, 3]);
        assert!(validation.is_valid());

        // Size mismatch
        let mut validation = MeshTopologyValidation::new();
        validation.validate_face_vertex_indices(&[0, 1, 2, 3], &[3, 3]);
        assert!(!validation.is_valid());

        // Negative index
        let mut validation = MeshTopologyValidation::new();
        validation.validate_face_vertex_indices(&[0, -1, 2], &[3]);
        assert!(!validation.is_valid());
    }

    #[test]
    fn test_iterator() {
        let mut validation = MeshTopologyValidation::new();
        validation.add_error(Invalidation::new(ValidationCode::InvalidScheme, "Error 1"));
        validation.add_error(Invalidation::new(
            ValidationCode::InvalidOrientation,
            "Error 2",
        ));

        let mut count = 0;
        for error in &validation {
            count += 1;
            assert!(!error.message.is_empty());
        }
        assert_eq!(count, 2);
    }
}
