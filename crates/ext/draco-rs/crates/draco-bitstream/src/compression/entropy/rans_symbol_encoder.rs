//! rANS symbol encoder.
//! Reference: `_ref/draco/src/draco/compression/entropy/rans_symbol_encoder.h`.
//!
//! Encodes a probability table and symbol stream using rANS. Used by symbol
//! encoding and bit coders.

use crate::compression::entropy::ans::{RAnsEncoder, RansSym};
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::core::varint_encoding::encode_varint;

pub struct RAnsSymbolEncoder<const RANS_PRECISION_BITS: u32> {
    probability_table: Vec<RansSym>,
    num_symbols: u32,
    num_expected_bits: u64,
    ans: RAnsEncoder<RANS_PRECISION_BITS>,
    encoded_buffer: Vec<u8>,
}

impl<const RANS_PRECISION_BITS: u32> RAnsSymbolEncoder<RANS_PRECISION_BITS> {
    const RANS_PRECISION: u32 = 1u32 << RANS_PRECISION_BITS;

    pub fn new() -> Self {
        Self {
            probability_table: Vec::new(),
            num_symbols: 0,
            num_expected_bits: 0,
            ans: RAnsEncoder::new(),
            encoded_buffer: Vec::new(),
        }
    }

    pub fn needs_reverse_encoding() -> bool {
        true
    }

    pub fn create(
        &mut self,
        frequencies: &[u64],
        num_symbols: usize,
        buffer: &mut EncoderBuffer,
    ) -> bool {
        let mut total_freq: u64 = 0;
        let mut max_valid_symbol: i32 = 0;
        for i in 0..num_symbols {
            total_freq += frequencies[i];
            if frequencies[i] > 0 {
                max_valid_symbol = i as i32;
            }
        }
        let num_symbols = (max_valid_symbol + 1) as usize;
        self.num_symbols = num_symbols as u32;
        self.probability_table
            .resize(num_symbols, RansSym::default());
        let total_freq_d = total_freq as f64;
        let rans_precision_d = Self::RANS_PRECISION as f64;
        let mut total_rans_prob: i32 = 0;

        for i in 0..num_symbols {
            let freq = frequencies[i];
            let prob = freq as f64 / total_freq_d;
            let mut rans_prob = (prob * rans_precision_d + 0.5f64) as u32;
            if rans_prob == 0 && freq > 0 {
                rans_prob = 1;
            }
            self.probability_table[i].prob = rans_prob;
            total_rans_prob += rans_prob as i32;
        }

        if total_rans_prob != Self::RANS_PRECISION as i32 {
            let mut sorted_probabilities: Vec<usize> = (0..num_symbols).collect();
            sorted_probabilities.sort_by(|a, b| {
                let pa = self.probability_table[*a].prob;
                let pb = self.probability_table[*b].prob;
                pa.cmp(&pb)
            });
            if total_rans_prob < Self::RANS_PRECISION as i32 {
                let extra = (Self::RANS_PRECISION as i32 - total_rans_prob) as u32;
                if let Some(last) = sorted_probabilities.last().cloned() {
                    self.probability_table[last].prob += extra;
                }
            } else {
                let mut error = total_rans_prob - Self::RANS_PRECISION as i32;
                while error > 0 {
                    let act_total_prob_d = total_rans_prob as f64;
                    let act_rel_error_d = rans_precision_d / act_total_prob_d;
                    for j in (1..num_symbols).rev() {
                        let symbol_id = sorted_probabilities[j];
                        if self.probability_table[symbol_id].prob <= 1 {
                            if j == num_symbols - 1 {
                                return false;
                            }
                            break;
                        }
                        let new_prob = (act_rel_error_d
                            * self.probability_table[symbol_id].prob as f64)
                            .floor() as i32;
                        let mut fix = self.probability_table[symbol_id].prob as i32 - new_prob;
                        if fix == 0 {
                            fix = 1;
                        }
                        if fix >= self.probability_table[symbol_id].prob as i32 {
                            fix = self.probability_table[symbol_id].prob as i32 - 1;
                        }
                        if fix > error {
                            fix = error;
                        }
                        self.probability_table[symbol_id].prob -= fix as u32;
                        total_rans_prob -= fix;
                        error -= fix;
                        if total_rans_prob == Self::RANS_PRECISION as i32 {
                            break;
                        }
                    }
                }
            }
        }

        let mut total_prob: u32 = 0;
        for i in 0..num_symbols {
            self.probability_table[i].cum_prob = total_prob;
            total_prob += self.probability_table[i].prob;
        }
        if total_prob != Self::RANS_PRECISION {
            return false;
        }

        let mut num_bits: f64 = 0.0;
        for i in 0..num_symbols {
            if self.probability_table[i].prob == 0 {
                continue;
            }
            let norm_prob = self.probability_table[i].prob as f64 / rans_precision_d;
            num_bits += frequencies[i] as f64 * norm_prob.log2();
        }
        self.num_expected_bits = (-num_bits).ceil() as u64;
        self.encode_table(buffer)
    }

    pub fn start_encoding(&mut self, _buffer: &mut EncoderBuffer) {
        let required_bits = 2 * self.num_expected_bits + 32;
        let required_bytes = ((required_bits + 7) / 8) as usize;
        self.encoded_buffer.clear();
        self.encoded_buffer.resize(required_bytes, 0);
        let ptr = self.encoded_buffer.as_mut_ptr();
        self.ans.write_init(ptr);
    }

    pub fn encode_symbol(&mut self, symbol: u32) {
        self.ans
            .rans_write(&self.probability_table[symbol as usize]);
    }

    pub fn end_encoding(&mut self, buffer: &mut EncoderBuffer) {
        let bytes_written = self.ans.write_end() as usize;
        let mut var_size_buffer = EncoderBuffer::new();
        encode_varint(bytes_written as u64, &mut var_size_buffer);
        buffer.encode_bytes(var_size_buffer.data());
        buffer.encode_bytes(&self.encoded_buffer[..bytes_written]);
    }

    fn encode_table(&mut self, buffer: &mut EncoderBuffer) -> bool {
        encode_varint(self.num_symbols, buffer);
        let mut i = 0usize;
        while i < self.num_symbols as usize {
            let prob = self.probability_table[i].prob;
            let mut num_extra_bytes = 0;
            if prob >= (1 << 6) {
                num_extra_bytes += 1;
                if prob >= (1 << 14) {
                    num_extra_bytes += 1;
                    if prob >= (1 << 22) {
                        return false;
                    }
                }
            }
            if prob == 0 {
                let mut offset: u32 = 0;
                while offset < (1 << 6) - 1 {
                    let next_prob = self.probability_table[i + offset as usize + 1].prob;
                    if next_prob > 0 {
                        break;
                    }
                    offset += 1;
                }
                buffer.encode((offset << 2 | 3) as u8);
                i += offset as usize + 1;
            } else {
                buffer.encode(((prob << 2) | (num_extra_bytes & 3)) as u8);
                for b in 0..num_extra_bytes {
                    buffer.encode((prob >> (8 * (b + 1) - 2)) as u8);
                }
                i += 1;
            }
        }
        true
    }
}
