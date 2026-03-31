//! Folded integer bit decoder.
//! Reference: `_ref/draco/src/draco/compression/bit_coders/folded_integer_bit_decoder.h`.
//!
//! Decodes values encoded by FoldedBit32Encoder.

use crate::compression::bit_coders::BitDecoder;
use draco_core::core::decoder_buffer::DecoderBuffer;

pub struct FoldedBit32Decoder<BitDecoderT: BitDecoder + Default> {
    folded_number_decoders: [BitDecoderT; 32],
    bit_decoder: BitDecoderT,
}

impl<BitDecoderT: BitDecoder + Default> FoldedBit32Decoder<BitDecoderT> {
    pub fn new() -> Self {
        Self {
            folded_number_decoders: std::array::from_fn(|_| BitDecoderT::default()),
            bit_decoder: BitDecoderT::default(),
        }
    }

    pub fn start_decoding(&mut self, source_buffer: &mut DecoderBuffer) -> bool {
        for dec in &mut self.folded_number_decoders {
            if !dec.start_decoding(source_buffer) {
                return false;
            }
        }
        self.bit_decoder.start_decoding(source_buffer)
    }

    pub fn decode_next_bit(&mut self) -> bool {
        self.bit_decoder.decode_next_bit()
    }

    pub fn decode_least_significant_bits32(&mut self, nbits: i32, value: &mut u32) {
        let mut result = 0u32;
        for i in 0..nbits {
            let bit = self.folded_number_decoders[i as usize].decode_next_bit();
            result = (result << 1) + if bit { 1 } else { 0 };
        }
        *value = result;
    }

    pub fn end_decoding(&mut self) {
        for dec in &mut self.folded_number_decoders {
            dec.end_decoding();
        }
        self.bit_decoder.end_decoding();
    }
}

impl<BitDecoderT: BitDecoder + Default> Default for FoldedBit32Decoder<BitDecoderT> {
    fn default() -> Self {
        Self::new()
    }
}
