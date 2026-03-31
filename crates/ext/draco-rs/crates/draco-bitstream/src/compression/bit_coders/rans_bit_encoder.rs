//! rANS bit encoder.
//! Reference: `_ref/draco/src/draco/compression/bit_coders/rans_bit_encoder.h|cc`.
//!
//! Encodes bits using a fixed probability table derived from counts.

use crate::compression::bit_coders::BitEncoder;
use crate::compression::entropy::ans::{ans_write_end, ans_write_init, rabs_write, AnsCoder};
use draco_core::core::bit_utils::{copy_bits32, count_one_bits32, reverse_bits32};
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::core::varint_encoding::encode_varint;
use draco_core::draco_dcheck_eq;

pub struct RAnsBitEncoder {
    bit_counts: [u64; 2],
    bits: Vec<u32>,
    local_bits: u32,
    num_local_bits: u32,
}

impl RAnsBitEncoder {
    pub fn new() -> Self {
        Self {
            bit_counts: [0, 0],
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
            self.bit_counts[1] += 1;
            self.local_bits |= 1 << self.num_local_bits;
        } else {
            self.bit_counts[0] += 1;
        }
        self.num_local_bits += 1;
        if self.num_local_bits == 32 {
            self.bits.push(self.local_bits);
            self.num_local_bits = 0;
            self.local_bits = 0;
        }
    }

    pub fn encode_least_significant_bits32(&mut self, nbits: i32, value: u32) {
        draco_dcheck_eq!(true, nbits <= 32);
        draco_dcheck_eq!(true, nbits > 0);

        let reversed = reverse_bits32(value) >> (32 - nbits);
        let ones = count_one_bits32(reversed) as u64;
        self.bit_counts[0] += (nbits as u64) - ones;
        self.bit_counts[1] += ones;

        let remaining = 32 - self.num_local_bits as i32;
        if nbits <= remaining {
            copy_bits32(
                &mut self.local_bits,
                self.num_local_bits as i32,
                reversed,
                0,
                nbits,
            );
            self.num_local_bits += nbits as u32;
            if self.num_local_bits == 32 {
                self.bits.push(self.local_bits);
                self.local_bits = 0;
                self.num_local_bits = 0;
            }
        } else {
            copy_bits32(
                &mut self.local_bits,
                self.num_local_bits as i32,
                reversed,
                0,
                remaining,
            );
            self.bits.push(self.local_bits);
            self.local_bits = 0;
            copy_bits32(
                &mut self.local_bits,
                0,
                reversed,
                remaining,
                nbits - remaining,
            );
            self.num_local_bits = (nbits - remaining) as u32;
        }
    }

    pub fn end_encoding(&mut self, target_buffer: &mut EncoderBuffer) {
        let mut total = self.bit_counts[1] + self.bit_counts[0];
        if total == 0 {
            total += 1;
        }
        let zero_prob_raw = ((self.bit_counts[0] as f64 / total as f64) * 256.0 + 0.5) as u32;
        let mut zero_prob = if zero_prob_raw < 255 {
            zero_prob_raw as u8
        } else {
            255u8
        };
        if zero_prob == 0 {
            zero_prob += 1;
        }

        let mut buffer = vec![0u8; (self.bits.len() + 8) * 8];
        let mut ans_coder = AnsCoder::new();
        ans_write_init(&mut ans_coder, buffer.as_mut_ptr());

        for i in (0..self.num_local_bits).rev() {
            let bit = ((self.local_bits >> i) & 1) as i32;
            rabs_write(&mut ans_coder, bit, zero_prob);
        }
        for &bits in self.bits.iter().rev() {
            for i in (0..32).rev() {
                let bit = ((bits >> i) & 1) as i32;
                rabs_write(&mut ans_coder, bit, zero_prob);
            }
        }

        let size_in_bytes = ans_write_end(&mut ans_coder) as u32;
        target_buffer.encode(zero_prob);
        encode_varint(size_in_bytes, target_buffer);
        target_buffer.encode_bytes(&buffer[..size_in_bytes as usize]);
        self.clear();
    }

    pub fn clear(&mut self) {
        self.bit_counts = [0, 0];
        self.bits.clear();
        self.local_bits = 0;
        self.num_local_bits = 0;
    }
}

impl Default for RAnsBitEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl BitEncoder for RAnsBitEncoder {
    fn start_encoding(&mut self) {
        RAnsBitEncoder::start_encoding(self);
    }

    fn encode_bit(&mut self, bit: bool) {
        RAnsBitEncoder::encode_bit(self, bit);
    }

    fn encode_least_significant_bits32(&mut self, nbits: i32, value: u32) {
        RAnsBitEncoder::encode_least_significant_bits32(self, nbits, value);
    }

    fn end_encoding(&mut self, target_buffer: &mut EncoderBuffer) {
        RAnsBitEncoder::end_encoding(self, target_buffer);
    }

    fn clear(&mut self) {
        RAnsBitEncoder::clear(self);
    }
}
