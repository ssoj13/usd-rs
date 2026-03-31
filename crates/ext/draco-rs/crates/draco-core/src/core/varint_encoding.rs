//! Varint encoding utilities.
//! Reference: `_ref/draco/src/draco/core/varint_encoding.h`.

use crate::core::bit_utils::{convert_signed_int_to_symbol, DracoUnsigned};
use crate::core::encoder_buffer::EncoderBuffer;

pub trait VarintEncode {
    fn encode_varint(self, out_buffer: &mut EncoderBuffer) -> bool;
}

fn encode_varint_unsigned<T: DracoUnsigned>(val: T, out_buffer: &mut EncoderBuffer) -> bool {
    let mut value = val.to_u128();
    loop {
        let mut out = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            out |= 0x80;
            if !out_buffer.encode(out) {
                return false;
            }
        } else {
            return out_buffer.encode(out);
        }
    }
}

macro_rules! impl_varint_encode_unsigned {
    ($($t:ty),* $(,)?) => {
        $(
            impl VarintEncode for $t {
                fn encode_varint(self, out_buffer: &mut EncoderBuffer) -> bool {
                    encode_varint_unsigned(self, out_buffer)
                }
            }
        )*
    };
}

macro_rules! impl_varint_encode_signed {
    ($($t:ty),* $(,)?) => {
        $(
            impl VarintEncode for $t {
                fn encode_varint(self, out_buffer: &mut EncoderBuffer) -> bool {
                    let symbol = convert_signed_int_to_symbol(self);
                    encode_varint_unsigned(symbol, out_buffer)
                }
            }
        )*
    };
}

impl_varint_encode_unsigned!(u8, u16, u32, u64, usize);
impl_varint_encode_signed!(i8, i16, i32, i64, isize);

pub fn encode_varint<T: VarintEncode>(val: T, out_buffer: &mut EncoderBuffer) -> bool {
    val.encode_varint(out_buffer)
}
