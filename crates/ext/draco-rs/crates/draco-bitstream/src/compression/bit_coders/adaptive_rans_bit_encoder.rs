//! Adaptive rANS bit encoder.
//! Reference: `_ref/draco/src/draco/compression/bit_coders/adaptive_rans_bit_encoder.h|cc`.
//!
//! Encodes bits with adaptive probabilities using rANS primitives.

use crate::compression::bit_coders::adaptive_rans_bit_coding_shared::{
    clamp_probability, update_probability,
};
use crate::compression::bit_coders::BitEncoder;
use crate::compression::entropy::ans::{ans_write_end, ans_write_init, rabs_write, AnsCoder};
use draco_core::core::encoder_buffer::EncoderBuffer;

pub struct AdaptiveRAnsBitEncoder {
    bits: Vec<bool>,
}

impl AdaptiveRAnsBitEncoder {
    pub fn new() -> Self {
        Self { bits: Vec::new() }
    }

    pub fn start_encoding(&mut self) {
        self.bits.clear();
    }

    pub fn encode_bit(&mut self, bit: bool) {
        self.bits.push(bit);
    }

    pub fn encode_least_significant_bits32(&mut self, nbits: i32, value: u32) {
        debug_assert!(nbits > 0 && nbits <= 32);
        let mut selector = 1u32 << (nbits - 1);
        while selector != 0 {
            self.encode_bit((value & selector) != 0);
            selector >>= 1;
        }
    }

    pub fn end_encoding(&mut self, target_buffer: &mut EncoderBuffer) {
        let mut buffer = vec![0u8; self.bits.len() + 16];
        let mut ans_coder = AnsCoder::new();
        ans_write_init(&mut ans_coder, buffer.as_mut_ptr());

        let mut p0_f = 0.5f64;
        let mut p0s: Vec<u8> = Vec::with_capacity(self.bits.len());
        for &b in &self.bits {
            p0s.push(clamp_probability(p0_f));
            p0_f = update_probability(p0_f, b);
        }
        let mut bit_iter = self.bits.iter().rev();
        let mut pit = p0s.iter().rev();
        while let (Some(&bit), Some(&p0)) = (bit_iter.next(), pit.next()) {
            rabs_write(&mut ans_coder, if bit { 1 } else { 0 }, p0);
        }

        let size_in_bytes = ans_write_end(&mut ans_coder) as u32;
        target_buffer.encode(size_in_bytes);
        target_buffer.encode_bytes(&buffer[..size_in_bytes as usize]);
        self.bits.clear();
    }
}

impl Default for AdaptiveRAnsBitEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl BitEncoder for AdaptiveRAnsBitEncoder {
    fn start_encoding(&mut self) {
        AdaptiveRAnsBitEncoder::start_encoding(self);
    }

    fn encode_bit(&mut self, bit: bool) {
        AdaptiveRAnsBitEncoder::encode_bit(self, bit);
    }

    fn encode_least_significant_bits32(&mut self, nbits: i32, value: u32) {
        AdaptiveRAnsBitEncoder::encode_least_significant_bits32(self, nbits, value);
    }

    fn end_encoding(&mut self, target_buffer: &mut EncoderBuffer) {
        AdaptiveRAnsBitEncoder::end_encoding(self, target_buffer);
    }

    fn clear(&mut self) {
        self.bits.clear();
    }
}
