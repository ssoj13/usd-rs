//! rANS symbol decoder.
//! Reference: `_ref/draco/src/draco/compression/entropy/rans_symbol_decoder.h`.
//!
//! Decodes probability tables and symbol streams encoded by rANS.

use crate::compression::config::compression_shared::bitstream_version;
use crate::compression::entropy::ans::RAnsDecoder;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::varint_decoding::decode_varint;

const BACKWARDS_COMPATIBILITY_SUPPORTED: bool = true;

pub struct RAnsSymbolDecoder<const RANS_PRECISION_BITS: u32> {
    probability_table: Vec<u32>,
    num_symbols: u32,
    ans: RAnsDecoder<RANS_PRECISION_BITS>,
}

impl<const RANS_PRECISION_BITS: u32> RAnsSymbolDecoder<RANS_PRECISION_BITS> {
    pub fn new() -> Self {
        Self {
            probability_table: Vec::new(),
            num_symbols: 0,
            ans: RAnsDecoder::new(),
        }
    }

    pub fn num_symbols(&self) -> u32 {
        self.num_symbols
    }

    pub fn create(&mut self, buffer: &mut DecoderBuffer) -> bool {
        if buffer.bitstream_version() == 0 {
            return false;
        }
        if BACKWARDS_COMPATIBILITY_SUPPORTED && buffer.bitstream_version() < bitstream_version(2, 0)
        {
            if !buffer.decode(&mut self.num_symbols) {
                return false;
            }
        } else if !decode_varint(&mut self.num_symbols, buffer) {
            return false;
        }
        if (self.num_symbols / 64) as i64 > buffer.remaining_size() {
            return false;
        }
        self.probability_table.resize(self.num_symbols as usize, 0);
        let mut i: u32 = 0;
        while i < self.num_symbols {
            let mut prob_data: u8 = 0;
            if !buffer.decode(&mut prob_data) {
                return false;
            }
            let token = prob_data & 3;
            if token == 3 {
                let offset = (prob_data >> 2) as u32;
                if i + offset >= self.num_symbols {
                    return false;
                }
                for j in 0..=offset {
                    self.probability_table[(i + j) as usize] = 0;
                }
                i += offset + 1;
            } else {
                let extra_bytes = token as i32;
                let mut prob = (prob_data >> 2) as u32;
                for b in 0..extra_bytes {
                    let mut eb: u8 = 0;
                    if !buffer.decode(&mut eb) {
                        return false;
                    }
                    prob |= (eb as u32) << (8 * (b + 1) - 2);
                }
                self.probability_table[i as usize] = prob;
                i += 1;
            }
        }
        if !self
            .ans
            .rans_build_look_up_table(&self.probability_table, self.num_symbols)
        {
            return false;
        }
        true
    }

    pub fn start_decoding(&mut self, buffer: &mut DecoderBuffer) -> bool {
        let mut bytes_encoded: u64 = 0;
        if BACKWARDS_COMPATIBILITY_SUPPORTED && buffer.bitstream_version() < bitstream_version(2, 0)
        {
            if !buffer.decode(&mut bytes_encoded) {
                return false;
            }
        } else if !decode_varint(&mut bytes_encoded, buffer) {
            return false;
        }
        if bytes_encoded as i64 > buffer.remaining_size() {
            return false;
        }
        let data_head = buffer.data_head().as_ptr();
        buffer.advance(bytes_encoded as i64);
        if bytes_encoded > i32::MAX as u64 {
            return false;
        }
        if self.ans.read_init(data_head, bytes_encoded as i32) != 0 {
            return false;
        }
        true
    }

    pub fn decode_symbol(&mut self) -> u32 {
        self.ans.rans_read()
    }

    pub fn end_decoding(&mut self) {
        self.ans.read_end();
    }
}
