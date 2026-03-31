//! Fast data compression/decompression using LZ4.
//!
//! This module provides simple, fast compression and decompression routines
//! using the LZ4 algorithm. It is designed for high-speed compression with
//! reasonable compression ratios.
//!
//! # Overview
//!
//! The compression format supports data larger than the LZ4 single-block limit
//! by splitting into chunks. The first byte indicates the number of chunks:
//! - 0 means single chunk (data fits in one LZ4 block)
//! - N > 0 means N chunks, each prefixed with a 4-byte compressed size
//!
//! # Examples
//!
//! ```
//! use usd_tf::fast_compression::FastCompression;
//!
//! let data = b"Hello, World! This is some test data for compression.";
//! let compressed = FastCompression::compress(data).unwrap();
//! let decompressed = FastCompression::decompress(&compressed, data.len()).unwrap();
//! assert_eq!(data.as_slice(), decompressed.as_slice());
//! ```

use pxr_lz4::{LZ4_MAX_INPUT_SIZE, Lz4Error};

/// Error type for compression/decompression operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompressionError {
    /// Input size exceeds the maximum allowed.
    InputTooLarge {
        /// Actual input size.
        size: usize,
        /// Maximum allowed size.
        max: usize,
    },
    /// Decompression failed (possibly corrupt data).
    DecompressionFailed(String),
    /// Output buffer too small.
    OutputBufferTooSmall {
        /// Required size.
        required: usize,
        /// Provided size.
        provided: usize,
    },
    /// Invalid compressed data format.
    InvalidFormat(String),
}

impl std::fmt::Display for CompressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputTooLarge { size, max } => {
                write!(f, "input size {} exceeds maximum {}", size, max)
            }
            Self::DecompressionFailed(msg) => {
                write!(f, "decompression failed: {}", msg)
            }
            Self::OutputBufferTooSmall { required, provided } => {
                write!(
                    f,
                    "output buffer too small: need {} bytes, got {}",
                    required, provided
                )
            }
            Self::InvalidFormat(msg) => {
                write!(f, "invalid compressed format: {}", msg)
            }
        }
    }
}

impl std::error::Error for CompressionError {}

impl From<Lz4Error> for CompressionError {
    fn from(e: Lz4Error) -> Self {
        match e {
            Lz4Error::CorruptInput(msg) => CompressionError::DecompressionFailed(msg),
            Lz4Error::OutputTooSmall { needed, capacity } => {
                CompressionError::OutputBufferTooSmall {
                    required: needed,
                    provided: capacity,
                }
            }
            Lz4Error::InputTooLarge { size, max } => CompressionError::InputTooLarge { size, max },
        }
    }
}

/// Fast LZ4 compression utilities.
///
/// Provides high-performance compression and decompression using the LZ4
/// algorithm. Supports data larger than the single-block limit by automatic
/// chunking.
pub struct FastCompression;

impl FastCompression {
    /// Returns the largest input buffer size that can be compressed.
    ///
    /// Guaranteed to be at least 200 GB.
    #[must_use]
    pub const fn max_input_size() -> usize {
        127 * LZ4_MAX_INPUT_SIZE
    }

    /// Returns the largest possible compressed size for the given input size.
    ///
    /// This is the worst-case size when data is incompressible. Returns `None`
    /// if `input_size` exceeds `max_input_size()`.
    #[must_use]
    pub fn compressed_buffer_size(input_size: usize) -> Option<usize> {
        pxr_lz4::tf_compressed_buffer_size(input_size)
    }

    /// Compresses data to a new buffer.
    ///
    /// Returns the compressed data or an error if the input is too large.
    pub fn compress(input: &[u8]) -> Result<Vec<u8>, CompressionError> {
        pxr_lz4::compress_tf_fast(input).map_err(Into::into)
    }

    /// Compresses data into the provided buffer.
    ///
    /// Returns the number of bytes written to the output buffer.
    pub fn compress_to_buffer(input: &[u8], output: &mut [u8]) -> Result<usize, CompressionError> {
        let compressed = Self::compress(input)?;

        if compressed.len() > output.len() {
            return Err(CompressionError::OutputBufferTooSmall {
                required: compressed.len(),
                provided: output.len(),
            });
        }

        output[..compressed.len()].copy_from_slice(&compressed);
        Ok(compressed.len())
    }

    /// Decompresses data to a new buffer.
    ///
    /// `max_output_size` is the maximum expected decompressed size.
    pub fn decompress(
        compressed: &[u8],
        max_output_size: usize,
    ) -> Result<Vec<u8>, CompressionError> {
        pxr_lz4::decompress_tf_fast(compressed, max_output_size).map_err(Into::into)
    }

    /// Decompresses data into the provided buffer.
    ///
    /// Returns the number of bytes written to the output buffer.
    pub fn decompress_to_buffer(
        compressed: &[u8],
        output: &mut [u8],
        max_output_size: usize,
    ) -> Result<usize, CompressionError> {
        let decompressed = Self::decompress(compressed, max_output_size)?;

        if decompressed.len() > output.len() {
            return Err(CompressionError::OutputBufferTooSmall {
                required: decompressed.len(),
                provided: output.len(),
            });
        }

        output[..decompressed.len()].copy_from_slice(&decompressed);
        Ok(decompressed.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_input_size() {
        let max = FastCompression::max_input_size();
        // Should be at least 200 GB
        assert!(max >= 200 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_compressed_buffer_size() {
        // Small size
        let size = FastCompression::compressed_buffer_size(100);
        assert!(size.is_some());
        assert!(size.unwrap() > 100);

        // Zero size
        let size = FastCompression::compressed_buffer_size(0);
        assert!(size.is_some());

        // Maximum size
        let max = FastCompression::max_input_size();
        let size = FastCompression::compressed_buffer_size(max);
        assert!(size.is_some());

        // Over maximum
        let size = FastCompression::compressed_buffer_size(max + 1);
        assert!(size.is_none());
    }

    #[test]
    fn test_roundtrip_small() {
        let data = b"Hello, World!";
        let compressed = FastCompression::compress(data).unwrap();
        let decompressed = FastCompression::decompress(&compressed, data.len()).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_roundtrip_empty() {
        let data: &[u8] = b"";
        let compressed = FastCompression::compress(data).unwrap();
        let decompressed = FastCompression::decompress(&compressed, 0).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    fn test_roundtrip_repeating() {
        let data = vec![0xABu8; 10000];
        let compressed = FastCompression::compress(&data).unwrap();
        let decompressed = FastCompression::decompress(&compressed, data.len()).unwrap();
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_roundtrip_random_like() {
        // Pseudo-random data (harder to compress)
        let mut data = vec![0u8; 10000];
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = ((i * 17 + 31) % 256) as u8;
        }
        let compressed = FastCompression::compress(&data).unwrap();
        let decompressed = FastCompression::decompress(&compressed, data.len()).unwrap();
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_compress_to_buffer() {
        let data = b"Test data for buffer compression";
        let mut buffer = vec![0u8; 200];
        let written = FastCompression::compress_to_buffer(data, &mut buffer).unwrap();
        assert!(written > 0);

        // Verify the compressed data works
        let decompressed = FastCompression::decompress(&buffer[..written], data.len()).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_decompress_to_buffer() {
        let original = b"Test data for buffer decompression";
        let compressed = FastCompression::compress(original).unwrap();
        let mut buffer = vec![0u8; 100];
        let written =
            FastCompression::decompress_to_buffer(&compressed, &mut buffer, original.len())
                .unwrap();
        assert_eq!(&buffer[..written], original);
    }

    #[test]
    fn test_decompress_invalid_empty() {
        let result = FastCompression::decompress(&[], 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_compression_error_display() {
        let err = CompressionError::InputTooLarge { size: 100, max: 50 };
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("50"));

        let err = CompressionError::DecompressionFailed("test".to_string());
        assert!(err.to_string().contains("test"));

        let err = CompressionError::OutputBufferTooSmall {
            required: 100,
            provided: 50,
        };
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("50"));

        let err = CompressionError::InvalidFormat("bad".to_string());
        assert!(err.to_string().contains("bad"));
    }

    #[test]
    fn test_output_buffer_too_small() {
        let data = b"Some data to compress that needs space";
        let mut small_buffer = vec![0u8; 5];
        let result = FastCompression::compress_to_buffer(data, &mut small_buffer);
        assert!(matches!(
            result,
            Err(CompressionError::OutputBufferTooSmall { .. })
        ));
    }

    #[test]
    fn test_large_data_single_chunk() {
        // Just under the chunk limit (use 1MB for testing)
        let data = vec![0x42u8; 1024 * 1024];
        let compressed = FastCompression::compress(&data).unwrap();
        // First byte should be 0 (single chunk, C++ "zero byte means one chunk")
        assert_eq!(compressed[0], 0);
        let decompressed = FastCompression::decompress(&compressed, data.len()).unwrap();
        assert_eq!(data, decompressed);
    }
}
