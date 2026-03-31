//! Shared utilities for rANS symbol coding.
//! Reference: `_ref/draco/src/draco/compression/entropy/rans_symbol_coding.h`.
//!
//! Provides precision selection and frequency table size estimation used by
//! rANS symbol encoders/decoders.

/// Computes the desired precision (unclamped) of rANS for the given symbol bit-length.
#[inline]
pub const fn compute_rans_unclamped_precision(symbols_bit_length: i32) -> i32 {
    (3 * symbols_bit_length) / 2
}

/// Computes clamped rANS precision to keep the coding tables valid.
#[inline]
pub const fn compute_rans_precision_from_unique_symbols_bit_length(symbols_bit_length: i32) -> i32 {
    let unclamped = compute_rans_unclamped_precision(symbols_bit_length);
    if unclamped < 12 {
        12
    } else if unclamped > 20 {
        20
    } else {
        unclamped
    }
}

/// Approximate frequency table size needed for storing the provided symbols.
#[inline]
pub fn approximate_rans_frequency_table_bits(max_value: i32, num_unique_symbols: i32) -> i64 {
    let table_zero_frequency_bits =
        8 * (num_unique_symbols as i64 + ((max_value - num_unique_symbols) / 64) as i64);
    8 * num_unique_symbols as i64 + table_zero_frequency_bits
}
