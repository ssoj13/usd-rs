//! Symbol decoding utilities.
//! Reference: `_ref/draco/src/draco/compression/entropy/symbol_decoding.h|cc`.
//!
//! Decodes symbol streams encoded by symbol_encoding.

use crate::compression::config::compression_shared::SymbolCodingMethod;
use crate::compression::entropy::rans_symbol_decoder::RAnsSymbolDecoder;
use draco_core::core::decoder_buffer::DecoderBuffer;

pub fn decode_symbols(
    num_values: u32,
    num_components: i32,
    src_buffer: &mut DecoderBuffer,
    out_values: &mut [u32],
) -> bool {
    if num_values == 0 {
        return true;
    }
    let mut scheme: u8 = 0;
    if !src_buffer.decode(&mut scheme) {
        return false;
    }
    if scheme == SymbolCodingMethod::SymbolCodingTagged as u8 {
        return decode_tagged_symbols(num_values, num_components, src_buffer, out_values);
    } else if scheme == SymbolCodingMethod::SymbolCodingRaw as u8 {
        return decode_raw_symbols(num_values, src_buffer, out_values);
    }
    false
}

fn decode_tagged_symbols(
    num_values: u32,
    num_components: i32,
    src_buffer: &mut DecoderBuffer,
    out_values: &mut [u32],
) -> bool {
    let mut tag_decoder = RAnsSymbolDecoder::<12>::new();
    if !tag_decoder.create(src_buffer) {
        return false;
    }
    if !tag_decoder.start_decoding(src_buffer) {
        return false;
    }
    if num_values > 0 && tag_decoder.num_symbols() == 0 {
        return false;
    }
    let mut bit_size = 0u64;
    if !src_buffer.start_bit_decoding(false, &mut bit_size) {
        return false;
    }
    let mut value_id = 0usize;
    let num_components = if num_components <= 0 {
        1
    } else {
        num_components
    } as u32;
    let mut i = 0u32;
    while i < num_values {
        let bit_length = tag_decoder.decode_symbol();
        for _ in 0..num_components {
            let mut val = 0u32;
            if !src_buffer.decode_least_significant_bits32(bit_length, &mut val) {
                return false;
            }
            out_values[value_id] = val;
            value_id += 1;
        }
        i += num_components;
    }
    tag_decoder.end_decoding();
    src_buffer.end_bit_decoding();
    true
}

fn decode_raw_symbols_internal<const RANS_PRECISION_BITS: u32>(
    num_values: u32,
    src_buffer: &mut DecoderBuffer,
    out_values: &mut [u32],
) -> bool {
    let mut decoder = RAnsSymbolDecoder::<RANS_PRECISION_BITS>::new();
    if !decoder.create(src_buffer) {
        return false;
    }
    if num_values > 0 && decoder.num_symbols() == 0 {
        return false;
    }
    if !decoder.start_decoding(src_buffer) {
        return false;
    }
    for i in 0..num_values as usize {
        let value = decoder.decode_symbol();
        out_values[i] = value;
    }
    decoder.end_decoding();
    true
}

fn decode_raw_symbols(
    num_values: u32,
    src_buffer: &mut DecoderBuffer,
    out_values: &mut [u32],
) -> bool {
    let mut max_bit_length: u8 = 0;
    if !src_buffer.decode(&mut max_bit_length) {
        return false;
    }
    match max_bit_length {
        1 => decode_raw_symbols_internal::<12>(num_values, src_buffer, out_values),
        2 => decode_raw_symbols_internal::<12>(num_values, src_buffer, out_values),
        3 => decode_raw_symbols_internal::<12>(num_values, src_buffer, out_values),
        4 => decode_raw_symbols_internal::<12>(num_values, src_buffer, out_values),
        5 => decode_raw_symbols_internal::<12>(num_values, src_buffer, out_values),
        6 => decode_raw_symbols_internal::<12>(num_values, src_buffer, out_values),
        7 => decode_raw_symbols_internal::<12>(num_values, src_buffer, out_values),
        8 => decode_raw_symbols_internal::<12>(num_values, src_buffer, out_values),
        9 => decode_raw_symbols_internal::<13>(num_values, src_buffer, out_values),
        10 => decode_raw_symbols_internal::<15>(num_values, src_buffer, out_values),
        11 => decode_raw_symbols_internal::<16>(num_values, src_buffer, out_values),
        12 => decode_raw_symbols_internal::<18>(num_values, src_buffer, out_values),
        13 => decode_raw_symbols_internal::<19>(num_values, src_buffer, out_values),
        14 => decode_raw_symbols_internal::<20>(num_values, src_buffer, out_values),
        15 => decode_raw_symbols_internal::<20>(num_values, src_buffer, out_values),
        16 => decode_raw_symbols_internal::<20>(num_values, src_buffer, out_values),
        17 => decode_raw_symbols_internal::<20>(num_values, src_buffer, out_values),
        18 => decode_raw_symbols_internal::<20>(num_values, src_buffer, out_values),
        _ => false,
    }
}
