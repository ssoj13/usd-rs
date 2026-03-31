//! Bit manipulation utilities.
//! Reference: `_ref/draco/src/draco/core/bit_utils.h` + `.cc`.

/// Returns the number of set bits in a 32-bit integer.
#[inline]
pub fn count_one_bits32(n: u32) -> i32 {
    n.count_ones() as i32
}

/// Returns the bit-reversed value of a 32-bit integer.
#[inline]
pub fn reverse_bits32(n: u32) -> u32 {
    n.reverse_bits()
}

/// Copies `nbits` from `src` into `dst` using the provided bit offsets.
///
/// Precondition: `nbits` in 1..=32 and offsets are non-negative.
#[inline]
pub fn copy_bits32(dst: &mut u32, dst_offset: i32, src: u32, src_offset: i32, nbits: i32) {
    debug_assert!(nbits >= 1 && nbits <= 32);
    debug_assert!(dst_offset >= 0 && src_offset >= 0);

    if nbits <= 0 {
        return;
    }

    let dst_offset_u = dst_offset as u32;
    let src_offset_u = src_offset as u32;
    let nbits_u = nbits as u32;

    let mask = if nbits_u == 32 {
        !0u32 << dst_offset_u
    } else {
        (!0u32 >> (32 - nbits_u)) << dst_offset_u
    };

    *dst = (*dst & !mask) | (((src >> src_offset_u) << dst_offset_u) & mask);
}

/// Returns the index of the most significant set bit.
/// Behavior is undefined for `n == 0` in the reference; we return -1.
#[inline]
pub fn most_significant_bit(n: u32) -> i32 {
    if n == 0 {
        return -1;
    }
    (31 - n.leading_zeros()) as i32
}

/// C++ parity: BitEncoder::BitsRequired(uint32_t x). Returns same as MostSignificantBit.
#[inline]
pub fn bits_required(x: u32) -> u32 {
    most_significant_bit(x) as u32
}

/// Converts a slice of signed i32 values to unsigned symbols.
pub fn convert_signed_ints_to_symbols(input: &[i32], output: &mut [u32]) {
    debug_assert_eq!(input.len(), output.len());
    for (src, dst) in input.iter().zip(output.iter_mut()) {
        *dst = convert_signed_int_to_symbol(*src);
    }
}

/// Converts a slice of unsigned symbols back to signed i32 values.
pub fn convert_symbols_to_signed_ints(input: &[u32], output: &mut [i32]) {
    debug_assert_eq!(input.len(), output.len());
    for (src, dst) in input.iter().zip(output.iter_mut()) {
        *dst = convert_symbol_to_signed_int(*src);
    }
}

/// Trait mapping a signed integer to its unsigned counterpart.
pub trait DracoSigned: Copy {
    type Unsigned: DracoUnsigned<Signed = Self>;
    fn to_i128(self) -> i128;
}

/// Trait mapping an unsigned integer to its signed counterpart.
pub trait DracoUnsigned: Copy {
    type Signed: Copy;
    fn to_u128(self) -> u128;
    fn from_u128(value: u128) -> Self;
    fn from_i128(value: i128) -> Self::Signed;
}

macro_rules! impl_signed_unsigned_pair {
    ($signed:ty, $unsigned:ty) => {
        impl DracoSigned for $signed {
            type Unsigned = $unsigned;
            #[inline]
            fn to_i128(self) -> i128 {
                self as i128
            }
        }

        impl DracoUnsigned for $unsigned {
            type Signed = $signed;
            #[inline]
            fn to_u128(self) -> u128 {
                self as u128
            }
            #[inline]
            fn from_u128(value: u128) -> Self {
                value as Self
            }
            #[inline]
            fn from_i128(value: i128) -> Self::Signed {
                value as Self::Signed
            }
        }
    };
}

impl_signed_unsigned_pair!(i8, u8);
impl_signed_unsigned_pair!(i16, u16);
impl_signed_unsigned_pair!(i32, u32);
impl_signed_unsigned_pair!(i64, u64);
impl_signed_unsigned_pair!(isize, usize);

/// Converts a signed integer into an unsigned symbol for entropy coding.
#[inline]
pub fn convert_signed_int_to_symbol<T: DracoSigned>(val: T) -> T::Unsigned {
    let v = val.to_i128();
    if v >= 0 {
        return T::Unsigned::from_u128((v as u128) << 1);
    }
    // Map -1 to 0, -2 to -1, etc.
    let mapped = -(v + 1);
    let u = ((mapped as u128) << 1) | 1;
    T::Unsigned::from_u128(u)
}

/// Converts an unsigned symbol back to a signed integer.
#[inline]
pub fn convert_symbol_to_signed_int<T: DracoUnsigned>(val: T) -> T::Signed {
    let u = val.to_u128();
    let is_positive = (u & 1) == 0;
    let shifted = (u >> 1) as i128;
    if is_positive {
        return T::from_i128(shifted);
    }
    let ret = -shifted - 1;
    T::from_i128(ret)
}
