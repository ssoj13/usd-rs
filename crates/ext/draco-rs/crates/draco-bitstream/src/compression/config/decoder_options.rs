//! Decoder options.
//! Reference: `_ref/draco/src/draco/compression/config/decoder_options.h`.

use draco_core::attributes::geometry_attribute::GeometryAttributeType;

use crate::compression::config::draco_options::DracoOptions;

/// Options controlling decoding, keyed by geometry attribute type.
pub type DecoderOptions = DracoOptions<GeometryAttributeType>;
