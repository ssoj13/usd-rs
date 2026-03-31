//! Symbol encoding utilities.
//! Reference: `_ref/draco/src/draco/compression/entropy/symbol_encoding.h|cc`.
//!
//! Encodes symbol streams using tagged or raw rANS-based entropy coding.

use crate::compression::config::compression_shared::SymbolCodingMethod;
use crate::compression::entropy::rans_symbol_coding::approximate_rans_frequency_table_bits;
use crate::compression::entropy::rans_symbol_encoder::RAnsSymbolEncoder;
use crate::compression::entropy::shannon_entropy::compute_shannon_entropy;
use draco_core::core::bit_utils::most_significant_bit;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::core::options::Options;

const K_MAX_TAG_SYMBOL_BIT_LENGTH: i32 = 32;
const K_MAX_RAW_ENCODING_BIT_LENGTH: i32 = 18;
const K_DEFAULT_SYMBOL_CODING_COMPRESSION_LEVEL: i32 = 7;

pub fn set_symbol_encoding_method(options: &mut Options, method: SymbolCodingMethod) {
    options.set_int("symbol_encoding_method", method as i32);
}

pub fn set_symbol_encoding_compression_level(
    options: &mut Options,
    compression_level: i32,
) -> bool {
    if compression_level < 0 || compression_level > 10 {
        return false;
    }
    options.set_int("symbol_encoding_compression_level", compression_level);
    true
}

pub fn encode_symbols(
    symbols: &[u32],
    num_values: i32,
    num_components: i32,
    options: Option<&Options>,
    target_buffer: &mut EncoderBuffer,
) -> bool {
    if num_values < 0 {
        return false;
    }
    if num_values == 0 {
        return true;
    }
    let num_components = if num_components <= 0 {
        1
    } else {
        num_components
    };

    let mut bit_lengths: Vec<u32> = Vec::new();
    let mut max_value: u32 = 0;
    compute_bit_lengths(
        symbols,
        num_values,
        num_components,
        &mut bit_lengths,
        &mut max_value,
    );

    let tagged_scheme_total_bits = approximate_tagged_scheme_bits(&bit_lengths, num_components);

    let mut num_unique_symbols: i32 = 0;
    let raw_scheme_total_bits =
        approximate_raw_scheme_bits(symbols, num_values, max_value, &mut num_unique_symbols);

    let max_value_bit_length = most_significant_bit(std::cmp::max(1, max_value) as u32) + 1;

    let mut method: i32 = -1;
    if let Some(options) = options {
        if options.is_option_set("symbol_encoding_method") {
            method = options.get_int("symbol_encoding_method");
        }
    }
    if method == -1 {
        if tagged_scheme_total_bits < raw_scheme_total_bits
            || max_value_bit_length > K_MAX_RAW_ENCODING_BIT_LENGTH
        {
            method = SymbolCodingMethod::SymbolCodingTagged as i32;
        } else {
            method = SymbolCodingMethod::SymbolCodingRaw as i32;
        }
    }

    target_buffer.encode(method as u8);
    if method == SymbolCodingMethod::SymbolCodingTagged as i32 {
        return encode_tagged_symbols(
            symbols,
            num_values,
            num_components,
            &bit_lengths,
            target_buffer,
        );
    }
    if method == SymbolCodingMethod::SymbolCodingRaw as i32 {
        return encode_raw_symbols(
            symbols,
            num_values,
            max_value,
            num_unique_symbols,
            options,
            target_buffer,
        );
    }
    false
}

fn compute_bit_lengths(
    symbols: &[u32],
    num_values: i32,
    num_components: i32,
    out_bit_lengths: &mut Vec<u32>,
    out_max_value: &mut u32,
) {
    out_bit_lengths.clear();
    out_bit_lengths.reserve(num_values as usize);
    *out_max_value = 0;
    let mut i = 0i32;
    while i < num_values {
        let mut max_component_value = symbols[i as usize];
        for j in 1..num_components {
            let value = symbols[(i + j) as usize];
            if max_component_value < value {
                max_component_value = value;
            }
        }
        let mut value_msb_pos = 0;
        if max_component_value > 0 {
            value_msb_pos = most_significant_bit(max_component_value) as i32;
        }
        if max_component_value > *out_max_value {
            *out_max_value = max_component_value;
        }
        out_bit_lengths.push((value_msb_pos + 1) as u32);
        i += num_components;
    }
}

fn approximate_tagged_scheme_bits(bit_lengths: &[u32], num_components: i32) -> i64 {
    let mut total_bit_length: u64 = 0;
    for v in bit_lengths {
        total_bit_length += *v as u64;
    }
    let mut num_unique_symbols: i32 = 0;
    let tag_bits = compute_shannon_entropy(
        bit_lengths,
        bit_lengths.len() as i32,
        32,
        Some(&mut num_unique_symbols),
    );
    let tag_table_bits =
        approximate_rans_frequency_table_bits(num_unique_symbols, num_unique_symbols);
    tag_bits + tag_table_bits + total_bit_length as i64 * num_components as i64
}

fn approximate_raw_scheme_bits(
    symbols: &[u32],
    num_symbols: i32,
    max_value: u32,
    out_num_unique_symbols: &mut i32,
) -> i64 {
    let data_bits = compute_shannon_entropy(
        symbols,
        num_symbols,
        max_value as i32,
        Some(out_num_unique_symbols),
    );
    let table_bits =
        approximate_rans_frequency_table_bits(max_value as i32, *out_num_unique_symbols);
    table_bits + data_bits
}

fn encode_tagged_symbols(
    symbols: &[u32],
    num_values: i32,
    num_components: i32,
    bit_lengths: &[u32],
    target_buffer: &mut EncoderBuffer,
) -> bool {
    let mut frequencies = vec![0u64; K_MAX_TAG_SYMBOL_BIT_LENGTH as usize];
    for bit_length in bit_lengths {
        let idx = *bit_length as usize;
        if idx >= frequencies.len() {
            return false;
        }
        frequencies[idx] += 1;
    }

    let mut value_buffer = EncoderBuffer::new();
    let value_bits = K_MAX_TAG_SYMBOL_BIT_LENGTH as u64 * num_values as u64;

    let mut tag_encoder = RAnsSymbolEncoder::<12>::new();
    if !tag_encoder.create(
        &frequencies,
        K_MAX_TAG_SYMBOL_BIT_LENGTH as usize,
        target_buffer,
    ) {
        return false;
    }

    tag_encoder.start_encoding(target_buffer);
    if !value_buffer.start_bit_encoding(value_bits as i64, false) {
        return false;
    }

    if RAnsSymbolEncoder::<12>::needs_reverse_encoding() {
        let mut i = num_values - num_components;
        while i >= 0 {
            let bit_length = bit_lengths[(i / num_components) as usize] as u32;
            tag_encoder.encode_symbol(bit_length);

            let j = num_values - num_components - i;
            let value_bit_length = bit_lengths[(j / num_components) as usize] as u32;
            for c in 0..num_components {
                value_buffer.encode_least_significant_bits32(
                    value_bit_length as i32,
                    symbols[(j + c) as usize],
                );
            }
            if i == 0 {
                break;
            }
            i -= num_components;
        }
    } else {
        let mut i = 0;
        while i < num_values {
            let bit_length = bit_lengths[(i / num_components) as usize] as u32;
            tag_encoder.encode_symbol(bit_length);
            for j in 0..num_components {
                value_buffer
                    .encode_least_significant_bits32(bit_length as i32, symbols[(i + j) as usize]);
            }
            i += num_components;
        }
    }

    tag_encoder.end_encoding(target_buffer);
    value_buffer.end_bit_encoding();
    target_buffer.encode_bytes(value_buffer.data());
    true
}

fn encode_raw_symbols_internal<const RANS_PRECISION_BITS: u32>(
    symbols: &[u32],
    num_values: i32,
    max_entry_value: u32,
    target_buffer: &mut EncoderBuffer,
) -> bool {
    let mut frequencies = vec![0u64; max_entry_value as usize + 1];
    for i in 0..num_values {
        frequencies[symbols[i as usize] as usize] += 1;
    }

    let mut encoder = RAnsSymbolEncoder::<RANS_PRECISION_BITS>::new();
    if !encoder.create(&frequencies, frequencies.len(), target_buffer) {
        return false;
    }
    encoder.start_encoding(target_buffer);

    if RAnsSymbolEncoder::<RANS_PRECISION_BITS>::needs_reverse_encoding() {
        let mut i = num_values - 1;
        while i >= 0 {
            encoder.encode_symbol(symbols[i as usize]);
            if i == 0 {
                break;
            }
            i -= 1;
        }
    } else {
        for i in 0..num_values {
            encoder.encode_symbol(symbols[i as usize]);
        }
    }
    encoder.end_encoding(target_buffer);
    true
}

fn encode_raw_symbols(
    symbols: &[u32],
    num_values: i32,
    max_entry_value: u32,
    num_unique_symbols: i32,
    options: Option<&Options>,
    target_buffer: &mut EncoderBuffer,
) -> bool {
    let mut symbol_bits = 0;
    if num_unique_symbols > 0 {
        symbol_bits = most_significant_bit(num_unique_symbols as u32) as i32;
    }
    let mut unique_symbols_bit_length = symbol_bits + 1;
    if unique_symbols_bit_length > K_MAX_RAW_ENCODING_BIT_LENGTH {
        return false;
    }
    let mut compression_level = K_DEFAULT_SYMBOL_CODING_COMPRESSION_LEVEL;
    if let Some(options) = options {
        if options.is_option_set("symbol_encoding_compression_level") {
            compression_level = options.get_int("symbol_encoding_compression_level");
        }
    }
    if compression_level < 4 {
        unique_symbols_bit_length -= 2;
    } else if compression_level < 6 {
        unique_symbols_bit_length -= 1;
    } else if compression_level > 9 {
        unique_symbols_bit_length += 2;
    } else if compression_level > 7 {
        unique_symbols_bit_length += 1;
    }
    unique_symbols_bit_length = std::cmp::min(
        std::cmp::max(1, unique_symbols_bit_length),
        K_MAX_RAW_ENCODING_BIT_LENGTH,
    );
    target_buffer.encode(unique_symbols_bit_length as u8);

    match unique_symbols_bit_length {
        0 | 1 => {
            encode_raw_symbols_internal::<12>(symbols, num_values, max_entry_value, target_buffer)
        }
        2 => encode_raw_symbols_internal::<12>(symbols, num_values, max_entry_value, target_buffer),
        3 => encode_raw_symbols_internal::<12>(symbols, num_values, max_entry_value, target_buffer),
        4 => encode_raw_symbols_internal::<12>(symbols, num_values, max_entry_value, target_buffer),
        5 => encode_raw_symbols_internal::<12>(symbols, num_values, max_entry_value, target_buffer),
        6 => encode_raw_symbols_internal::<12>(symbols, num_values, max_entry_value, target_buffer),
        7 => encode_raw_symbols_internal::<12>(symbols, num_values, max_entry_value, target_buffer),
        8 => encode_raw_symbols_internal::<12>(symbols, num_values, max_entry_value, target_buffer),
        9 => encode_raw_symbols_internal::<13>(symbols, num_values, max_entry_value, target_buffer),
        10 => {
            encode_raw_symbols_internal::<15>(symbols, num_values, max_entry_value, target_buffer)
        }
        11 => {
            encode_raw_symbols_internal::<16>(symbols, num_values, max_entry_value, target_buffer)
        }
        12 => {
            encode_raw_symbols_internal::<18>(symbols, num_values, max_entry_value, target_buffer)
        }
        13 => {
            encode_raw_symbols_internal::<19>(symbols, num_values, max_entry_value, target_buffer)
        }
        14 => {
            encode_raw_symbols_internal::<20>(symbols, num_values, max_entry_value, target_buffer)
        }
        15 => {
            encode_raw_symbols_internal::<20>(symbols, num_values, max_entry_value, target_buffer)
        }
        16 => {
            encode_raw_symbols_internal::<20>(symbols, num_values, max_entry_value, target_buffer)
        }
        17 => {
            encode_raw_symbols_internal::<20>(symbols, num_values, max_entry_value, target_buffer)
        }
        18 => {
            encode_raw_symbols_internal::<20>(symbols, num_values, max_entry_value, target_buffer)
        }
        _ => false,
    }
}
