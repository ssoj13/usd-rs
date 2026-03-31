//! Point cloud compression algorithms (kD-tree, quantization, and helpers).
//! Reference: `_ref/draco/src/draco/compression/point_cloud/algorithms`.
//!
//! This module hosts the low-level integer/float point cloud coders used by
//! kD-tree attribute and point-cloud encoders/decoders.

pub mod dynamic_integer_points_kd_tree_decoder;
pub mod dynamic_integer_points_kd_tree_encoder;
pub mod float_points_tree_decoder;
pub mod float_points_tree_encoder;
pub mod integer_points_kd_tree_decoder;
pub mod integer_points_kd_tree_encoder;
pub mod point_cloud_compression_method;
pub mod point_cloud_types;
pub mod quantize_points_3;
pub mod queuing_policy;
