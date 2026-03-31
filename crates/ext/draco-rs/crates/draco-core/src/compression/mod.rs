//! Compression utilities required by core geometry types.
//!
//! This module currently hosts `DracoCompressionOptions` and the normal
//! compression helpers used by attribute transforms. Bitstream encode/decode
//! lives in a separate crate.

pub mod attributes;
pub mod draco_compression_options;

pub use draco_compression_options::{
    DracoCompressionOptions, SpatialQuantizationMode, SpatialQuantizationOptions,
};
