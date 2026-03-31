//! Math utilities.
//! Reference: `_ref/draco/src/draco/core/math_utils.h`.

/// Increment modulo helper (equivalent to DRACO_INCREMENT_MOD).
#[inline]
pub fn increment_mod(i: u32, m: u32) -> u32 {
    if i == m.saturating_sub(1) {
        0
    } else {
        i + 1
    }
}

/// Returns floor(sqrt(number)) for integer number.
#[inline]
pub fn int_sqrt(number: u64) -> u64 {
    if number == 0 {
        return 0;
    }

    let mut act_number = number;
    let mut square_root = 1u64;
    while act_number >= 2 {
        square_root *= 2;
        act_number /= 4;
    }

    loop {
        square_root = (square_root + number / square_root) / 2;
        if square_root * square_root <= number {
            break;
        }
    }

    square_root
}

/// Performs addition in unsigned type to avoid signed integer overflow.
#[inline]
pub fn add_as_unsigned<T>(a: T, b: T) -> T
where
    T: AddAsUnsigned,
{
    T::add_as_unsigned(a, b)
}

pub trait AddAsUnsigned: Sized {
    fn add_as_unsigned(a: Self, b: Self) -> Self;
}

macro_rules! impl_add_as_unsigned_signed {
    ($($t:ty),* $(,)?) => {
        $(
            impl AddAsUnsigned for $t {
                #[inline]
                fn add_as_unsigned(a: Self, b: Self) -> Self {
                    let ua = a as <$t as SignedToUnsigned>::Unsigned;
                    let ub = b as <$t as SignedToUnsigned>::Unsigned;
                    (ua.wrapping_add(ub)) as $t
                }
            }
        )*
    };
}

macro_rules! impl_add_as_unsigned_identity {
    ($($t:ty),* $(,)?) => {
        $(
            impl AddAsUnsigned for $t {
                #[inline]
                fn add_as_unsigned(a: Self, b: Self) -> Self {
                    a + b
                }
            }
        )*
    };
}

pub trait SignedToUnsigned {
    type Unsigned;
}

impl SignedToUnsigned for i8 {
    type Unsigned = u8;
}
impl SignedToUnsigned for i16 {
    type Unsigned = u16;
}
impl SignedToUnsigned for i32 {
    type Unsigned = u32;
}
impl SignedToUnsigned for i64 {
    type Unsigned = u64;
}
impl SignedToUnsigned for isize {
    type Unsigned = usize;
}

impl_add_as_unsigned_signed!(i8, i16, i32, i64, isize);
impl_add_as_unsigned_identity!(u8, u16, u32, u64, usize, f32, f64);
