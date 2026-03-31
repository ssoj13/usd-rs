//! Point cloud compression method enum.
//! Reference: `_ref/draco/src/draco/compression/point_cloud/algorithms/point_cloud_compression_method.h`.

/// Enum indicating the used compression method.
#[repr(i8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointCloudCompressionMethod {
    ReservedPointCloudMethod0 = 0,
    /// Generalized kD-tree/octree encoding (Devillers & Gandoin).
    KdTree = 1,
    ReservedPointCloudMethod2 = 2,
    ReservedPointCloudMethod3 = 3,
}
