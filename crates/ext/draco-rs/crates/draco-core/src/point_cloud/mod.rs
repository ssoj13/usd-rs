//! Rust port of Draco point_cloud module.
//! Reference: `_ref/draco/src/draco/point_cloud`.

pub mod point_cloud;
pub mod point_cloud_builder;

pub(crate) use point_cloud::build_point_deduplication_map;
pub use point_cloud::{PointCloud, PointCloudHasher};
pub use point_cloud_builder::PointCloudBuilder;

#[cfg(test)]
mod tests;
