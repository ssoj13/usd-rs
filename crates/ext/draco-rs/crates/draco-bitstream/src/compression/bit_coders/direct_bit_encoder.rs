//! Direct bit encoder.
//! Reference: `_ref/draco/src/draco/compression/bit_coders/direct_bit_encoder.h|cc`.
//!
//! Encodes bits directly into 32-bit words with a simple header.

use crate::compression::bit_coders::BitEncoder;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::draco_dcheck_eq;

pub struct DirectBitEncoder {
    bits: Vec<u32>,
    local_bits: u32,
    num_local_bits: u32,
}

impl DirectBitEncoder {
    pub fn new() -> Self {
        Self {
            bits: Vec::new(),
            local_bits: 0,
            num_local_bits: 0,
        }
    }

    pub fn start_encoding(&mut self) {
        self.clear();
    }

    pub fn encode_bit(&mut self, bit: bool) {
        if bit {
            self.local_bits |= 1 << (31 - self.num_local_bits);
        }
        self.num_local_bits += 1;
        if self.num_local_bits == 32 {
            self.bits.push(self.local_bits);
            self.num_local_bits = 0;
            self.local_bits = 0;
        }
    }

    pub fn encode_least_significant_bits32(&mut self, nbits: i32, mut value: u32) {
        draco_dcheck_eq!(true, nbits <= 32);
        draco_dcheck_eq!(true, nbits > 0);

        let remaining = 32 - self.num_local_bits as i32;
        value = value << (32 - nbits);
        if nbits <= remaining {
            value = value >> self.num_local_bits;
            self.local_bits |= value;
            self.num_local_bits += nbits as u32;
            if self.num_local_bits == 32 {
                self.bits.push(self.local_bits);
                self.local_bits = 0;
                self.num_local_bits = 0;
            }
        } else {
            value = value >> (32 - nbits);
            self.num_local_bits = (nbits - remaining) as u32;
            let value_l = value >> self.num_local_bits;
            self.local_bits |= value_l;
            self.bits.push(self.local_bits);
            self.local_bits = value << (32 - self.num_local_bits);
        }
    }

    pub fn end_encoding(&mut self, target_buffer: &mut EncoderBuffer) {
        self.bits.push(self.local_bits);
        let size_in_bytes = (self.bits.len() * 4) as u32;
        target_buffer.encode(size_in_bytes);
        let mut bytes = Vec::with_capacity(self.bits.len() * 4);
        for word in &self.bits {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        target_buffer.encode_bytes(&bytes);
        self.clear();
    }

    pub fn clear(&mut self) {
        self.bits.clear();
        self.local_bits = 0;
        self.num_local_bits = 0;
    }
}

impl Default for DirectBitEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl BitEncoder for DirectBitEncoder {
    fn start_encoding(&mut self) {
        DirectBitEncoder::start_encoding(self);
    }

    fn encode_bit(&mut self, bit: bool) {
        DirectBitEncoder::encode_bit(self, bit);
    }

    fn encode_least_significant_bits32(&mut self, nbits: i32, value: u32) {
        DirectBitEncoder::encode_least_significant_bits32(self, nbits, value);
    }

    fn end_encoding(&mut self, target_buffer: &mut EncoderBuffer) {
        DirectBitEncoder::end_encoding(self, target_buffer);
    }

    fn clear(&mut self) {
        DirectBitEncoder::clear(self);
    }
}
