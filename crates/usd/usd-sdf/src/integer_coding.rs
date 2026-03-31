//! Integer compression for crate format.
//!
//! This module implements the integer compression scheme used in USD crate files.
//! The encoding works as follows:
//!
//! 1. Delta encoding: transform integers to differences from previous value
//! 2. Find most common delta value
//! 3. Encode each delta as 2-bit code (00=common, 01=8bit, 10=16bit, 11=32bit)
//! 4. Write codes + variable-length data
//! 5. LZ4 compress the result (TfFastCompression format)
//!
//! For 64-bit integers, the small/medium sizes are 16/32 bits instead of 8/16.
//!
//! Note: Some older USDC files may contain raw integer-encoded data without
//! the LZ4/TfFastCompression wrapper. This module detects and handles both formats.

use std::collections::HashMap;

use usd_tf::fast_compression::FastCompression;

/// Error type for integer coding operations.
#[derive(Debug, Clone)]
pub enum IntegerCodingError {
    /// Decompression failed
    DecompressionFailed(String),
    /// Invalid data format
    InvalidFormat(String),
}

impl std::fmt::Display for IntegerCodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DecompressionFailed(msg) => write!(f, "Decompression failed: {}", msg),
            Self::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
        }
    }
}

impl std::error::Error for IntegerCodingError {}

// Code values for 2-bit encoding
const CODE_COMMON: u8 = 0;
const CODE_SMALL: u8 = 1;
const CODE_MEDIUM: u8 = 2;
const CODE_LARGE: u8 = 3;

/// Calculate encoded buffer size for 32-bit integers.
fn encoded_buffer_size_32(num_ints: usize) -> usize {
    if num_ints == 0 {
        return 0;
    }
    // commonValue (4) + numCodesBytes + maxIntBytes
    4 + (num_ints * 2).div_ceil(8) + num_ints * 4
}

/// Calculate encoded buffer size for 64-bit integers.
fn encoded_buffer_size_64(num_ints: usize) -> usize {
    if num_ints == 0 {
        return 0;
    }
    // commonValue (8) + numCodesBytes + maxIntBytes
    8 + (num_ints * 2).div_ceil(8) + num_ints * 8
}

/// 32-bit integer compression.
pub struct IntegerCompression;

impl IntegerCompression {
    /// Returns the compressed buffer size needed for num_ints 32-bit integers.
    pub fn compressed_buffer_size(num_ints: usize) -> usize {
        FastCompression::compressed_buffer_size(encoded_buffer_size_32(num_ints)).unwrap_or(0)
    }

    /// Returns the decompression working space size for num_ints 32-bit integers.
    pub fn decompression_working_space_size(num_ints: usize) -> usize {
        encoded_buffer_size_32(num_ints)
    }

    /// Compress 32-bit signed integers.
    pub fn compress_i32(ints: &[i32]) -> Result<Vec<u8>, IntegerCodingError> {
        if ints.is_empty() {
            return Ok(Vec::new());
        }

        // Encode
        let encoded = encode_integers_i32(ints);

        // Compress with LZ4
        FastCompression::compress(&encoded)
            .map_err(|e| IntegerCodingError::DecompressionFailed(format!("{:?}", e)))
    }

    /// Compress 32-bit unsigned integers.
    pub fn compress_u32(ints: &[u32]) -> Result<Vec<u8>, IntegerCodingError> {
        // Reinterpret as signed for encoding
        let signed: Vec<i32> = ints.iter().map(|&x| x as i32).collect();
        Self::compress_i32(&signed)
    }

    /// Decompress to 32-bit signed integers.
    ///
    /// C++ always uses TfFastCompression (LZ4) as the outer wrapper,
    /// with integer encoding (delta + variable width) as the inner layer.
    pub fn decompress_i32(
        compressed: &[u8],
        num_ints: usize,
    ) -> Result<Vec<i32>, IntegerCodingError> {
        if num_ints == 0 || compressed.is_empty() {
            return Ok(Vec::new());
        }

        // Decompress LZ4 via TfFastCompression, then decode integers.
        let working_size = Self::decompression_working_space_size(num_ints);
        let decompressed = FastCompression::decompress(compressed, working_size)
            .map_err(|e| IntegerCodingError::DecompressionFailed(format!("{:?}", e)))?;
        decode_integers_i32(&decompressed, num_ints)
    }

    /// Decompress to 32-bit unsigned integers.
    pub fn decompress_u32(
        compressed: &[u8],
        num_ints: usize,
    ) -> Result<Vec<u32>, IntegerCodingError> {
        let signed = Self::decompress_i32(compressed, num_ints)?;
        Ok(signed.into_iter().map(|x| x as u32).collect())
    }

    // Proxy methods for 64-bit integers - delegates to IntegerCompression64

    /// Decompress to 64-bit signed integers.
    /// Proxy to IntegerCompression64::decompress_i64.
    pub fn decompress_i64(
        compressed: &[u8],
        num_ints: usize,
    ) -> Result<Vec<i64>, IntegerCodingError> {
        IntegerCompression64::decompress_i64(compressed, num_ints)
    }

    /// Decompress to 64-bit unsigned integers.
    /// Proxy to IntegerCompression64 with reinterpret cast.
    pub fn decompress_u64(
        compressed: &[u8],
        num_ints: usize,
    ) -> Result<Vec<u64>, IntegerCodingError> {
        let signed = IntegerCompression64::decompress_i64(compressed, num_ints)?;
        Ok(signed.into_iter().map(|x| x as u64).collect())
    }
}

/// 64-bit integer compression.
pub struct IntegerCompression64;

impl IntegerCompression64 {
    /// Returns the compressed buffer size needed for num_ints 64-bit integers.
    pub fn compressed_buffer_size(num_ints: usize) -> usize {
        FastCompression::compressed_buffer_size(encoded_buffer_size_64(num_ints)).unwrap_or(0)
    }

    /// Returns the decompression working space size for num_ints 64-bit integers.
    pub fn decompression_working_space_size(num_ints: usize) -> usize {
        encoded_buffer_size_64(num_ints)
    }

    /// Compress 64-bit signed integers.
    pub fn compress_i64(ints: &[i64]) -> Result<Vec<u8>, IntegerCodingError> {
        if ints.is_empty() {
            return Ok(Vec::new());
        }

        // Encode
        let encoded = encode_integers_i64(ints);

        // Compress with LZ4
        FastCompression::compress(&encoded)
            .map_err(|e| IntegerCodingError::DecompressionFailed(format!("{:?}", e)))
    }

    /// Decompress to 64-bit signed integers.
    ///
    /// Decompress to 64-bit signed integers.
    ///
    /// Tries TfFastCompression (LZ4) first, falls back to raw integer-encoded data.
    pub fn decompress_i64(
        compressed: &[u8],
        num_ints: usize,
    ) -> Result<Vec<i64>, IntegerCodingError> {
        if num_ints == 0 || compressed.is_empty() {
            return Ok(Vec::new());
        }

        let working_size = Self::decompression_working_space_size(num_ints);
        let decompressed = FastCompression::decompress(compressed, working_size)
            .map_err(|e| IntegerCodingError::DecompressionFailed(format!("{:?}", e)))?;
        decode_integers_i64(&decompressed, num_ints)
    }
}

/// Encode 32-bit integers using delta encoding + variable width.
fn encode_integers_i32(ints: &[i32]) -> Vec<u8> {
    if ints.is_empty() {
        return Vec::new();
    }

    let num_ints = ints.len();

    // Calculate deltas and find most common value
    let mut deltas: Vec<i32> = Vec::with_capacity(num_ints);
    let mut prev: i32 = 0;
    for &val in ints {
        deltas.push(val.wrapping_sub(prev));
        prev = val;
    }

    // Find most common delta
    let mut counts: HashMap<i32, usize> = HashMap::new();
    for &delta in &deltas {
        *counts.entry(delta).or_insert(0) += 1;
    }

    let common_value = counts
        .into_iter()
        .max_by(|a, b| {
            // Primary: count, secondary: value (larger wins on tie)
            a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0))
        })
        .map(|(v, _)| v)
        .unwrap_or(0);

    // Allocate output buffer
    let num_codes_bytes = (num_ints * 2).div_ceil(8);
    let mut output = Vec::with_capacity(encoded_buffer_size_32(num_ints));

    // Write common value
    output.extend_from_slice(&common_value.to_le_bytes());

    // Reserve space for codes
    let codes_start = output.len();
    output.resize(codes_start + num_codes_bytes, 0);

    // Write codes and variable-length data
    let mut code_byte_idx = 0;
    let mut code_bit_idx = 0;

    for &delta in &deltas {
        let code = get_code_32(delta, common_value);

        // Write 2-bit code
        output[codes_start + code_byte_idx] |= code << (code_bit_idx * 2);
        code_bit_idx += 1;
        if code_bit_idx == 4 {
            code_bit_idx = 0;
            code_byte_idx += 1;
        }

        // Write variable-length data
        match code {
            CODE_COMMON => {}
            CODE_SMALL => {
                output.push(delta as i8 as u8);
            }
            CODE_MEDIUM => {
                output.extend_from_slice(&(delta as i16).to_le_bytes());
            }
            CODE_LARGE => {
                output.extend_from_slice(&delta.to_le_bytes());
            }
            _ => unreachable!(),
        }
    }

    output
}

/// Decode 32-bit integers from encoded buffer.
fn decode_integers_i32(data: &[u8], num_ints: usize) -> Result<Vec<i32>, IntegerCodingError> {
    if data.len() < 4 {
        return Err(IntegerCodingError::InvalidFormat(
            "Data too short for common value".to_string(),
        ));
    }

    // Read common value
    let common_value = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);

    let num_codes_bytes = (num_ints * 2).div_ceil(8);
    let codes_start = 4;
    let vints_start = codes_start + num_codes_bytes;

    if data.len() < vints_start {
        return Err(IntegerCodingError::InvalidFormat(
            "Data too short for codes".to_string(),
        ));
    }

    let mut result = Vec::with_capacity(num_ints);
    let mut prev: i32 = 0;
    let mut vints_offset = vints_start;

    for i in 0..num_ints {
        let code_byte_idx = i / 4;
        let code_bit_idx = i % 4;
        let code = (data[codes_start + code_byte_idx] >> (code_bit_idx * 2)) & 0x03;

        let delta: i32 = match code {
            CODE_COMMON => common_value,
            CODE_SMALL => {
                if vints_offset >= data.len() {
                    return Err(IntegerCodingError::InvalidFormat(
                        "Unexpected end of data".to_string(),
                    ));
                }
                let v = data[vints_offset] as i8 as i32;
                vints_offset += 1;
                v
            }
            CODE_MEDIUM => {
                if vints_offset + 2 > data.len() {
                    return Err(IntegerCodingError::InvalidFormat(
                        "Unexpected end of data".to_string(),
                    ));
                }
                let v = i16::from_le_bytes([data[vints_offset], data[vints_offset + 1]]) as i32;
                vints_offset += 2;
                v
            }
            CODE_LARGE => {
                if vints_offset + 4 > data.len() {
                    return Err(IntegerCodingError::InvalidFormat(
                        "Unexpected end of data".to_string(),
                    ));
                }
                let v = i32::from_le_bytes([
                    data[vints_offset],
                    data[vints_offset + 1],
                    data[vints_offset + 2],
                    data[vints_offset + 3],
                ]);
                vints_offset += 4;
                v
            }
            _ => {
                return Err(IntegerCodingError::InvalidFormat(format!(
                    "Invalid code: {}",
                    code
                )));
            }
        };

        prev = prev.wrapping_add(delta);
        result.push(prev);
    }

    Ok(result)
}

/// Get 2-bit code for a 32-bit delta value.
fn get_code_32(delta: i32, common_value: i32) -> u8 {
    if delta == common_value {
        CODE_COMMON
    } else if delta >= i8::MIN as i32 && delta <= i8::MAX as i32 {
        CODE_SMALL
    } else if delta >= i16::MIN as i32 && delta <= i16::MAX as i32 {
        CODE_MEDIUM
    } else {
        CODE_LARGE
    }
}

/// Encode 64-bit integers using delta encoding + variable width.
fn encode_integers_i64(ints: &[i64]) -> Vec<u8> {
    if ints.is_empty() {
        return Vec::new();
    }

    let num_ints = ints.len();

    // Calculate deltas and find most common value
    let mut deltas: Vec<i64> = Vec::with_capacity(num_ints);
    let mut prev: i64 = 0;
    for &val in ints {
        deltas.push(val.wrapping_sub(prev));
        prev = val;
    }

    // Find most common delta
    let mut counts: HashMap<i64, usize> = HashMap::new();
    for &delta in &deltas {
        *counts.entry(delta).or_insert(0) += 1;
    }

    let common_value = counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
        .map(|(v, _)| v)
        .unwrap_or(0);

    // Allocate output buffer
    let num_codes_bytes = (num_ints * 2).div_ceil(8);
    let mut output = Vec::with_capacity(encoded_buffer_size_64(num_ints));

    // Write common value
    output.extend_from_slice(&common_value.to_le_bytes());

    // Reserve space for codes
    let codes_start = output.len();
    output.resize(codes_start + num_codes_bytes, 0);

    // Write codes and variable-length data
    let mut code_byte_idx = 0;
    let mut code_bit_idx = 0;

    for &delta in &deltas {
        let code = get_code_64(delta, common_value);

        // Write 2-bit code
        output[codes_start + code_byte_idx] |= code << (code_bit_idx * 2);
        code_bit_idx += 1;
        if code_bit_idx == 4 {
            code_bit_idx = 0;
            code_byte_idx += 1;
        }

        // Write variable-length data
        match code {
            CODE_COMMON => {}
            CODE_SMALL => {
                // For 64-bit, small is 16-bit
                output.extend_from_slice(&(delta as i16).to_le_bytes());
            }
            CODE_MEDIUM => {
                // For 64-bit, medium is 32-bit
                output.extend_from_slice(&(delta as i32).to_le_bytes());
            }
            CODE_LARGE => {
                output.extend_from_slice(&delta.to_le_bytes());
            }
            _ => unreachable!(),
        }
    }

    output
}

/// Decode 64-bit integers from encoded buffer.
fn decode_integers_i64(data: &[u8], num_ints: usize) -> Result<Vec<i64>, IntegerCodingError> {
    if data.len() < 8 {
        return Err(IntegerCodingError::InvalidFormat(
            "Data too short for common value".to_string(),
        ));
    }

    // Read common value
    let common_value = i64::from_le_bytes([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
    ]);

    let num_codes_bytes = (num_ints * 2).div_ceil(8);
    let codes_start = 8;
    let vints_start = codes_start + num_codes_bytes;

    if data.len() < vints_start {
        return Err(IntegerCodingError::InvalidFormat(
            "Data too short for codes".to_string(),
        ));
    }

    let mut result = Vec::with_capacity(num_ints);
    let mut prev: i64 = 0;
    let mut vints_offset = vints_start;

    for i in 0..num_ints {
        let code_byte_idx = i / 4;
        let code_bit_idx = i % 4;
        let code = (data[codes_start + code_byte_idx] >> (code_bit_idx * 2)) & 0x03;

        let delta: i64 = match code {
            CODE_COMMON => common_value,
            CODE_SMALL => {
                // For 64-bit, small is 16-bit
                if vints_offset + 2 > data.len() {
                    return Err(IntegerCodingError::InvalidFormat(
                        "Unexpected end of data".to_string(),
                    ));
                }
                let v = i16::from_le_bytes([data[vints_offset], data[vints_offset + 1]]) as i64;
                vints_offset += 2;
                v
            }
            CODE_MEDIUM => {
                // For 64-bit, medium is 32-bit
                if vints_offset + 4 > data.len() {
                    return Err(IntegerCodingError::InvalidFormat(
                        "Unexpected end of data".to_string(),
                    ));
                }
                let v = i32::from_le_bytes([
                    data[vints_offset],
                    data[vints_offset + 1],
                    data[vints_offset + 2],
                    data[vints_offset + 3],
                ]) as i64;
                vints_offset += 4;
                v
            }
            CODE_LARGE => {
                if vints_offset + 8 > data.len() {
                    return Err(IntegerCodingError::InvalidFormat(
                        "Unexpected end of data".to_string(),
                    ));
                }
                let v = i64::from_le_bytes([
                    data[vints_offset],
                    data[vints_offset + 1],
                    data[vints_offset + 2],
                    data[vints_offset + 3],
                    data[vints_offset + 4],
                    data[vints_offset + 5],
                    data[vints_offset + 6],
                    data[vints_offset + 7],
                ]);
                vints_offset += 8;
                v
            }
            _ => {
                return Err(IntegerCodingError::InvalidFormat(format!(
                    "Invalid code: {}",
                    code
                )));
            }
        };

        prev = prev.wrapping_add(delta);
        result.push(prev);
    }

    Ok(result)
}

/// Get 2-bit code for a 64-bit delta value.
fn get_code_64(delta: i64, common_value: i64) -> u8 {
    if delta == common_value {
        CODE_COMMON
    } else if delta >= i16::MIN as i64 && delta <= i16::MAX as i64 {
        CODE_SMALL
    } else if delta >= i32::MIN as i64 && delta <= i32::MAX as i64 {
        CODE_MEDIUM
    } else {
        CODE_LARGE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_i32_simple() {
        let input: Vec<i32> = vec![1, 2, 3, 4, 5];
        let encoded = encode_integers_i32(&input);
        let decoded = decode_integers_i32(&encoded, input.len()).unwrap();
        assert_eq!(input, decoded);
    }

    #[test]
    fn test_encode_decode_i32_with_gaps() {
        let input: Vec<i32> = vec![0, 100, 200, 300, 400];
        let encoded = encode_integers_i32(&input);
        let decoded = decode_integers_i32(&encoded, input.len()).unwrap();
        assert_eq!(input, decoded);
    }

    #[test]
    fn test_encode_decode_i32_negative() {
        let input: Vec<i32> = vec![-100, -50, 0, 50, 100];
        let encoded = encode_integers_i32(&input);
        let decoded = decode_integers_i32(&encoded, input.len()).unwrap();
        assert_eq!(input, decoded);
    }

    #[test]
    fn test_encode_decode_i32_large_values() {
        let input: Vec<i32> = vec![0, 100000, 200000, 100000, 0];
        let encoded = encode_integers_i32(&input);
        let decoded = decode_integers_i32(&encoded, input.len()).unwrap();
        assert_eq!(input, decoded);
    }

    #[test]
    fn test_compress_decompress_i32() {
        let input: Vec<i32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let compressed = IntegerCompression::compress_i32(&input).unwrap();
        let decompressed = IntegerCompression::decompress_i32(&compressed, input.len()).unwrap();
        assert_eq!(input, decompressed);
    }

    #[test]
    fn test_compress_decompress_u32() {
        let input: Vec<u32> = vec![1, 2, 3, 4, 5];
        let compressed = IntegerCompression::compress_u32(&input).unwrap();
        let decompressed = IntegerCompression::decompress_u32(&compressed, input.len()).unwrap();
        assert_eq!(input, decompressed);
    }

    #[test]
    fn test_encode_decode_i64_simple() {
        let input: Vec<i64> = vec![1, 2, 3, 4, 5];
        let encoded = encode_integers_i64(&input);
        let decoded = decode_integers_i64(&encoded, input.len()).unwrap();
        assert_eq!(input, decoded);
    }

    #[test]
    fn test_compress_decompress_i64() {
        let input: Vec<i64> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let compressed = IntegerCompression64::compress_i64(&input).unwrap();
        let decompressed = IntegerCompression64::decompress_i64(&compressed, input.len()).unwrap();
        assert_eq!(input, decompressed);
    }

    #[test]
    fn test_empty() {
        let empty_i32: Vec<i32> = vec![];
        let compressed = IntegerCompression::compress_i32(&empty_i32).unwrap();
        assert!(compressed.is_empty());

        let decompressed = IntegerCompression::decompress_i32(&compressed, 0).unwrap();
        assert!(decompressed.is_empty());
    }
}
