//! Rust port of Draco attributes module.
//! Reference: `_ref/draco/src/draco/attributes`.

pub mod attribute_octahedron_transform;
pub mod attribute_quantization_transform;
pub mod attribute_transform;
pub mod attribute_transform_data;
pub mod attribute_transform_type;
pub mod draco_numeric;

pub mod geometry_attribute;
pub mod geometry_indices;
pub mod point_attribute;

#[cfg(test)]
mod tests;
