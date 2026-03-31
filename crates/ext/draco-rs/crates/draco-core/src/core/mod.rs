//! Rust port of Draco core module.
//! Reference: `_ref/draco/src/draco/core`.

pub mod bit_utils;
pub mod bounding_box;
pub mod constants;
pub mod cycle_timer;
pub mod data_buffer;
pub mod decoder_buffer;

pub mod divide;
pub mod draco_index_type;
pub mod draco_index_type_vector;
#[cfg(test)]
pub mod draco_test_base;
#[cfg(test)]
pub mod draco_test_utils;
pub mod draco_types;
pub mod draco_version;
pub mod encoder_buffer;
pub mod hash_utils;
pub mod macros;
pub mod math_utils;
pub mod options;
pub mod quantization_utils;
pub mod status;
pub mod status_or;
pub mod varint_decoding;
pub mod varint_encoding;
pub mod vector_d;

#[cfg(test)]
mod tests;
