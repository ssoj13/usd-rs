//! Oriented 3D bounding box.
//!
//! BBox3d represents a three-dimensional bounding box as an axis-aligned box
//! ([`Range3d`]) and a transformation matrix ([`Matrix4d`]).
//!
//! This is more useful than Range3d alone because:
//! - When transformed multiple times, the transformation can be applied once
//!   at the end, resulting in a tighter fit.
//! - When combining boxes, there's opportunity to choose a better coordinate
//!   space for the combination.
//!
//! # Zero-area Primitives Flag
//!
//! For intersection test culling, it's sometimes useful to extend bounding
//! boxes to allow lower-dimensional objects (lines, points) to be intersected.
//! The `has_zero_area_primitives` flag indicates when this loosening is needed.
//!
//! # Examples
//!
//! ```
//! use usd_gf::{BBox3d, Range3d, Matrix4d, vec3d};
//!
//! let range = Range3d::new(vec3d(0.0, 0.0, 0.0), vec3d(1.0, 1.0, 1.0));
//! let bbox = BBox3d::from_range(range);
//!
//! assert!((bbox.volume() - 1.0).abs() < 1e-10);
//! ```

use crate::matrix4::Matrix4d;
use crate::range::Range3d;
use crate::vec3::Vec3d;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Precision limit for matrix degeneracy detection.
const PRECISION_LIMIT: f64 = 1.0e-13;

/// An arbitrarily oriented 3D bounding box.
///
/// Stores an axis-aligned box and a transformation matrix separately,
/// allowing tighter bounds when transformed multiple times.
///
/// # Examples
///
/// ```
/// use usd_gf::{BBox3d, Range3d, Matrix4d, vec3d};
///
/// let range = Range3d::new(vec3d(-1.0, -1.0, -1.0), vec3d(1.0, 1.0, 1.0));
/// let bbox = BBox3d::from_range(range);
///
/// // Volume of a 2x2x2 cube = 8
/// assert!((bbox.volume() - 8.0).abs() < 1e-10);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct BBox3d {
    /// Axis-aligned box in local space.
    range: Range3d,
    /// Transformation matrix.
    matrix: Matrix4d,
    /// Cached inverse of transformation matrix.
    inverse: Matrix4d,
    /// True if matrix is degenerate (non-invertible).
    is_degenerate: bool,
    /// True if bbox contains zero-area primitives (lines, points).
    has_zero_area_primitives: bool,
}

impl Default for BBox3d {
    /// Creates an empty bbox with identity transformation.
    fn default() -> Self {
        Self {
            range: Range3d::default(),
            matrix: Matrix4d::identity(),
            inverse: Matrix4d::identity(),
            is_degenerate: false,
            has_zero_area_primitives: false,
        }
    }
}

impl BBox3d {
    /// Creates an empty bbox with identity transformation.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a bbox from a range with identity transformation.
    #[must_use]
    pub fn from_range(range: Range3d) -> Self {
        Self {
            range,
            matrix: Matrix4d::identity(),
            inverse: Matrix4d::identity(),
            is_degenerate: false,
            has_zero_area_primitives: false,
        }
    }

    /// Creates a bbox from a range and transformation matrix.
    #[must_use]
    pub fn from_range_matrix(range: Range3d, matrix: Matrix4d) -> Self {
        let mut bbox = Self {
            range,
            matrix: Matrix4d::identity(),
            inverse: Matrix4d::identity(),
            is_degenerate: false,
            has_zero_area_primitives: false,
        };
        bbox.set_matrices(matrix);
        bbox
    }

    /// Sets the axis-aligned box and transformation matrix.
    pub fn set(&mut self, range: Range3d, matrix: Matrix4d) {
        self.range = range;
        self.set_matrices(matrix);
    }

    /// Sets only the transformation matrix.
    pub fn set_matrix(&mut self, matrix: Matrix4d) {
        self.set_matrices(matrix);
    }

    /// Sets only the range (axis-aligned box).
    #[inline]
    pub fn set_range(&mut self, range: Range3d) {
        self.range = range;
    }

    /// Returns the axis-aligned untransformed range.
    #[inline]
    #[must_use]
    pub fn range(&self) -> &Range3d {
        &self.range
    }

    /// Returns the axis-aligned untransformed range (alias for compatibility).
    #[inline]
    #[must_use]
    pub fn get_box(&self) -> &Range3d {
        &self.range
    }

    /// Returns the transformation matrix.
    #[inline]
    #[must_use]
    pub fn matrix(&self) -> &Matrix4d {
        &self.matrix
    }

    /// Returns the inverse transformation matrix.
    ///
    /// Returns identity if the matrix is degenerate.
    #[inline]
    #[must_use]
    pub fn inverse_matrix(&self) -> &Matrix4d {
        &self.inverse
    }

    /// Returns true if the transformation matrix is degenerate.
    #[inline]
    #[must_use]
    pub fn is_degenerate(&self) -> bool {
        self.is_degenerate
    }

    /// Sets the zero-area primitives flag.
    #[inline]
    pub fn set_has_zero_area_primitives(&mut self, has_them: bool) {
        self.has_zero_area_primitives = has_them;
    }

    /// Returns true if this bbox may contain zero-area primitives.
    #[inline]
    #[must_use]
    pub fn has_zero_area_primitives(&self) -> bool {
        self.has_zero_area_primitives
    }

    /// Returns the volume of the box (0 for empty box).
    ///
    /// Volume = |determinant(3x3)| * width * height * depth.
    #[must_use]
    pub fn volume(&self) -> f64 {
        if self.range.is_empty() {
            return 0.0;
        }

        // Volume of transformed box = untransformed volume * |det(3x3)|
        let size = self.range.size();
        (self.matrix.determinant3() * size.x * size.y * size.z).abs()
    }

    /// Transforms the bbox by post-multiplying the matrix.
    ///
    /// This applies a global transformation to the box.
    pub fn transform(&mut self, matrix: &Matrix4d) {
        let new_matrix = self.matrix * *matrix;
        self.set_matrices(new_matrix);
    }

    /// Computes the axis-aligned range after applying the transformation.
    ///
    /// Uses James Arvo's algorithm from Graphics Gems I, pp 548-550.
    #[must_use]
    pub fn compute_aligned_range(&self) -> Range3d {
        if self.range.is_empty() {
            return self.range;
        }

        // Start with translation (row 3 in row-major)
        let trans = Vec3d::new(self.matrix[3][0], self.matrix[3][1], self.matrix[3][2]);
        let mut aligned_min = trans;
        let mut aligned_max = trans;

        let min = *self.range.min();
        let max = *self.range.max();

        // Arvo's algorithm: accumulate min/max contributions from each matrix element
        for j in 0..3 {
            for i in 0..3 {
                let a = min[i] * self.matrix[i][j];
                let b = max[i] * self.matrix[i][j];
                if a < b {
                    aligned_min[j] += a;
                    aligned_max[j] += b;
                } else {
                    aligned_min[j] += b;
                    aligned_max[j] += a;
                }
            }
        }

        Range3d::new(aligned_min, aligned_max)
    }

    /// Computes the axis-aligned range (alias for compatibility).
    #[inline]
    #[must_use]
    pub fn compute_aligned_box(&self) -> Range3d {
        self.compute_aligned_range()
    }

    /// Computes the centroid of the bounding box.
    ///
    /// Returns the transformed center of the range.
    #[must_use]
    pub fn compute_centroid(&self) -> Vec3d {
        let center = (*self.range.min() + *self.range.max()) * 0.5;
        self.matrix.transform_point(&center)
    }

    /// Combines two bboxes, returning a new bbox containing both.
    ///
    /// Chooses the coordinate space that produces the smaller result.
    #[must_use]
    pub fn combine(b1: &BBox3d, b2: &BBox3d) -> BBox3d {
        let mut result;

        // If either box is empty, use the other as-is
        if b1.range.is_empty() {
            result = *b2;
        } else if b2.range.is_empty() {
            result = *b1;
        }
        // If both are degenerate, combine their projected boxes
        else if b1.is_degenerate {
            if b2.is_degenerate {
                let union =
                    Range3d::get_union(&b1.compute_aligned_range(), &b2.compute_aligned_range());
                result = BBox3d::from_range(union);
            } else {
                result = Self::combine_in_order(b2, b1);
            }
        } else if b2.is_degenerate {
            result = Self::combine_in_order(b1, b2);
        }
        // Non-degenerate: try both spaces, pick smaller volume
        else {
            let result1 = Self::combine_in_order(b1, b2);
            let result2 = Self::combine_in_order(b2, b1);

            let v1 = result1.volume();
            let v2 = result2.volume();
            let tolerance = (1e-10_f64).max(1e-6 * v1.max(v2).abs());

            // Use result1 if volumes are within tolerance OR v1 is smaller
            result = if v1 <= v2 + tolerance {
                result1
            } else {
                result2
            };
        }

        // Combine zero-area primitive flags
        result.has_zero_area_primitives =
            b1.has_zero_area_primitives || b2.has_zero_area_primitives;

        result
    }

    /// Sets the transformation matrix and its inverse.
    fn set_matrices(&mut self, matrix: Matrix4d) {
        self.is_degenerate = false;
        self.matrix = matrix;

        let det = matrix.determinant();
        if det.abs() <= PRECISION_LIMIT {
            self.is_degenerate = true;
            self.inverse = Matrix4d::identity();
        } else {
            // Safe: we checked determinant > PRECISION_LIMIT
            self.inverse = matrix.inverse().unwrap_or_else(Matrix4d::identity);
        }
    }

    /// Combines b2 into b1's space.
    fn combine_in_order(b1: &BBox3d, b2: &BBox3d) -> BBox3d {
        // Transform b2 into b1's space
        let mut b2t = BBox3d::new();
        b2t.range = b2.range;
        b2t.matrix = b2.matrix * b1.inverse;
        b2t.inverse = b1.matrix * b2.inverse;

        // Project b2t and extend b1's range
        let proj = b2t.compute_aligned_range();

        let mut result = *b1;
        result.range.union_with(&proj);
        result
    }
}

impl PartialEq for BBox3d {
    /// Component-wise equality of range and matrix.
    ///
    /// Note: To compare actual boxes, compute aligned ranges and compare those.
    fn eq(&self, other: &Self) -> bool {
        self.range == other.range && self.matrix == other.matrix
    }
}

impl Eq for BBox3d {}

impl Hash for BBox3d {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash range min/max coordinates
        self.range.min().x.to_bits().hash(state);
        self.range.min().y.to_bits().hash(state);
        self.range.min().z.to_bits().hash(state);
        self.range.max().x.to_bits().hash(state);
        self.range.max().y.to_bits().hash(state);
        self.range.max().z.to_bits().hash(state);
        // Hash matrix elements
        for i in 0..4 {
            for j in 0..4 {
                self.matrix[i][j].to_bits().hash(state);
            }
        }
    }
}

impl fmt::Display for BBox3d {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[({}) ({}) {}]",
            self.range,
            self.matrix,
            if self.has_zero_area_primitives {
                "true"
            } else {
                "false"
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3d;

    #[test]
    fn test_default() {
        let bbox = BBox3d::new();
        assert!(bbox.range().is_empty());
        assert!(!bbox.is_degenerate());
        assert!(!bbox.has_zero_area_primitives());
    }

    #[test]
    fn test_from_range() {
        let range = Range3d::new(vec3d(0.0, 0.0, 0.0), vec3d(1.0, 1.0, 1.0));
        let bbox = BBox3d::from_range(range);

        assert_eq!(bbox.range().min(), &vec3d(0.0, 0.0, 0.0));
        assert_eq!(bbox.range().max(), &vec3d(1.0, 1.0, 1.0));
        assert_eq!(bbox.matrix(), &Matrix4d::identity());
    }

    #[test]
    fn test_volume() {
        let range = Range3d::new(vec3d(0.0, 0.0, 0.0), vec3d(2.0, 3.0, 4.0));
        let bbox = BBox3d::from_range(range);

        // Volume = 2 * 3 * 4 = 24
        assert!((bbox.volume() - 24.0).abs() < 1e-10);
    }

    #[test]
    fn test_empty_volume() {
        let bbox = BBox3d::new();
        assert_eq!(bbox.volume(), 0.0);
    }

    #[test]
    fn test_compute_aligned_range_identity() {
        let range = Range3d::new(vec3d(-1.0, -2.0, -3.0), vec3d(1.0, 2.0, 3.0));
        let bbox = BBox3d::from_range(range);

        let aligned = bbox.compute_aligned_range();
        assert!((aligned.min().x - (-1.0)).abs() < 1e-10);
        assert!((aligned.max().x - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_compute_centroid() {
        let range = Range3d::new(vec3d(0.0, 0.0, 0.0), vec3d(2.0, 4.0, 6.0));
        let bbox = BBox3d::from_range(range);

        let centroid = bbox.compute_centroid();
        assert!((centroid.x - 1.0).abs() < 1e-10);
        assert!((centroid.y - 2.0).abs() < 1e-10);
        assert!((centroid.z - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_transform() {
        let range = Range3d::new(vec3d(0.0, 0.0, 0.0), vec3d(1.0, 1.0, 1.0));
        let mut bbox = BBox3d::from_range(range);

        // Apply scale by 2
        let scale = Matrix4d::from_scale(2.0);
        bbox.transform(&scale);

        // Volume should be 8x original (2^3)
        assert!((bbox.volume() - 8.0).abs() < 1e-10);
    }

    #[test]
    fn test_zero_area_primitives_flag() {
        let mut bbox = BBox3d::new();
        assert!(!bbox.has_zero_area_primitives());

        bbox.set_has_zero_area_primitives(true);
        assert!(bbox.has_zero_area_primitives());
    }

    #[test]
    fn test_combine_empty() {
        let range = Range3d::new(vec3d(0.0, 0.0, 0.0), vec3d(1.0, 1.0, 1.0));
        let b1 = BBox3d::from_range(range);
        let b2 = BBox3d::new(); // empty

        let result = BBox3d::combine(&b1, &b2);
        assert_eq!(result.range(), b1.range());
    }

    #[test]
    fn test_combine_both_valid() {
        let r1 = Range3d::new(vec3d(0.0, 0.0, 0.0), vec3d(1.0, 1.0, 1.0));
        let r2 = Range3d::new(vec3d(2.0, 2.0, 2.0), vec3d(3.0, 3.0, 3.0));

        let b1 = BBox3d::from_range(r1);
        let b2 = BBox3d::from_range(r2);

        let result = BBox3d::combine(&b1, &b2);
        let aligned = result.compute_aligned_range();

        // Result should contain both boxes
        assert!(aligned.min().x <= 0.0);
        assert!(aligned.max().x >= 3.0);
    }

    #[test]
    fn test_combine_zero_area_flag() {
        let r1 = Range3d::new(vec3d(0.0, 0.0, 0.0), vec3d(1.0, 1.0, 1.0));
        let r2 = Range3d::new(vec3d(2.0, 2.0, 2.0), vec3d(3.0, 3.0, 3.0));

        let mut b1 = BBox3d::from_range(r1);
        let b2 = BBox3d::from_range(r2);

        b1.set_has_zero_area_primitives(true);

        let result = BBox3d::combine(&b1, &b2);
        assert!(result.has_zero_area_primitives());
    }

    #[test]
    fn test_equality() {
        let range = Range3d::new(vec3d(0.0, 0.0, 0.0), vec3d(1.0, 1.0, 1.0));
        let b1 = BBox3d::from_range(range);
        let b2 = BBox3d::from_range(range);

        assert_eq!(b1, b2);
    }

    #[test]
    fn test_display() {
        let range = Range3d::new(vec3d(0.0, 0.0, 0.0), vec3d(1.0, 1.0, 1.0));
        let bbox = BBox3d::from_range(range);

        let s = format!("{}", bbox);
        assert!(s.contains("false")); // has_zero_area_primitives = false
    }
}
