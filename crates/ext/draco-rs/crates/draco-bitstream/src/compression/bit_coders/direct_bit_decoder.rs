//! Direct bit decoder.
//! Reference: `_ref/draco/src/draco/compression/bit_coders/direct_bit_decoder.h|cc`.
//!
//! Decodes bits encoded by DirectBitEncoder.

use crate::compression::bit_coders::BitDecoder;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::draco_dcheck_eq;

pub struct DirectBitDecoder {
    bits: Vec<u32>,
    pos: usize,
    num_used_bits: u32,
}

impl DirectBitDecoder {
    pub fn new() -> Self {
        Self {
            bits: Vec::new(),
            pos: 0,
            num_used_bits: 0,
        }
    }

    pub fn start_decoding(&mut self, source_buffer: &mut DecoderBuffer) -> bool {
        self.clear();
        let mut size_in_bytes: u32 = 0;
        if !source_buffer.decode(&mut size_in_bytes) {
            return false;
        }
        if size_in_bytes == 0 || (size_in_bytes & 0x3) != 0 {
            return false;
        }
        if size_in_bytes as i64 > source_buffer.remaining_size() {
            return false;
        }
        let num_32bit_elements = size_in_bytes / 4;
        let mut bytes = vec![0u8; size_in_bytes as usize];
        if !source_buffer.decode_bytes(&mut bytes) {
            return false;
        }
        self.bits.clear();
        self.bits.reserve(num_32bit_elements as usize);
        for chunk in bytes.chunks_exact(4) {
            let word = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            self.bits.push(word);
        }
        self.pos = 0;
        self.num_used_bits = 0;
        true
    }

    pub fn decode_next_bit(&mut self) -> bool {
        let selector = 1u32 << (31 - self.num_used_bits);
        if self.pos >= self.bits.len() {
            return false;
        }
        let bit = (self.bits[self.pos] & selector) != 0;
        self.num_used_bits += 1;
        if self.num_used_bits == 32 {
            self.pos += 1;
            self.num_used_bits = 0;
        }
        bit
    }

    pub fn decode_least_significant_bits32(&mut self, nbits: i32, value: &mut u32) -> bool {
        draco_dcheck_eq!(true, nbits <= 32);
        draco_dcheck_eq!(true, nbits > 0);
        let remaining = 32 - self.num_used_bits as i32;
        if nbits <= remaining {
            if self.pos >= self.bits.len() {
                return false;
            }
            let v = (self.bits[self.pos] << self.num_used_bits) >> (32 - nbits);
            *value = v;
            self.num_used_bits += nbits as u32;
            if self.num_used_bits == 32 {
                self.pos += 1;
                self.num_used_bits = 0;
            }
        } else {
            if self.pos >= self.bits.len() {
                return false;
            }
            let mut result = self.bits[self.pos] << self.num_used_bits;
            self.pos += 1;
            if self.pos >= self.bits.len() {
                return false;
            }
            result |= self.bits[self.pos] >> (32 - self.num_used_bits);
            result >>= 32 - nbits;
            self.num_used_bits = (nbits - remaining) as u32;
            *value = result;
        }
        true
    }

    pub fn clear(&mut self) {
        self.bits.clear();
        self.num_used_bits = 0;
        self.pos = self.bits.len();
    }
}

impl Default for DirectBitDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl BitDecoder for DirectBitDecoder {
    fn start_decoding(&mut self, source_buffer: &mut DecoderBuffer) -> bool {
        DirectBitDecoder::start_decoding(self, source_buffer)
    }

    fn decode_next_bit(&mut self) -> bool {
        DirectBitDecoder::decode_next_bit(self)
    }

    fn decode_least_significant_bits32(&mut self, nbits: i32, value: &mut u32) {
        let _ = DirectBitDecoder::decode_least_significant_bits32(self, nbits, value);
    }

    fn end_decoding(&mut self) {
        // Direct decoder has no explicit end state.
    }

    fn clear(&mut self) {
        DirectBitDecoder::clear(self);
    }
}
