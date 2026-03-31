//! Varint decoding utilities.
//! Reference: `_ref/draco/src/draco/core/varint_decoding.h`.

use crate::core::bit_utils::{convert_symbol_to_signed_int, DracoSigned, DracoUnsigned};
use crate::core::decoder_buffer::DecoderBuffer;

pub trait VarintDecode: Sized {
    fn decode_varint(out_val: &mut Self, buffer: &mut DecoderBuffer) -> bool;
}

fn decode_varint_unsigned<T: DracoUnsigned>(out_val: &mut T, buffer: &mut DecoderBuffer) -> bool {
    let max_depth = std::mem::size_of::<T>() + 1 + (std::mem::size_of::<T>() >> 3);
    let mut bytes: Vec<u8> = Vec::new();
    loop {
        if bytes.len() + 1 > max_depth {
            return false;
        }
        let mut byte: u8 = 0;
        if !buffer.decode(&mut byte) {
            return false;
        }
        bytes.push(byte);
        if (byte & 0x80) == 0 {
            break;
        }
    }

    let mut result: u128 = 0;
    for b in bytes.iter().rev() {
        result = (result << 7) | ((b & 0x7f) as u128);
    }
    *out_val = T::from_u128(result);
    true
}

macro_rules! impl_varint_decode_unsigned {
    ($($t:ty),* $(,)?) => {
        $(
            impl VarintDecode for $t {
                fn decode_varint(out_val: &mut Self, buffer: &mut DecoderBuffer) -> bool {
                    decode_varint_unsigned(out_val, buffer)
                }
            }
        )*
    };
}

macro_rules! impl_varint_decode_signed {
    ($($t:ty),* $(,)?) => {
        $(
            impl VarintDecode for $t {
                fn decode_varint(out_val: &mut Self, buffer: &mut DecoderBuffer) -> bool {
                    let mut symbol: <Self as DracoSigned>::Unsigned = <Self as DracoSigned>::Unsigned::from_u128(0);
                    if !decode_varint_unsigned(&mut symbol, buffer) {
                        return false;
                    }
                    *out_val = convert_symbol_to_signed_int(symbol);
                    true
                }
            }
        )*
    };
}

impl_varint_decode_unsigned!(u8, u16, u32, u64, usize);
impl_varint_decode_signed!(i8, i16, i32, i64, isize);

pub fn decode_varint<T: VarintDecode>(out_val: &mut T, buffer: &mut DecoderBuffer) -> bool {
    T::decode_varint(out_val, buffer)
}
