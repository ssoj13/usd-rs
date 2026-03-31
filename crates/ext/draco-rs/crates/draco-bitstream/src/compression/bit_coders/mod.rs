//! Draco compression: bit_coders submodule.
//! Reference: `_ref/draco/src/draco/compression/bit_coders`.
//!
//! These coders provide bit-level encoding/decoding with entropy coding
//! backends (direct, rANS, adaptive rANS, symbol-based), used throughout
//! compression algorithms.

use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::encoder_buffer::EncoderBuffer;

/// Common bit-encoder interface used by folded coders.
pub trait BitEncoder {
    fn start_encoding(&mut self);
    fn encode_bit(&mut self, bit: bool);
    fn encode_least_significant_bits32(&mut self, nbits: i32, value: u32);
    fn end_encoding(&mut self, target_buffer: &mut EncoderBuffer);
    fn clear(&mut self);
}

/// Common bit-decoder interface used by folded coders.
pub trait BitDecoder {
    fn start_decoding(&mut self, source_buffer: &mut DecoderBuffer) -> bool;
    fn decode_next_bit(&mut self) -> bool;
    fn decode_least_significant_bits32(&mut self, nbits: i32, value: &mut u32);
    fn end_decoding(&mut self);
    fn clear(&mut self);
}

pub mod adaptive_rans_bit_coding_shared;
pub mod adaptive_rans_bit_decoder;
pub mod adaptive_rans_bit_encoder;
pub mod direct_bit_decoder;
pub mod direct_bit_encoder;
pub mod folded_integer_bit_decoder;
pub mod folded_integer_bit_encoder;
pub mod rans_bit_decoder;
pub mod rans_bit_encoder;
pub mod symbol_bit_decoder;
pub mod symbol_bit_encoder;
