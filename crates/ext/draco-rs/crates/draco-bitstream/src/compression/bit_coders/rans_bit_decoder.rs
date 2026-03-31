//! rANS bit decoder.
//! Reference: `_ref/draco/src/draco/compression/bit_coders/rans_bit_decoder.h|cc`.
//!
//! Decodes bits encoded by RAnsBitEncoder.

use crate::compression::bit_coders::BitDecoder;
use crate::compression::config::compression_shared::bitstream_version;
use crate::compression::entropy::ans::{ans_read_end, ans_read_init, rabs_read, AnsDecoder};
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::varint_decoding::decode_varint;
use draco_core::draco_dcheck_eq;

const BACKWARDS_COMPATIBILITY_SUPPORTED: bool = true;

pub struct RAnsBitDecoder {
    ans_decoder: AnsDecoder,
    prob_zero: u8,
}

impl RAnsBitDecoder {
    pub fn new() -> Self {
        Self {
            ans_decoder: AnsDecoder::new(),
            prob_zero: 0,
        }
    }

    pub fn start_decoding(&mut self, source_buffer: &mut DecoderBuffer) -> bool {
        self.clear();
        if !source_buffer.decode(&mut self.prob_zero) {
            return false;
        }
        let mut size_in_bytes: u32 = 0;
        if BACKWARDS_COMPATIBILITY_SUPPORTED
            && source_buffer.bitstream_version() < bitstream_version(2, 2)
        {
            if !source_buffer.decode(&mut size_in_bytes) {
                return false;
            }
        } else if !decode_varint(&mut size_in_bytes, source_buffer) {
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
        let bit = rabs_read(&mut self.ans_decoder, self.prob_zero);
        bit > 0
    }

    pub fn decode_least_significant_bits32(&mut self, nbits: i32, value: &mut u32) {
        draco_dcheck_eq!(true, nbits <= 32);
        draco_dcheck_eq!(true, nbits > 0);
        let mut result = 0u32;
        let mut remaining = nbits;
        while remaining > 0 {
            result = (result << 1) + if self.decode_next_bit() { 1 } else { 0 };
            remaining -= 1;
        }
        *value = result;
    }

    pub fn clear(&mut self) {
        ans_read_end(&mut self.ans_decoder);
    }
}

impl Default for RAnsBitDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl BitDecoder for RAnsBitDecoder {
    fn start_decoding(&mut self, source_buffer: &mut DecoderBuffer) -> bool {
        RAnsBitDecoder::start_decoding(self, source_buffer)
    }

    fn decode_next_bit(&mut self) -> bool {
        RAnsBitDecoder::decode_next_bit(self)
    }

    fn decode_least_significant_bits32(&mut self, nbits: i32, value: &mut u32) {
        RAnsBitDecoder::decode_least_significant_bits32(self, nbits, value);
    }

    fn end_decoding(&mut self) {
        // No-op for rANS bit decoder.
    }

    fn clear(&mut self) {
        RAnsBitDecoder::clear(self);
    }
}
