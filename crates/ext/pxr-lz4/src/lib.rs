//! Pure Rust LZ4 decompression compatible with OpenUSD TfFastCompression.
//!
//! This crate provides LZ4 decompression that matches the format used by
//! OpenUSD's TfFastCompression. Unlike lz4_flex::block::decompress which
//! requires knowing the exact output size, this implementation works like
//! LZ4_decompress_safe - it only needs max output size.

use std::cmp::min;

/// LZ4 minimum match length
const MINMATCH: usize = 4;

/// LZ4 maximum input size per block (~2GB)
pub const LZ4_MAX_INPUT_SIZE: usize = 0x7E000000;

/// Error type for LZ4 operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lz4Error {
    /// Input data is corrupted or invalid
    CorruptInput(String),
    /// Output buffer too small
    OutputTooSmall { needed: usize, capacity: usize },
    /// Input too large
    InputTooLarge { size: usize, max: usize },
}

impl std::fmt::Display for Lz4Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CorruptInput(msg) => write!(f, "corrupt LZ4 input: {}", msg),
            Self::OutputTooSmall { needed, capacity } => {
                write!(
                    f,
                    "output buffer too small: need {}, have {}",
                    needed, capacity
                )
            }
            Self::InputTooLarge { size, max } => {
                write!(f, "input too large: {} > {}", size, max)
            }
        }
    }
}

impl std::error::Error for Lz4Error {}

/// Decompress LZ4 block data.
///
/// This is equivalent to `LZ4_decompress_safe` from the C library.
/// It reads compressed LZ4 data and writes to the output buffer.
///
/// # Arguments
/// * `src` - Compressed LZ4 data (raw block, no frame header)
/// * `max_output_size` - Maximum bytes to write to output
///
/// # Returns
/// Decompressed data or error
pub fn decompress_safe(src: &[u8], max_output_size: usize) -> Result<Vec<u8>, Lz4Error> {
    if src.is_empty() {
        return Ok(Vec::new());
    }

    let mut output = Vec::with_capacity(min(max_output_size, src.len() * 4));
    let mut ip = 0; // input position
    let src_end = src.len();

    while ip < src_end {
        // Read token
        let token = src[ip];
        ip += 1;

        // Decode literal length (upper 4 bits)
        let mut literal_len = (token >> 4) as usize;
        if literal_len == 15 {
            // Extended length
            loop {
                if ip >= src_end {
                    return Err(Lz4Error::CorruptInput("truncated literal length".into()));
                }
                let s = src[ip] as usize;
                ip += 1;
                literal_len += s;
                if s != 255 {
                    break;
                }
            }
        }

        // Copy literals
        if literal_len > 0 {
            if ip + literal_len > src_end {
                return Err(Lz4Error::CorruptInput(format!(
                    "literal overrun: need {} at {}, have {}",
                    literal_len,
                    ip,
                    src_end - ip
                )));
            }
            if output.len() + literal_len > max_output_size {
                return Err(Lz4Error::OutputTooSmall {
                    needed: output.len() + literal_len,
                    capacity: max_output_size,
                });
            }
            output.extend_from_slice(&src[ip..ip + literal_len]);
            ip += literal_len;
        }

        // Check if this was the last sequence (no match follows)
        if ip >= src_end {
            break;
        }

        // Read match offset (2 bytes, little-endian)
        if ip + 2 > src_end {
            return Err(Lz4Error::CorruptInput("truncated match offset".into()));
        }
        let offset = u16::from_le_bytes([src[ip], src[ip + 1]]) as usize;
        ip += 2;

        if offset == 0 {
            return Err(Lz4Error::CorruptInput("zero match offset".into()));
        }
        if offset > output.len() {
            return Err(Lz4Error::CorruptInput(format!(
                "match offset {} > output len {}",
                offset,
                output.len()
            )));
        }

        // Decode match length (lower 4 bits + MINMATCH)
        let mut match_len = (token & 0x0F) as usize;
        if match_len == 15 {
            // Extended length
            loop {
                if ip >= src_end {
                    return Err(Lz4Error::CorruptInput("truncated match length".into()));
                }
                let s = src[ip] as usize;
                ip += 1;
                match_len += s;
                if s != 255 {
                    break;
                }
            }
        }
        match_len += MINMATCH;

        // Copy match (may overlap!)
        if output.len() + match_len > max_output_size {
            return Err(Lz4Error::OutputTooSmall {
                needed: output.len() + match_len,
                capacity: max_output_size,
            });
        }

        let match_start = output.len() - offset;
        // Handle overlapping copy byte by byte
        for i in 0..match_len {
            let b = output[match_start + i];
            output.push(b);
        }
    }

    Ok(output)
}

/// Decompress LZ4 data directly into provided buffer.
///
/// Decompresses `src` in-place into `dst` without an intermediate allocation.
/// Returns number of bytes written.
pub fn decompress_safe_into(src: &[u8], dst: &mut [u8]) -> Result<usize, Lz4Error> {
    if src.is_empty() {
        return Ok(0);
    }

    let max_output_size = dst.len();
    let mut written = 0usize;
    let mut ip = 0;
    let src_end = src.len();

    while ip < src_end {
        let token = src[ip];
        ip += 1;

        // Decode literal length
        let mut literal_len = (token >> 4) as usize;
        if literal_len == 15 {
            loop {
                if ip >= src_end {
                    return Err(Lz4Error::CorruptInput("truncated literal length".into()));
                }
                let s = src[ip] as usize;
                ip += 1;
                literal_len += s;
                if s != 255 {
                    break;
                }
            }
        }

        // Copy literals directly into dst
        if literal_len > 0 {
            if ip + literal_len > src_end {
                return Err(Lz4Error::CorruptInput(format!(
                    "literal overrun: need {} at {}, have {}",
                    literal_len,
                    ip,
                    src_end - ip
                )));
            }
            if written + literal_len > max_output_size {
                return Err(Lz4Error::OutputTooSmall {
                    needed: written + literal_len,
                    capacity: max_output_size,
                });
            }
            dst[written..written + literal_len].copy_from_slice(&src[ip..ip + literal_len]);
            written += literal_len;
            ip += literal_len;
        }

        if ip >= src_end {
            break;
        }

        // Read match offset
        if ip + 2 > src_end {
            return Err(Lz4Error::CorruptInput("truncated match offset".into()));
        }
        let offset = u16::from_le_bytes([src[ip], src[ip + 1]]) as usize;
        ip += 2;

        if offset == 0 {
            return Err(Lz4Error::CorruptInput("zero match offset".into()));
        }
        if offset > written {
            return Err(Lz4Error::CorruptInput(format!(
                "match offset {} > output len {}",
                offset, written
            )));
        }

        // Decode match length
        let mut match_len = (token & 0x0F) as usize;
        if match_len == 15 {
            loop {
                if ip >= src_end {
                    return Err(Lz4Error::CorruptInput("truncated match length".into()));
                }
                let s = src[ip] as usize;
                ip += 1;
                match_len += s;
                if s != 255 {
                    break;
                }
            }
        }
        match_len += MINMATCH;

        if written + match_len > max_output_size {
            return Err(Lz4Error::OutputTooSmall {
                needed: written + match_len,
                capacity: max_output_size,
            });
        }

        // Overlapping copy byte-by-byte directly in dst
        let match_start = written - offset;
        for i in 0..match_len {
            dst[written + i] = dst[match_start + i];
        }
        written += match_len;
    }

    Ok(written)
}

/// TfFastCompression format decompression.
///
/// OpenUSD's TfFastCompression uses a simple chunked format:
/// - First byte = number of chunks (0 means single chunk)
/// - If single chunk: rest is raw LZ4 data
/// - If multiple chunks: each chunk has 4-byte size prefix (i32 LE)
pub fn decompress_tf_fast(compressed: &[u8], max_output_size: usize) -> Result<Vec<u8>, Lz4Error> {
    if compressed.is_empty() {
        return Err(Lz4Error::CorruptInput("empty input".into()));
    }

    let n_chunks = compressed[0] as usize;
    let data = &compressed[1..];

    if n_chunks == 0 {
        // Single chunk - raw LZ4 block.
        // C++ writes compressed[0] = 0 for single chunk ("zero byte means one chunk").
        decompress_safe(data, max_output_size)
    } else {
        // Multiple chunks: each chunk has a 4-byte i32 size prefix, then LZ4 data.
        // C++ format: [nChunks] [size0] [lz4_0] [size1] [lz4_1] ...
        let mut output = Vec::with_capacity(max_output_size);
        let mut offset = 0;

        for i in 0..n_chunks {
            if offset + 4 > data.len() {
                return Err(Lz4Error::CorruptInput(format!(
                    "truncated chunk {} header at offset {}",
                    i, offset
                )));
            }

            let chunk_size_i32 = i32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;

            // Negative chunk size means corrupt data — would wrap to huge usize
            if chunk_size_i32 < 0 {
                return Err(Lz4Error::CorruptInput(format!(
                    "negative chunk {} size: {}",
                    i, chunk_size_i32
                )));
            }
            let chunk_size = chunk_size_i32 as usize;

            if offset + chunk_size > data.len() {
                return Err(Lz4Error::CorruptInput(format!(
                    "truncated chunk {} data: need {} at {}, have {}",
                    i,
                    chunk_size,
                    offset,
                    data.len() - offset
                )));
            }

            let chunk_data = &data[offset..offset + chunk_size];
            let max_chunk_output = min(LZ4_MAX_INPUT_SIZE, max_output_size - output.len());
            let decompressed = decompress_safe(chunk_data, max_chunk_output)?;

            if output.len() + decompressed.len() > max_output_size {
                return Err(Lz4Error::OutputTooSmall {
                    needed: output.len() + decompressed.len(),
                    capacity: max_output_size,
                });
            }

            output.extend_from_slice(&decompressed);
            offset += chunk_size;
        }

        Ok(output)
    }
}

/// Compress data using LZ4 (simple implementation).
///
/// Returns compressed data in TfFastCompression format.
pub fn compress_tf_fast(input: &[u8]) -> Result<Vec<u8>, Lz4Error> {
    if input.len() > 127 * LZ4_MAX_INPUT_SIZE {
        return Err(Lz4Error::InputTooLarge {
            size: input.len(),
            max: 127 * LZ4_MAX_INPUT_SIZE,
        });
    }

    // For simplicity, use a basic compression that stores literals only
    // This is valid LZ4 but not optimal - real compression would find matches

    if input.len() <= LZ4_MAX_INPUT_SIZE {
        // Single chunk: [0] [raw LZ4 data]
        // C++ writes compressed[0] = 0 for single chunk ("zero byte means one chunk").
        let compressed = compress_block(input);
        let mut output = Vec::with_capacity(1 + compressed.len());
        output.push(0); // Single chunk marker (matches C++)
        output.extend_from_slice(&compressed);
        Ok(output)
    } else {
        // Multiple chunks
        let n_whole_chunks = input.len() / LZ4_MAX_INPUT_SIZE;
        let part_chunk_sz = input.len() % LZ4_MAX_INPUT_SIZE;
        let n_chunks = n_whole_chunks + if part_chunk_sz > 0 { 1 } else { 0 };

        let mut output = Vec::new();
        output.push(n_chunks as u8);

        let mut offset = 0;
        for _ in 0..n_whole_chunks {
            let chunk = &input[offset..offset + LZ4_MAX_INPUT_SIZE];
            let compressed = compress_block(chunk);
            let size = compressed.len() as i32;
            output.extend_from_slice(&size.to_le_bytes());
            output.extend_from_slice(&compressed);
            offset += LZ4_MAX_INPUT_SIZE;
        }

        if part_chunk_sz > 0 {
            let chunk = &input[offset..];
            let compressed = compress_block(chunk);
            let size = compressed.len() as i32;
            output.extend_from_slice(&size.to_le_bytes());
            output.extend_from_slice(&compressed);
        }

        Ok(output)
    }
}

/// Compress a single LZ4 block using literal-only encoding.
///
/// # Limitation: No match finding
/// This implementation encodes all input bytes as a single literal sequence
/// (no LZ4 back-reference matches). Output is always >= input size.
/// This produces valid LZ4 blocks that any conformant decoder can read,
/// but achieves 0% compression ratio. For real compression, a match-finding
/// hash table (e.g. LZ4_HC or the standard 64KB hash chain) would be needed.
/// Adding match finding is left as a future enhancement.
fn compress_block(input: &[u8]) -> Vec<u8> {
    if input.is_empty() {
        return Vec::new();
    }

    let mut output = Vec::new();
    let literal_len = input.len();

    // Token: literal length in upper 4 bits, match length (0) in lower 4 bits
    // Since this is the final sequence (no match follows), we store all literals
    if literal_len < 15 {
        // Short literal - fits in token
        output.push((literal_len as u8) << 4);
    } else {
        // Extended literal length
        output.push(0xF0); // 15 << 4
        let mut len = literal_len - 15;
        while len >= 255 {
            output.push(255);
            len -= 255;
        }
        output.push(len as u8);
    }

    // Write all literals
    output.extend_from_slice(input);
    output
}

/// Returns the maximum compressed size for given input size.
pub fn compress_bound(input_size: usize) -> usize {
    // LZ4 worst case: input_size + (input_size / 255) + 16
    input_size + (input_size / 255) + 16
}

/// Returns maximum compressed buffer size for TfFastCompression format.
pub fn tf_compressed_buffer_size(input_size: usize) -> Option<usize> {
    if input_size > 127 * LZ4_MAX_INPUT_SIZE {
        return None;
    }

    if input_size <= LZ4_MAX_INPUT_SIZE {
        // 1 byte header + compressed data
        Some(1 + compress_bound(input_size))
    } else {
        let n_whole_chunks = input_size / LZ4_MAX_INPUT_SIZE;
        let part_chunk_sz = input_size % LZ4_MAX_INPUT_SIZE;

        let mut sz = 1 + n_whole_chunks * (compress_bound(LZ4_MAX_INPUT_SIZE) + 4);
        if part_chunk_sz > 0 {
            sz += compress_bound(part_chunk_sz) + 4;
        }
        Some(sz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_small() {
        let data = b"Hello, World!";
        let compressed = compress_tf_fast(data).unwrap();
        let decompressed = decompress_tf_fast(&compressed, 1024).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_roundtrip_empty() {
        let data: &[u8] = b"";
        let compressed = compress_tf_fast(data).unwrap();
        let decompressed = decompress_tf_fast(&compressed, 0).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    fn test_roundtrip_repeating() {
        let data = vec![0xABu8; 10000];
        let compressed = compress_tf_fast(&data).unwrap();
        let decompressed = decompress_tf_fast(&compressed, data.len()).unwrap();
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_decompress_lz4_flex_compressed() {
        // Test compatibility with lz4_flex compressed data
        use lz4_flex::block::compress;

        let data = b"Test data for compression roundtrip testing!";
        let compressed = compress(data);

        // Our decompressor should be able to decompress lz4_flex output
        let decompressed = decompress_safe(&compressed, 1024).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_decompress_lz4_flex_repeating() {
        use lz4_flex::block::compress;

        let data = vec![0x42u8; 1000];
        let compressed = compress(&data);

        let decompressed = decompress_safe(&compressed, 1024).unwrap();
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_tf_fast_format_single_chunk() {
        let data = b"Single chunk test data";
        let compressed = compress_tf_fast(data).unwrap();

        // First byte should be 0 (single chunk)
        assert_eq!(compressed[0], 0);

        let decompressed = decompress_tf_fast(&compressed, 1024).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_corrupt_input() {
        // Invalid LZ4 data
        let result = decompress_safe(&[0xFF, 0xFF], 1024);
        assert!(result.is_err());
    }

    #[test]
    fn test_compress_bound() {
        assert!(compress_bound(100) > 100);
        assert!(compress_bound(0) >= 16);
    }

    #[test]
    fn test_decompress_safe_into_no_alloc() {
        // Verify decompress_safe_into writes directly into dst without double-alloc
        use lz4_flex::block::compress;
        let data = b"Direct buffer decompression test!";
        let compressed = compress(data);
        let mut buf = vec![0u8; 128];
        let n = decompress_safe_into(&compressed, &mut buf).unwrap();
        assert_eq!(&buf[..n], data.as_slice());
    }

    #[test]
    fn test_negative_chunk_size_rejected() {
        // Craft a multi-chunk TfFast payload with a negative i32 chunk size.
        // Format: [n_chunks=1] [size: -1 as i32 LE] ...
        let mut bad = Vec::new();
        bad.push(1u8); // n_chunks = 1
        bad.extend_from_slice(&(-1i32).to_le_bytes()); // negative size
        let result = decompress_tf_fast(&bad, 1024);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("negative"),
            "expected 'negative' in error: {}",
            msg
        );
    }
}
