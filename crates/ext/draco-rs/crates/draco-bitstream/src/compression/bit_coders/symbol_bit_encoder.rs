//! Symbol bit encoder.
//! Reference: `_ref/draco/src/draco/compression/bit_coders/symbol_bit_encoder.h|cc`.
//!
//! Encodes bits as symbols using symbol entropy coding.

use crate::compression::bit_coders::BitEncoder;
use crate::compression::entropy::symbol_encoding::encode_symbols;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::draco_dcheck_le;

pub struct SymbolBitEncoder {
    symbols: Vec<u32>,
}

impl SymbolBitEncoder {
    pub fn new() -> Self {
        Self {
            symbols: Vec::new(),
        }
    }

    pub fn start_encoding(&mut self) {
        self.clear();
    }

    pub fn encode_bit(&mut self, bit: bool) {
        self.encode_least_significant_bits32(1, if bit { 1 } else { 0 });
    }

    pub fn encode_least_significant_bits32(&mut self, nbits: i32, mut value: u32) {
        draco_dcheck_le!(1, nbits);
        draco_dcheck_le!(nbits, 32);
        let discarded_bits = 32 - nbits;
        value <<= discarded_bits;
        value >>= discarded_bits;
        self.symbols.push(value);
    }

    pub fn end_encoding(&mut self, target_buffer: &mut EncoderBuffer) {
        target_buffer.encode(self.symbols.len() as u32);
        encode_symbols(
            &self.symbols,
            self.symbols.len() as i32,
            1,
            None,
            target_buffer,
        );
        self.clear();
    }

    pub fn clear(&mut self) {
        self.symbols.clear();
        self.symbols.shrink_to_fit();
    }
}

impl Default for SymbolBitEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl BitEncoder for SymbolBitEncoder {
    fn start_encoding(&mut self) {
        SymbolBitEncoder::start_encoding(self);
    }

    fn encode_bit(&mut self, bit: bool) {
        SymbolBitEncoder::encode_bit(self, bit);
    }

    fn encode_least_significant_bits32(&mut self, nbits: i32, value: u32) {
        SymbolBitEncoder::encode_least_significant_bits32(self, nbits, value);
    }

    fn end_encoding(&mut self, target_buffer: &mut EncoderBuffer) {
        SymbolBitEncoder::end_encoding(self, target_buffer);
    }

    fn clear(&mut self) {
        SymbolBitEncoder::clear(self);
    }
}
