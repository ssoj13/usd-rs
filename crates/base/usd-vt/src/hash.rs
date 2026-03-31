//! Hash utilities for vt types.
//!
//! This module provides hash functions for vt value types.

use std::hash::{Hash, Hasher};

use super::{Array, Value};

/// Computes a hash value for a Value.
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, hash_value};
///
/// let v = Value::from(42i32);
/// let h = hash_value(&v);
/// ```
#[inline]
pub fn hash_value(value: &Value) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// Computes a hash value for an Array.
///
/// # Examples
///
/// ```
/// use usd_vt::{Array, hash_array};
///
/// let arr: Array<i32> = Array::from(vec![1, 2, 3]);
/// let h = hash_array(&arr);
/// ```
#[inline]
pub fn hash_array<T: Clone + Send + Sync + Hash + 'static>(array: &Array<T>) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    array.hash(&mut hasher);
    hasher.finish()
}

/// Combines two hash values.
///
/// Uses the same approach as boost::hash_combine.
///
/// # Examples
///
/// ```
/// use usd_vt::hash_combine;
///
/// let h1 = 12345u64;
/// let h2 = 67890u64;
/// let combined = hash_combine(h1, h2);
/// ```
#[inline]
#[must_use]
pub fn hash_combine(seed: u64, value: u64) -> u64 {
    seed ^ (value
        .wrapping_add(0x9e3779b9)
        .wrapping_add(seed << 6)
        .wrapping_add(seed >> 2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_value() {
        let v1 = Value::from(42i32);
        let v2 = Value::from(42i32);
        let v3 = Value::from(43i32);

        assert_eq!(hash_value(&v1), hash_value(&v2));
        assert_ne!(hash_value(&v1), hash_value(&v3));
    }

    #[test]
    fn test_hash_array() {
        let arr1: Array<i32> = Array::from(vec![1, 2, 3]);
        let arr2: Array<i32> = Array::from(vec![1, 2, 3]);
        let arr3: Array<i32> = Array::from(vec![1, 2, 4]);

        assert_eq!(hash_array(&arr1), hash_array(&arr2));
        assert_ne!(hash_array(&arr1), hash_array(&arr3));
    }

    #[test]
    fn test_hash_combine() {
        let h1 = hash_combine(0, 12345);
        let h2 = hash_combine(0, 12345);
        let h3 = hash_combine(0, 54321);

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }
}
