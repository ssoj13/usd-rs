//! Point cloud type aliases and traits.
//! Reference: `_ref/draco/src/draco/compression/point_cloud/algorithms/point_cloud_types.h`.
//!
//! Provides VectorD-based point aliases and small traits used by kd-tree
//! point cloud algorithms.

use std::marker::PhantomData;

use draco_core::core::vector_d::{
    Vector3f, Vector3ui, Vector4f, Vector4ui, Vector5ui, Vector6ui, Vector7ui, VectorD,
};

pub type Point3f = Vector3f;
pub type Point4f = Vector4f;
pub type Point3ui = Vector3ui;
pub type Point4ui = Vector4ui;
pub type Point5ui = Vector5ui;
pub type Point6ui = Vector6ui;
pub type Point7ui = Vector7ui;

pub type PointCloud3f = Vec<Point3f>;

/// Lexicographic comparison helper.
pub struct PointDLess<PointT>(PhantomData<PointT>);

impl<PointT: PartialOrd> PointDLess<PointT> {
    pub fn less(a: &PointT, b: &PointT) -> bool {
        a < b
    }
}

/// Point traits used by integer kd-tree algorithms.
pub trait PointTraits {
    type Point;
    type Coordinate;
    const DIMENSION: usize;

    fn origin() -> Self::Point;
    fn zero_levels() -> Vec<u32>;
}

impl<Coordinate: Copy + Default, const N: usize> PointTraits for VectorD<Coordinate, N> {
    type Point = VectorD<Coordinate, N>;
    type Coordinate = Coordinate;
    const DIMENSION: usize = N;

    fn origin() -> Self::Point {
        VectorD::default()
    }

    fn zero_levels() -> Vec<u32> {
        vec![0u32; N]
    }
}
