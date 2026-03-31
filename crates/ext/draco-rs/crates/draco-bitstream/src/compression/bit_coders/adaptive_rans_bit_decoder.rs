//! Adaptive rANS bit decoder.
//! Reference: `_ref/draco/src/draco/compression/bit_coders/adaptive_rans_bit_decoder.h|cc`.
//!
//! Decodes bits encoded by AdaptiveRAnsBitEncoder.

use crate::compression::bit_coders::adaptive_rans_bit_coding_shared::{
    clamp_probability, update_probability,
};
use crate::compression::bit_coders::BitDecoder;
use crate::compression::entropy::ans::{ans_read_end, ans_read_init, rabs_read, AnsDecoder};
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::draco_dcheck;

pub struct AdaptiveRAnsBitDecoder {
    ans_decoder: AnsDecoder,
    p0_f: f64,
}

impl AdaptiveRAnsBitDecoder {
    pub fn new() -> Self {
        Self {
            ans_decoder: AnsDecoder::new(),
            p0_f: 0.5,
        }
    }

    pub fn start_decoding(&mut self, source_buffer: &mut DecoderBuffer) -> bool {
        self.clear();
        let mut size_in_bytes: u32 = 0;
        if !source_buffer.decode(&mut size_in_bytes) {
            return false;
        }
        if size_in_bytes as i64 > source_buffer.remaining_size() {
            return false;
        }
        let data_head = source_buffer.data_head().as_ptr();
        if ans_read_init(&mut self.ans_decoder, data_head, size_in_bytes as i32) != 0 {
            return false;
        }
        source_buffer.advance(size_in_bytes as i64);
        true
    }

    pub fn decode_next_bit(&mut self) -> bool {
        let p0 = clamp_probability(self.p0_f);
        let bit = rabs_read(&mut self.ans_decoder, p0) != 0;
        self.p0_f = update_probability(self.p0_f, bit);
        bit
    }

    pub fn decode_least_significant_bits32(&mut self, nbits: i32, value: &mut u32) {
        draco_dcheck!(nbits <= 32 && nbits > 0);
        let mut result = 0u32;
        let mut remaining = nbits;
        while remaining > 0 {
            result = (result << 1) + if self.decode_next_bit() { 1 } else { 0 };
            remaining -= 1;
        }
        *value = result;
    }

    pub fn end_decoding(&mut self) {
        ans_read_end(&mut self.ans_decoder);
    }

    fn clear(&mut self) {
        ans_read_end(&mut self.ans_decoder);
        self.p0_f = 0.5;
    }
}

impl Default for AdaptiveRAnsBitDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl BitDecoder for AdaptiveRAnsBitDecoder {
    fn start_decoding(&mut self, source_buffer: &mut DecoderBuffer) -> bool {
        AdaptiveRAnsBitDecoder::start_decoding(self, source_buffer)
    }

    fn decode_next_bit(&mut self) -> bool {
        AdaptiveRAnsBitDecoder::decode_next_bit(self)
    }

    fn decode_least_significant_bits32(&mut self, nbits: i32, value: &mut u32) {
        AdaptiveRAnsBitDecoder::decode_least_significant_bits32(self, nbits, value);
    }

    fn end_decoding(&mut self) {
        AdaptiveRAnsBitDecoder::end_decoding(self);
    }

    fn clear(&mut self) {
        AdaptiveRAnsBitDecoder::clear(self);
    }
}
