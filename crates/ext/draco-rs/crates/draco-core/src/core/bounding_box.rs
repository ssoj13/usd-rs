//! Bounding box utilities.
//! Reference: `_ref/draco/src/draco/core/bounding_box.h` + `.cc`.

use crate::core::vector_d::Vector3f;

/// Bounding box for points in 3D space.
#[derive(Clone, Copy, Debug)]
pub struct BoundingBox {
    min_point: Vector3f,
    max_point: Vector3f,
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self::new(
            Vector3f::new3(f32::MAX, f32::MAX, f32::MAX),
            Vector3f::new3(-f32::MAX, -f32::MAX, -f32::MAX),
        )
    }
}

impl BoundingBox {
    /// Creates a bounding box with provided min and max points.
    pub fn new(min_point: Vector3f, max_point: Vector3f) -> Self {
        Self {
            min_point,
            max_point,
        }
    }

    /// Returns the minimum point of the bounding box.
    pub fn min_point(&self) -> &Vector3f {
        &self.min_point
    }

    /// Returns the maximum point of the bounding box.
    pub fn max_point(&self) -> &Vector3f {
        &self.max_point
    }

    /// Returns true if the bounding box has been updated with any points.
    pub fn is_valid(&self) -> bool {
        self.min_point[0] != f32::MAX
            && self.min_point[1] != f32::MAX
            && self.min_point[2] != f32::MAX
            && self.max_point[0] != -f32::MAX
            && self.max_point[1] != -f32::MAX
            && self.max_point[2] != -f32::MAX
    }

    /// Conditionally updates the bounding box with a new point.
    pub fn update_point(&mut self, new_point: &Vector3f) {
        for i in 0..3 {
            if new_point[i] < self.min_point[i] {
                self.min_point[i] = new_point[i];
            }
            if new_point[i] > self.max_point[i] {
                self.max_point[i] = new_point[i];
            }
        }
    }

    /// Updates the bounding box with another bounding box.
    pub fn update_box(&mut self, other: &BoundingBox) {
        self.update_point(other.min_point());
        self.update_point(other.max_point());
    }

    /// Returns the size of the bounding box along each axis.
    pub fn size(&self) -> Vector3f {
        *self.max_point() - *self.min_point()
    }

    /// Returns the center of the bounding box.
    pub fn center(&self) -> Vector3f {
        (*self.min_point() + *self.max_point()) / 2.0
    }
}
