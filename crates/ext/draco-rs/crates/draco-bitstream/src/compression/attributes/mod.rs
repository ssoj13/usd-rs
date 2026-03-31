//! Draco compression: attributes submodule.
//! Reference: `_ref/draco/src/draco/compression/attributes`.
//!
//! Bitstream-specific attribute encoders/decoders live here. Shared normal
//! compression utilities are re-exported from draco-core.

pub mod attributes_decoder;
pub mod attributes_decoder_interface;
pub mod attributes_encoder;
pub mod kd_tree_attributes_decoder;
pub mod kd_tree_attributes_encoder;
pub mod kd_tree_attributes_shared;
pub mod linear_sequencer;
pub mod mesh_attribute_indices_encoding_data;
pub mod point_d_vector;
pub mod points_sequencer;
pub mod prediction_schemes;
pub mod sequential_attribute_decoder;
pub mod sequential_attribute_decoders_controller;
pub mod sequential_attribute_encoder;
pub mod sequential_attribute_encoders_controller;
pub mod sequential_integer_attribute_decoder;
pub mod sequential_integer_attribute_encoder;
pub mod sequential_normal_attribute_decoder;
pub mod sequential_normal_attribute_encoder;
pub mod sequential_quantization_attribute_decoder;
pub mod sequential_quantization_attribute_encoder;

pub mod normal_compression_utils {
    pub use draco_core::compression::attributes::normal_compression_utils::*;
}
