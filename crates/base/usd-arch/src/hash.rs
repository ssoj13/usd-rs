//! Hash functions - SpookyHash V2 implementation.
//!
//! SpookyHash is a fast, non-cryptographic hash function designed by Bob Jenkins.
//! This is a faithful port of the SpookyHash V2 algorithm used in OpenUSD.
//!
//! # Features
//!
//! - Fast: ~3 bytes/cycle for long messages
//! - Good avalanche: all bits affected by all input bits
//! - Non-cryptographic: not suitable for security purposes
//!
//! # Examples
//!
//! ```
//! use usd_arch::{hash32, hash64, hash32_with_seed};
//!
//! let data = b"hello world";
//! let h32 = hash32(data);
//! let h64 = hash64(data);
//!
//! // With seed
//! let h32_seeded = hash32_with_seed(data, 42);
//! ```

/// Magic constant for SpookyHash
const SC_CONST: u64 = 0xdeadbeefdeadbeef;

/// Number of u64s in internal state
const SC_NUM_VARS: usize = 12;

/// Block size in bytes
const SC_BLOCK_SIZE: usize = SC_NUM_VARS * 8;

/// Buffer size for short messages
const SC_BUF_SIZE: usize = 2 * SC_BLOCK_SIZE;

/// Left rotate a 64-bit value
#[inline(always)]
const fn rot64(x: u64, k: u32) -> u64 {
    x.rotate_left(k)
}

/// Mix function for processing 96-byte blocks
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn mix(data: &[u64; SC_NUM_VARS], s: &mut [u64; SC_NUM_VARS]) {
    s[0] = s[0].wrapping_add(data[0]);
    s[2] ^= s[10];
    s[11] ^= s[0];
    s[0] = rot64(s[0], 11);
    s[11] = s[11].wrapping_add(s[1]);

    s[1] = s[1].wrapping_add(data[1]);
    s[3] ^= s[11];
    s[0] ^= s[1];
    s[1] = rot64(s[1], 32);
    s[0] = s[0].wrapping_add(s[2]);

    s[2] = s[2].wrapping_add(data[2]);
    s[4] ^= s[0];
    s[1] ^= s[2];
    s[2] = rot64(s[2], 43);
    s[1] = s[1].wrapping_add(s[3]);

    s[3] = s[3].wrapping_add(data[3]);
    s[5] ^= s[1];
    s[2] ^= s[3];
    s[3] = rot64(s[3], 31);
    s[2] = s[2].wrapping_add(s[4]);

    s[4] = s[4].wrapping_add(data[4]);
    s[6] ^= s[2];
    s[3] ^= s[4];
    s[4] = rot64(s[4], 17);
    s[3] = s[3].wrapping_add(s[5]);

    s[5] = s[5].wrapping_add(data[5]);
    s[7] ^= s[3];
    s[4] ^= s[5];
    s[5] = rot64(s[5], 28);
    s[4] = s[4].wrapping_add(s[6]);

    s[6] = s[6].wrapping_add(data[6]);
    s[8] ^= s[4];
    s[5] ^= s[6];
    s[6] = rot64(s[6], 39);
    s[5] = s[5].wrapping_add(s[7]);

    s[7] = s[7].wrapping_add(data[7]);
    s[9] ^= s[5];
    s[6] ^= s[7];
    s[7] = rot64(s[7], 57);
    s[6] = s[6].wrapping_add(s[8]);

    s[8] = s[8].wrapping_add(data[8]);
    s[10] ^= s[6];
    s[7] ^= s[8];
    s[8] = rot64(s[8], 55);
    s[7] = s[7].wrapping_add(s[9]);

    s[9] = s[9].wrapping_add(data[9]);
    s[11] ^= s[7];
    s[8] ^= s[9];
    s[9] = rot64(s[9], 54);
    s[8] = s[8].wrapping_add(s[10]);

    s[10] = s[10].wrapping_add(data[10]);
    s[0] ^= s[8];
    s[9] ^= s[10];
    s[10] = rot64(s[10], 22);
    s[9] = s[9].wrapping_add(s[11]);

    s[11] = s[11].wrapping_add(data[11]);
    s[1] ^= s[9];
    s[10] ^= s[11];
    s[11] = rot64(s[11], 46);
    s[10] = s[10].wrapping_add(s[0]);
}

/// End partial mixing
#[inline(always)]
fn end_partial(h: &mut [u64; SC_NUM_VARS]) {
    h[11] = h[11].wrapping_add(h[1]);
    h[2] ^= h[11];
    h[1] = rot64(h[1], 44);

    h[0] = h[0].wrapping_add(h[2]);
    h[3] ^= h[0];
    h[2] = rot64(h[2], 15);

    h[1] = h[1].wrapping_add(h[3]);
    h[4] ^= h[1];
    h[3] = rot64(h[3], 34);

    h[2] = h[2].wrapping_add(h[4]);
    h[5] ^= h[2];
    h[4] = rot64(h[4], 21);

    h[3] = h[3].wrapping_add(h[5]);
    h[6] ^= h[3];
    h[5] = rot64(h[5], 38);

    h[4] = h[4].wrapping_add(h[6]);
    h[7] ^= h[4];
    h[6] = rot64(h[6], 33);

    h[5] = h[5].wrapping_add(h[7]);
    h[8] ^= h[5];
    h[7] = rot64(h[7], 10);

    h[6] = h[6].wrapping_add(h[8]);
    h[9] ^= h[6];
    h[8] = rot64(h[8], 13);

    h[7] = h[7].wrapping_add(h[9]);
    h[10] ^= h[7];
    h[9] = rot64(h[9], 38);

    h[8] = h[8].wrapping_add(h[10]);
    h[11] ^= h[8];
    h[10] = rot64(h[10], 53);

    h[9] = h[9].wrapping_add(h[11]);
    h[0] ^= h[9];
    h[11] = rot64(h[11], 42);

    h[10] = h[10].wrapping_add(h[0]);
    h[1] ^= h[10];
    h[0] = rot64(h[0], 54);
}

/// End mixing
fn end(data: &[u64; SC_NUM_VARS], h: &mut [u64; SC_NUM_VARS]) {
    for i in 0..SC_NUM_VARS {
        h[i] = h[i].wrapping_add(data[i]);
    }
    end_partial(h);
    end_partial(h);
    end_partial(h);
}

/// Short mix for messages under 192 bytes
#[inline(always)]
fn short_mix(h: &mut [u64; 4]) {
    h[2] = rot64(h[2], 50);
    h[2] = h[2].wrapping_add(h[3]);
    h[0] ^= h[2];

    h[3] = rot64(h[3], 52);
    h[3] = h[3].wrapping_add(h[0]);
    h[1] ^= h[3];

    h[0] = rot64(h[0], 30);
    h[0] = h[0].wrapping_add(h[1]);
    h[2] ^= h[0];

    h[1] = rot64(h[1], 41);
    h[1] = h[1].wrapping_add(h[2]);
    h[3] ^= h[1];

    h[2] = rot64(h[2], 54);
    h[2] = h[2].wrapping_add(h[3]);
    h[0] ^= h[2];

    h[3] = rot64(h[3], 48);
    h[3] = h[3].wrapping_add(h[0]);
    h[1] ^= h[3];

    h[0] = rot64(h[0], 38);
    h[0] = h[0].wrapping_add(h[1]);
    h[2] ^= h[0];

    h[1] = rot64(h[1], 37);
    h[1] = h[1].wrapping_add(h[2]);
    h[3] ^= h[1];

    h[2] = rot64(h[2], 62);
    h[2] = h[2].wrapping_add(h[3]);
    h[0] ^= h[2];

    h[3] = rot64(h[3], 34);
    h[3] = h[3].wrapping_add(h[0]);
    h[1] ^= h[3];

    h[0] = rot64(h[0], 5);
    h[0] = h[0].wrapping_add(h[1]);
    h[2] ^= h[0];

    h[1] = rot64(h[1], 36);
    h[1] = h[1].wrapping_add(h[2]);
    h[3] ^= h[1];
}

/// Short end mixing
#[inline(always)]
fn short_end(h: &mut [u64; 4]) {
    h[3] ^= h[2];
    h[2] = rot64(h[2], 15);
    h[3] = h[3].wrapping_add(h[2]);

    h[0] ^= h[3];
    h[3] = rot64(h[3], 52);
    h[0] = h[0].wrapping_add(h[3]);

    h[1] ^= h[0];
    h[0] = rot64(h[0], 26);
    h[1] = h[1].wrapping_add(h[0]);

    h[2] ^= h[1];
    h[1] = rot64(h[1], 51);
    h[2] = h[2].wrapping_add(h[1]);

    h[3] ^= h[2];
    h[2] = rot64(h[2], 28);
    h[3] = h[3].wrapping_add(h[2]);

    h[0] ^= h[3];
    h[3] = rot64(h[3], 9);
    h[0] = h[0].wrapping_add(h[3]);

    h[1] ^= h[0];
    h[0] = rot64(h[0], 47);
    h[1] = h[1].wrapping_add(h[0]);

    h[2] ^= h[1];
    h[1] = rot64(h[1], 54);
    h[2] = h[2].wrapping_add(h[1]);

    h[3] ^= h[2];
    h[2] = rot64(h[2], 32);
    h[3] = h[3].wrapping_add(h[2]);

    h[0] ^= h[3];
    h[3] = rot64(h[3], 25);
    h[0] = h[0].wrapping_add(h[3]);

    h[1] ^= h[0];
    h[0] = rot64(h[0], 63);
    h[1] = h[1].wrapping_add(h[0]);
}

/// Read a little-endian u64 from a byte slice
#[inline(always)]
fn read_u64_le(data: &[u8], offset: usize) -> u64 {
    let bytes: [u8; 8] = data[offset..offset + 8].try_into().unwrap_or([0; 8]);
    u64::from_le_bytes(bytes)
}

/// Read a little-endian u32 from a byte slice
#[inline(always)]
fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    let bytes: [u8; 4] = data[offset..offset + 4].try_into().unwrap_or([0; 4]);
    u32::from_le_bytes(bytes)
}

/// Short hash for messages under 192 bytes
fn hash_short(message: &[u8], hash1: &mut u64, hash2: &mut u64) {
    let length = message.len();

    let mut h = [*hash1, *hash2, SC_CONST, SC_CONST];

    if length > 15 {
        let end = (length / 32) * 4;
        let mut pos = 0;

        while pos < end {
            h[2] = h[2].wrapping_add(read_u64_le(message, pos * 8));
            h[3] = h[3].wrapping_add(read_u64_le(message, pos * 8 + 8));
            short_mix(&mut h);
            h[0] = h[0].wrapping_add(read_u64_le(message, pos * 8 + 16));
            h[1] = h[1].wrapping_add(read_u64_le(message, pos * 8 + 24));
            pos += 4;
        }

        let remainder_start = end * 8;
        let mut remainder = length - remainder_start;

        if remainder >= 16 {
            h[2] = h[2].wrapping_add(read_u64_le(message, remainder_start));
            h[3] = h[3].wrapping_add(read_u64_le(message, remainder_start + 8));
            short_mix(&mut h);
            remainder -= 16;
        }

        // Handle remaining 0-15 bytes
        let offset = length - remainder;
        h[3] = h[3].wrapping_add((length as u64) << 56);

        match remainder {
            15 => {
                h[3] = h[3].wrapping_add((message[offset + 14] as u64) << 48);
                h[3] = h[3].wrapping_add((message[offset + 13] as u64) << 40);
                h[3] = h[3].wrapping_add((message[offset + 12] as u64) << 32);
                h[3] = h[3].wrapping_add(read_u32_le(message, offset + 8) as u64);
                h[2] = h[2].wrapping_add(read_u64_le(message, offset));
            }
            14 => {
                h[3] = h[3].wrapping_add((message[offset + 13] as u64) << 40);
                h[3] = h[3].wrapping_add((message[offset + 12] as u64) << 32);
                h[3] = h[3].wrapping_add(read_u32_le(message, offset + 8) as u64);
                h[2] = h[2].wrapping_add(read_u64_le(message, offset));
            }
            13 => {
                h[3] = h[3].wrapping_add((message[offset + 12] as u64) << 32);
                h[3] = h[3].wrapping_add(read_u32_le(message, offset + 8) as u64);
                h[2] = h[2].wrapping_add(read_u64_le(message, offset));
            }
            12 => {
                h[3] = h[3].wrapping_add(read_u32_le(message, offset + 8) as u64);
                h[2] = h[2].wrapping_add(read_u64_le(message, offset));
            }
            11 => {
                h[3] = h[3].wrapping_add((message[offset + 10] as u64) << 16);
                h[3] = h[3].wrapping_add((message[offset + 9] as u64) << 8);
                h[3] = h[3].wrapping_add(message[offset + 8] as u64);
                h[2] = h[2].wrapping_add(read_u64_le(message, offset));
            }
            10 => {
                h[3] = h[3].wrapping_add((message[offset + 9] as u64) << 8);
                h[3] = h[3].wrapping_add(message[offset + 8] as u64);
                h[2] = h[2].wrapping_add(read_u64_le(message, offset));
            }
            9 => {
                h[3] = h[3].wrapping_add(message[offset + 8] as u64);
                h[2] = h[2].wrapping_add(read_u64_le(message, offset));
            }
            8 => {
                h[2] = h[2].wrapping_add(read_u64_le(message, offset));
            }
            7 => {
                h[2] = h[2].wrapping_add((message[offset + 6] as u64) << 48);
                h[2] = h[2].wrapping_add((message[offset + 5] as u64) << 40);
                h[2] = h[2].wrapping_add((message[offset + 4] as u64) << 32);
                h[2] = h[2].wrapping_add(read_u32_le(message, offset) as u64);
            }
            6 => {
                h[2] = h[2].wrapping_add((message[offset + 5] as u64) << 40);
                h[2] = h[2].wrapping_add((message[offset + 4] as u64) << 32);
                h[2] = h[2].wrapping_add(read_u32_le(message, offset) as u64);
            }
            5 => {
                h[2] = h[2].wrapping_add((message[offset + 4] as u64) << 32);
                h[2] = h[2].wrapping_add(read_u32_le(message, offset) as u64);
            }
            4 => {
                h[2] = h[2].wrapping_add(read_u32_le(message, offset) as u64);
            }
            3 => {
                h[2] = h[2].wrapping_add((message[offset + 2] as u64) << 16);
                h[2] = h[2].wrapping_add((message[offset + 1] as u64) << 8);
                h[2] = h[2].wrapping_add(message[offset] as u64);
            }
            2 => {
                h[2] = h[2].wrapping_add((message[offset + 1] as u64) << 8);
                h[2] = h[2].wrapping_add(message[offset] as u64);
            }
            1 => {
                h[2] = h[2].wrapping_add(message[offset] as u64);
            }
            0 => {
                h[2] = h[2].wrapping_add(SC_CONST);
                h[3] = h[3].wrapping_add(SC_CONST);
            }
            _ => {}
        }
    } else {
        // Very short message (0-15 bytes)
        h[3] = h[3].wrapping_add((length as u64) << 56);

        match length {
            15 => {
                h[3] = h[3].wrapping_add((message[14] as u64) << 48);
                h[3] = h[3].wrapping_add((message[13] as u64) << 40);
                h[3] = h[3].wrapping_add((message[12] as u64) << 32);
                h[3] = h[3].wrapping_add(read_u32_le(message, 8) as u64);
                h[2] = h[2].wrapping_add(read_u64_le(message, 0));
            }
            14 => {
                h[3] = h[3].wrapping_add((message[13] as u64) << 40);
                h[3] = h[3].wrapping_add((message[12] as u64) << 32);
                h[3] = h[3].wrapping_add(read_u32_le(message, 8) as u64);
                h[2] = h[2].wrapping_add(read_u64_le(message, 0));
            }
            13 => {
                h[3] = h[3].wrapping_add((message[12] as u64) << 32);
                h[3] = h[3].wrapping_add(read_u32_le(message, 8) as u64);
                h[2] = h[2].wrapping_add(read_u64_le(message, 0));
            }
            12 => {
                h[3] = h[3].wrapping_add(read_u32_le(message, 8) as u64);
                h[2] = h[2].wrapping_add(read_u64_le(message, 0));
            }
            11 => {
                h[3] = h[3].wrapping_add((message[10] as u64) << 16);
                h[3] = h[3].wrapping_add((message[9] as u64) << 8);
                h[3] = h[3].wrapping_add(message[8] as u64);
                h[2] = h[2].wrapping_add(read_u64_le(message, 0));
            }
            10 => {
                h[3] = h[3].wrapping_add((message[9] as u64) << 8);
                h[3] = h[3].wrapping_add(message[8] as u64);
                h[2] = h[2].wrapping_add(read_u64_le(message, 0));
            }
            9 => {
                h[3] = h[3].wrapping_add(message[8] as u64);
                h[2] = h[2].wrapping_add(read_u64_le(message, 0));
            }
            8 => {
                h[2] = h[2].wrapping_add(read_u64_le(message, 0));
            }
            7 => {
                h[2] = h[2].wrapping_add((message[6] as u64) << 48);
                h[2] = h[2].wrapping_add((message[5] as u64) << 40);
                h[2] = h[2].wrapping_add((message[4] as u64) << 32);
                h[2] = h[2].wrapping_add(read_u32_le(message, 0) as u64);
            }
            6 => {
                h[2] = h[2].wrapping_add((message[5] as u64) << 40);
                h[2] = h[2].wrapping_add((message[4] as u64) << 32);
                h[2] = h[2].wrapping_add(read_u32_le(message, 0) as u64);
            }
            5 => {
                h[2] = h[2].wrapping_add((message[4] as u64) << 32);
                h[2] = h[2].wrapping_add(read_u32_le(message, 0) as u64);
            }
            4 => {
                h[2] = h[2].wrapping_add(read_u32_le(message, 0) as u64);
            }
            3 => {
                h[2] = h[2].wrapping_add((message[2] as u64) << 16);
                h[2] = h[2].wrapping_add((message[1] as u64) << 8);
                h[2] = h[2].wrapping_add(message[0] as u64);
            }
            2 => {
                h[2] = h[2].wrapping_add((message[1] as u64) << 8);
                h[2] = h[2].wrapping_add(message[0] as u64);
            }
            1 => {
                h[2] = h[2].wrapping_add(message[0] as u64);
            }
            0 => {
                h[2] = h[2].wrapping_add(SC_CONST);
                h[3] = h[3].wrapping_add(SC_CONST);
            }
            _ => {}
        }
    }

    short_end(&mut h);
    *hash1 = h[0];
    *hash2 = h[1];
}

/// Computes a 128-bit hash of the given data.
///
/// # Arguments
///
/// * `message` - The data to hash
/// * `hash1` - In: first seed, Out: first 64 bits of hash
/// * `hash2` - In: second seed, Out: second 64 bits of hash
pub fn hash128(message: &[u8], hash1: &mut u64, hash2: &mut u64) {
    let length = message.len();

    if length < SC_BUF_SIZE {
        hash_short(message, hash1, hash2);
        return;
    }

    let mut h = [0u64; SC_NUM_VARS];
    h[0] = *hash1;
    h[3] = *hash1;
    h[6] = *hash1;
    h[9] = *hash1;
    h[1] = *hash2;
    h[4] = *hash2;
    h[7] = *hash2;
    h[10] = *hash2;
    h[2] = SC_CONST;
    h[5] = SC_CONST;
    h[8] = SC_CONST;
    h[11] = SC_CONST;

    let num_blocks = length / SC_BLOCK_SIZE;
    let mut pos = 0;

    // Process full blocks
    for _ in 0..num_blocks {
        let mut data = [0u64; SC_NUM_VARS];
        for (i, slot) in data.iter_mut().enumerate() {
            *slot = read_u64_le(message, pos + i * 8);
        }
        mix(&data, &mut h);
        pos += SC_BLOCK_SIZE;
    }

    // Handle the last partial block
    let remainder = length - pos;
    let mut buf = [0u64; SC_NUM_VARS];

    // Copy remaining bytes
    for i in 0..remainder {
        let byte_pos = i % 8;
        let word_pos = i / 8;
        buf[word_pos] |= (message[pos + i] as u64) << (byte_pos * 8);
    }

    // Set the length in the last byte
    let last_word = (SC_BLOCK_SIZE - 1) / 8;
    buf[last_word] &= !(0xFFu64 << 56);
    buf[last_word] |= (remainder as u64) << 56;

    end(&buf, &mut h);
    *hash1 = h[0];
    *hash2 = h[1];
}

/// Computes a 64-bit hash of the given data.
///
/// # Examples
///
/// ```
/// use usd_arch::hash64;
///
/// let data = b"hello world";
/// let h = hash64(data);
/// ```
#[inline]
#[must_use]
pub fn hash64(data: &[u8]) -> u64 {
    hash64_with_seed(data, 0)
}

/// Computes a 64-bit hash with a seed value.
///
/// # Examples
///
/// ```
/// use usd_arch::hash64_with_seed;
///
/// let data = b"hello world";
/// let h = hash64_with_seed(data, 42);
/// ```
#[must_use]
pub fn hash64_with_seed(data: &[u8], seed: u64) -> u64 {
    let mut hash1 = seed;
    let mut hash2 = seed;
    hash128(data, &mut hash1, &mut hash2);
    hash1
}

/// Computes a 32-bit hash of the given data.
///
/// # Examples
///
/// ```
/// use usd_arch::hash32;
///
/// let data = b"hello world";
/// let h = hash32(data);
/// ```
#[inline]
#[must_use]
pub fn hash32(data: &[u8]) -> u32 {
    hash32_with_seed(data, 0)
}

/// Computes a 32-bit hash with a seed value.
///
/// # Examples
///
/// ```
/// use usd_arch::hash32_with_seed;
///
/// let data = b"hello world";
/// let h = hash32_with_seed(data, 42);
/// ```
#[must_use]
pub fn hash32_with_seed(data: &[u8], seed: u32) -> u32 {
    let mut hash1 = seed as u64;
    let mut hash2 = seed as u64;
    hash128(data, &mut hash1, &mut hash2);
    hash1 as u32
}

/// A hasher that implements the SpookyHash algorithm.
///
/// This can be used with `std::collections::HashMap` and `HashSet`.
#[derive(Clone)]
pub struct SpookyHasher {
    seed1: u64,
    seed2: u64,
    buffer: Vec<u8>,
}

impl SpookyHasher {
    /// Creates a new hasher with the given seeds.
    #[must_use]
    pub fn new(seed1: u64, seed2: u64) -> Self {
        Self {
            seed1,
            seed2,
            buffer: Vec::with_capacity(256),
        }
    }

    /// Creates a new hasher with default seeds.
    #[must_use]
    pub fn new_default() -> Self {
        Self::new(0, 0)
    }
}

impl Default for SpookyHasher {
    fn default() -> Self {
        Self::new_default()
    }
}

impl std::hash::Hasher for SpookyHasher {
    fn finish(&self) -> u64 {
        let mut h1 = self.seed1;
        let mut h2 = self.seed2;
        hash128(&self.buffer, &mut h1, &mut h2);
        h1
    }

    fn write(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }
}

/// A builder for creating `SpookyHasher` instances.
#[derive(Clone, Default)]
pub struct SpookyHasherBuilder {
    seed1: u64,
    seed2: u64,
}

impl SpookyHasherBuilder {
    /// Creates a new builder with the given seeds.
    #[must_use]
    pub fn new(seed1: u64, seed2: u64) -> Self {
        Self { seed1, seed2 }
    }
}

impl std::hash::BuildHasher for SpookyHasherBuilder {
    type Hasher = SpookyHasher;

    fn build_hasher(&self) -> Self::Hasher {
        SpookyHasher::new(self.seed1, self.seed2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash32_empty() {
        let h = hash32(b"");
        assert_ne!(h, 0); // Empty string should still produce a hash
    }

    #[test]
    fn test_hash32_short() {
        let h1 = hash32(b"hello");
        let h2 = hash32(b"hello");
        assert_eq!(h1, h2); // Same input should produce same hash

        let h3 = hash32(b"world");
        assert_ne!(h1, h3); // Different input should produce different hash
    }

    #[test]
    fn test_hash64_consistency() {
        let data = b"The quick brown fox jumps over the lazy dog";
        let h1 = hash64(data);
        let h2 = hash64(data);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_with_seed() {
        let data = b"test data";
        let h1 = hash64_with_seed(data, 0);
        let h2 = hash64_with_seed(data, 42);
        assert_ne!(h1, h2); // Different seeds should produce different hashes
    }

    #[test]
    fn test_hash_long_message() {
        // Test with a message longer than SC_BUF_SIZE (192 bytes)
        let data: Vec<u8> = (0..500).map(|i| (i % 256) as u8).collect();
        let h = hash64(&data);
        assert_ne!(h, 0);
    }

    #[test]
    fn test_hasher_trait() {
        use std::hash::Hasher;

        let mut hasher = SpookyHasher::new_default();
        hasher.write(b"hello");
        hasher.write(b" ");
        hasher.write(b"world");
        let h1 = hasher.finish();

        let h2 = hash64(b"hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_builder() {
        use std::collections::HashMap;

        let builder = SpookyHasherBuilder::new(12345, 67890);
        let mut map: HashMap<String, i32, _> = HashMap::with_hasher(builder);
        map.insert("key".to_string(), 42);
        assert_eq!(map.get("key"), Some(&42));
    }
}
