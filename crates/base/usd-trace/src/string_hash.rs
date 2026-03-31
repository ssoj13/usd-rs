//! Compile-time string hashing.
//!
//! Port of pxr/base/trace/stringHash.h

/// Compile-time string hash computation.
///
/// Provides a const fn to compute DJB2 hash at compile time for string literals.
pub struct StringHash;

impl StringHash {
    /// Computes a compile-time hash of a string slice using DJB2 algorithm.
    ///
    /// This is a const fn so it can be evaluated at compile time.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_trace::StringHash;
    ///
    /// const HASH: u32 = StringHash::hash("my_function");
    /// ```
    pub const fn hash(s: &str) -> u32 {
        Self::djb2_hash(s.as_bytes())
    }

    /// DJB2 hash implementation (XOR variant).
    ///
    /// This is the same algorithm as used in C++ USD.
    const fn djb2_hash(bytes: &[u8]) -> u32 {
        let mut hash: u32 = 5381;
        let mut i = 0;
        while i < bytes.len() {
            hash = hash.wrapping_mul(33) ^ (bytes[i] as u32);
            i += 1;
        }
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_consistency() {
        const HASH1: u32 = StringHash::hash("test");
        const HASH2: u32 = StringHash::hash("test");
        assert_eq!(HASH1, HASH2);
    }

    #[test]
    fn test_hash_different_strings() {
        const HASH1: u32 = StringHash::hash("foo");
        const HASH2: u32 = StringHash::hash("bar");
        assert_ne!(HASH1, HASH2);
    }

    #[test]
    fn test_hash_empty_string() {
        const HASH: u32 = StringHash::hash("");
        assert_eq!(HASH, 5381); // Initial DJB2 value
    }

    #[test]
    fn test_compile_time() {
        // This test verifies that hash can be computed at compile time
        const _COMPILE_TIME_HASH: u32 = StringHash::hash("compile_time_test");
    }
}
