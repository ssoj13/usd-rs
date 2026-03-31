//! Symbol bit decoder.
//! Reference: `_ref/draco/src/draco/compression/bit_coders/symbol_bit_decoder.h|cc`.
//!
//! Decodes symbols encoded by SymbolBitEncoder.

use crate::compression::bit_coders::BitDecoder;
use crate::compression::entropy::symbol_decoding::decode_symbols;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::{draco_dcheck, draco_dcheck_gt, draco_dcheck_le, draco_dcheck_ne};

pub struct SymbolBitDecoder {
    symbols: Vec<u32>,
}

impl SymbolBitDecoder {
    pub fn new() -> Self {
        Self {
            symbols: Vec::new(),
        }
    }

    pub fn start_decoding(&mut self, source_buffer: &mut DecoderBuffer) -> bool {
        let mut size: u32 = 0;
        if !source_buffer.decode(&mut size) {
            return false;
        }
        self.symbols.resize(size as usize, 0);
        if !decode_symbols(size, 1, source_buffer, &mut self.symbols) {
            return false;
        }
        self.symbols.reverse();
        true
    }

    pub fn decode_next_bit(&mut self) -> bool {
        let mut symbol = 0u32;
        self.decode_least_significant_bits32(1, &mut symbol);
        draco_dcheck!(symbol == 0 || symbol == 1);
        symbol == 1
    }

    pub fn decode_least_significant_bits32(&mut self, nbits: i32, value: &mut u32) {
        draco_dcheck_le!(1, nbits);
        draco_dcheck_le!(nbits, 32);
        draco_dcheck_ne!(value as *const u32, std::ptr::null());
        draco_dcheck_gt!(self.symbols.len() as i32, 0);
        *value = *self.symbols.last().unwrap();
        self.symbols.pop();
        let discarded_bits = 32 - nbits;
        *value <<= discarded_bits;
        *value >>= discarded_bits;
    }

    pub fn clear(&mut self) {
        self.symbols.clear();
        self.symbols.shrink_to_fit();
    }
}

impl Default for SymbolBitDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl BitDecoder for SymbolBitDecoder {
    fn start_decoding(&mut self, source_buffer: &mut DecoderBuffer) -> bool {
        SymbolBitDecoder::start_decoding(self, source_buffer)
    }

    fn decode_next_bit(&mut self) -> bool {
        SymbolBitDecoder::decode_next_bit(self)
    }

    fn decode_least_significant_bits32(&mut self, nbits: i32, value: &mut u32) {
        SymbolBitDecoder::decode_least_significant_bits32(self, nbits, value);
    }

    fn end_decoding(&mut self) {
        // No-op for symbol decoder.
    }

    fn clear(&mut self) {
        SymbolBitDecoder::clear(self);
    }
}
