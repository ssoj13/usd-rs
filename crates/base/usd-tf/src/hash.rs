//! Hash utilities for USD.
//!
//! This module provides a user-extensible hashing mechanism for use with
//! runtime hash tables. It wraps the SpookyHash implementation from the
//! arch module and provides higher-level hash combination utilities.
//!
//! # Examples
//!
//! ```
//! use usd_tf::hash::{TfHash, hash_combine};
//!
//! // Hash a single value
//! let h = TfHash::hash(&42);
//!
//! // Combine multiple hash values
//! let combined = hash_combine(&[1u64, 2, 3]);
//!
//! // Use TfHash as a hasher builder
//! use std::collections::HashMap;
//! let mut map: HashMap<String, i32, TfHash> = HashMap::with_hasher(TfHash);
//! map.insert("key".to_string(), 42);
//! ```
//!
//! # Note on Hash Stability
//!
//! The hash functions here are appropriate for storing objects in runtime
//! hash tables. They are NOT appropriate for document signatures,
//! fingerprinting, or for storage and offline use. No guarantee is made
//! about repeatability from run-to-run.

use std::hash::{BuildHasher, Hash, Hasher};

use usd_arch::hash as arch_hash;

/// A hash function object for use with hash maps.
///
/// TfHash provides a fast, high-quality hash function suitable for runtime
/// hash tables. It uses SpookyHash V2 internally.
///
/// # Examples
///
/// ```
/// use usd_tf::hash::TfHash;
/// use std::collections::HashMap;
///
/// // Use as a BuildHasher
/// let mut map: HashMap<i32, &str, TfHash> = HashMap::with_hasher(TfHash);
/// map.insert(1, "one");
/// map.insert(2, "two");
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct TfHash;

impl TfHash {
    /// Hash a value using TfHash.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::hash::TfHash;
    ///
    /// let hash = TfHash::hash(&"hello");
    /// assert_ne!(hash, 0);
    /// ```
    #[must_use]
    pub fn hash<T: Hash + ?Sized>(value: &T) -> u64 {
        let mut hasher = TfHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    /// Combine multiple hash values into one.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::hash::TfHash;
    ///
    /// let h1 = TfHash::hash(&1);
    /// let h2 = TfHash::hash(&2);
    /// let combined = TfHash::combine(&[h1, h2]);
    /// ```
    #[must_use]
    pub fn combine(hashes: &[u64]) -> u64 {
        hash_combine(hashes)
    }
}

impl BuildHasher for TfHash {
    type Hasher = TfHasher;

    fn build_hasher(&self) -> Self::Hasher {
        TfHasher::new()
    }
}

/// A hasher implementation using SpookyHash V2.
///
/// This hasher accumulates bytes and produces a 64-bit hash code.
#[derive(Debug, Clone)]
pub struct TfHasher {
    state: u64,
    buffer: Vec<u8>,
}

impl TfHasher {
    /// Create a new hasher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: 0,
            buffer: Vec::new(),
        }
    }

    /// Create a new hasher with a seed.
    #[must_use]
    pub fn with_seed(seed: u64) -> Self {
        Self {
            state: seed,
            buffer: Vec::new(),
        }
    }
}

impl Default for TfHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher for TfHasher {
    fn finish(&self) -> u64 {
        if self.buffer.is_empty() {
            self.state
        } else {
            arch_hash::hash64_with_seed(&self.buffer, self.state)
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    fn write_u8(&mut self, i: u8) {
        self.buffer.push(i);
    }

    fn write_u16(&mut self, i: u16) {
        self.buffer.extend_from_slice(&i.to_le_bytes());
    }

    fn write_u32(&mut self, i: u32) {
        self.buffer.extend_from_slice(&i.to_le_bytes());
    }

    fn write_u64(&mut self, i: u64) {
        self.buffer.extend_from_slice(&i.to_le_bytes());
    }

    fn write_u128(&mut self, i: u128) {
        self.buffer.extend_from_slice(&i.to_le_bytes());
    }

    fn write_usize(&mut self, i: usize) {
        self.buffer.extend_from_slice(&i.to_le_bytes());
    }

    fn write_i8(&mut self, i: i8) {
        self.write_u8(i as u8);
    }

    fn write_i16(&mut self, i: i16) {
        self.write_u16(i as u16);
    }

    fn write_i32(&mut self, i: i32) {
        self.write_u32(i as u32);
    }

    fn write_i64(&mut self, i: i64) {
        self.write_u64(i as u64);
    }

    fn write_i128(&mut self, i: i128) {
        self.write_u128(i as u128);
    }

    fn write_isize(&mut self, i: isize) {
        self.write_usize(i as usize);
    }
}

/// Combine multiple hash values into a single hash.
///
/// This uses a technique based on triangular numbers to avoid collisions
/// when combining hash values that differ by a fixed amount.
///
/// # Examples
///
/// ```
/// use usd_tf::hash::hash_combine;
///
/// let h1 = 12345u64;
/// let h2 = 67890u64;
/// let combined = hash_combine(&[h1, h2]);
/// assert_ne!(combined, h1);
/// assert_ne!(combined, h2);
/// ```
#[must_use]
pub fn hash_combine(hashes: &[u64]) -> u64 {
    let mut result = 0u64;
    for &hash in hashes {
        result = combine_two(result, hash);
    }
    result
}

/// Combine two hash values.
///
/// Uses the triangular number technique for better distribution.
#[inline]
#[must_use]
pub fn combine_two(x: u64, y: u64) -> u64 {
    // This is based on the triangular number technique from OpenUSD.
    // See the detailed explanation in _ref/OpenUSD/pxr/base/tf/hash.h
    let sum = x.wrapping_add(y);
    y.wrapping_add(sum.wrapping_mul(sum.wrapping_add(1)) / 2)
}

/// Hash bytes directly using SpookyHash.
///
/// # Examples
///
/// ```
/// use usd_tf::hash::hash_bytes;
///
/// let data = b"hello world";
/// let h = hash_bytes(data);
/// assert_ne!(h, 0);
/// ```
#[must_use]
pub fn hash_bytes(bytes: &[u8]) -> u64 {
    arch_hash::hash64(bytes)
}

/// Hash bytes with a seed using SpookyHash.
///
/// # Examples
///
/// ```
/// use usd_tf::hash::hash_bytes_with_seed;
///
/// let data = b"hello world";
/// let h1 = hash_bytes_with_seed(data, 0);
/// let h2 = hash_bytes_with_seed(data, 42);
/// assert_ne!(h1, h2);
/// ```
#[must_use]
pub fn hash_bytes_with_seed(bytes: &[u8], seed: u64) -> u64 {
    arch_hash::hash64_with_seed(bytes, seed)
}

/// Hash a string using SpookyHash.
///
/// # Examples
///
/// ```
/// use usd_tf::hash::hash_str;
///
/// let h = hash_str("hello");
/// assert_ne!(h, 0);
/// ```
#[must_use]
pub fn hash_str(s: &str) -> u64 {
    arch_hash::hash64(s.as_bytes())
}

/// Hash a C-style null-terminated string.
///
/// This is equivalent to `hash_str` but makes the intent clear.
///
/// # Examples
///
/// ```
/// use usd_tf::hash::hash_cstr;
///
/// let h = hash_cstr("hello");
/// assert_eq!(h, usd_tf::hash::hash_str("hello"));
/// ```
#[must_use]
pub fn hash_cstr(s: &str) -> u64 {
    hash_str(s)
}

/// Hash a pointer value (the address, not the contents).
///
/// # Examples
///
/// ```
/// use usd_tf::hash::hash_ptr;
///
/// let x = 42;
/// let h = hash_ptr(&x);
/// assert_ne!(h, 0);
/// ```
#[must_use]
pub fn hash_ptr<T: ?Sized>(ptr: *const T) -> u64 {
    let addr = ptr as *const () as usize;
    arch_hash::hash64(&addr.to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_tf_hash_simple() {
        let h = TfHash::hash(&42);
        assert_ne!(h, 0);
    }

    #[test]
    fn test_tf_hash_string() {
        let h1 = TfHash::hash(&"hello");
        let h2 = TfHash::hash(&"world");
        assert_ne!(h1, h2);

        // Same string should hash the same
        let h3 = TfHash::hash(&"hello");
        assert_eq!(h1, h3);
    }

    #[test]
    fn test_tf_hash_combine() {
        let h1 = TfHash::hash(&1);
        let h2 = TfHash::hash(&2);
        let combined = TfHash::combine(&[h1, h2]);

        // Combined should be different from both inputs
        assert_ne!(combined, h1);
        assert_ne!(combined, h2);
    }

    #[test]
    fn test_tf_hash_as_build_hasher() {
        let mut map: HashMap<String, i32, TfHash> = HashMap::with_hasher(TfHash);
        map.insert("key1".to_string(), 1);
        map.insert("key2".to_string(), 2);

        assert_eq!(map.get("key1"), Some(&1));
        assert_eq!(map.get("key2"), Some(&2));
        assert_eq!(map.get("key3"), None);
    }

    #[test]
    fn test_tf_hasher() {
        let mut hasher = TfHasher::new();
        hasher.write(b"hello");
        let h1 = hasher.finish();

        let mut hasher = TfHasher::new();
        hasher.write(b"hello");
        let h2 = hasher.finish();

        assert_eq!(h1, h2);
    }

    #[test]
    fn test_tf_hasher_with_seed() {
        let mut hasher1 = TfHasher::with_seed(0);
        hasher1.write(b"hello");
        let h1 = hasher1.finish();

        let mut hasher2 = TfHasher::with_seed(42);
        hasher2.write(b"hello");
        let h2 = hasher2.finish();

        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_combine() {
        let combined = hash_combine(&[1, 2, 3]);
        assert_ne!(combined, 0);

        // Order matters
        let combined2 = hash_combine(&[3, 2, 1]);
        assert_ne!(combined, combined2);
    }

    #[test]
    fn test_combine_two() {
        let c1 = combine_two(100, 200);
        let c2 = combine_two(200, 100);
        assert_ne!(c1, c2);

        // Same values should produce same result
        let c3 = combine_two(100, 200);
        assert_eq!(c1, c3);
    }

    #[test]
    fn test_hash_bytes() {
        let h = hash_bytes(b"test data");
        assert_ne!(h, 0);

        // Same data should hash the same
        let h2 = hash_bytes(b"test data");
        assert_eq!(h, h2);

        // Different data should hash differently
        let h3 = hash_bytes(b"other data");
        assert_ne!(h, h3);
    }

    #[test]
    fn test_hash_str() {
        let h = hash_str("hello world");
        assert_ne!(h, 0);

        // Should match hash_bytes
        assert_eq!(h, hash_bytes(b"hello world"));
    }

    #[test]
    fn test_hash_ptr() {
        let x = 42;
        let y = 42;
        let h1 = hash_ptr(&x);
        let h2 = hash_ptr(&y);

        // Different addresses should produce different hashes
        assert_ne!(h1, h2);

        // Same address should produce same hash
        let h3 = hash_ptr(&x);
        assert_eq!(h1, h3);
    }

    #[test]
    fn test_hasher_write_integers() {
        let mut hasher = TfHasher::new();
        hasher.write_u8(1);
        hasher.write_u16(2);
        hasher.write_u32(3);
        hasher.write_u64(4);
        let h = hasher.finish();
        assert_ne!(h, 0);
    }

    #[test]
    fn test_hash_distribution() {
        // Test that similar values don't produce similar hashes
        let h1 = TfHash::hash(&1000);
        let h2 = TfHash::hash(&1001);
        let h3 = TfHash::hash(&1002);

        // The hashes should be well distributed
        assert_ne!(h1, h2);
        assert_ne!(h2, h3);
        assert_ne!(h1, h3);
    }
}
