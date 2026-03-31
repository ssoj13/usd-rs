//! Folded integer bit encoder.
//! Reference: `_ref/draco/src/draco/compression/bit_coders/folded_integer_bit_encoder.h`.
//!
//! Encodes each bit position with a dedicated bit encoder context.

use crate::compression::bit_coders::BitEncoder;
use draco_core::core::encoder_buffer::EncoderBuffer;

pub struct FoldedBit32Encoder<BitEncoderT: BitEncoder + Default> {
    folded_number_encoders: [BitEncoderT; 32],
    bit_encoder: BitEncoderT,
}

impl<BitEncoderT: BitEncoder + Default> FoldedBit32Encoder<BitEncoderT> {
    pub fn new() -> Self {
        Self {
            folded_number_encoders: std::array::from_fn(|_| BitEncoderT::default()),
            bit_encoder: BitEncoderT::default(),
        }
    }

    pub fn start_encoding(&mut self) {
        for enc in &mut self.folded_number_encoders {
            enc.start_encoding();
        }
        self.bit_encoder.start_encoding();
    }

    pub fn encode_bit(&mut self, bit: bool) {
        self.bit_encoder.encode_bit(bit);
    }

    pub fn encode_least_significant_bits32(&mut self, nbits: i32, value: u32) {
        let mut selector = 1u32 << (nbits - 1);
        for i in 0..nbits {
            let bit = (value & selector) != 0;
            self.folded_number_encoders[i as usize].encode_bit(bit);
            selector >>= 1;
        }
    }

    pub fn end_encoding(&mut self, target_buffer: &mut EncoderBuffer) {
        for enc in &mut self.folded_number_encoders {
            enc.end_encoding(target_buffer);
        }
        self.bit_encoder.end_encoding(target_buffer);
    }
}

impl<BitEncoderT: BitEncoder + Default> Default for FoldedBit32Encoder<BitEncoderT> {
    fn default() -> Self {
        Self::new()
    }
}
