//! Bit manipulation utilities.
//!
//! Provides low-level bit manipulation functions for counting bits,
//! finding bit positions, and other bitwise operations.
//!
//! # Examples
//!
//! ```
//! use usd_tf::bit_utils::*;
//!
//! // Count set bits
//! assert_eq!(tf_count_bits(0b10110u64), 3);
//!
//! // Find lowest set bit position
//! assert_eq!(tf_find_lowest_bit(0b10100u64), Some(2));
//!
//! // Find highest set bit position  
//! assert_eq!(tf_find_highest_bit(0b10100u64), Some(4));
//! ```

/// Count the number of set (1) bits in a value.
///
/// # Examples
///
/// ```
/// use usd_tf::bit_utils::tf_count_bits;
///
/// assert_eq!(tf_count_bits(0b1011u64), 3);
/// assert_eq!(tf_count_bits(0u64), 0);
/// assert_eq!(tf_count_bits(u64::MAX), 64);
/// ```
#[inline]
pub fn tf_count_bits(value: u64) -> u32 {
    value.count_ones()
}

/// Count the number of set bits in a u32.
#[inline]
pub fn tf_count_bits32(value: u32) -> u32 {
    value.count_ones()
}

/// Find the position of the lowest set bit (0-indexed).
///
/// Returns `None` if no bits are set.
///
/// # Examples
///
/// ```
/// use usd_tf::bit_utils::tf_find_lowest_bit;
///
/// assert_eq!(tf_find_lowest_bit(0b10100u64), Some(2));
/// assert_eq!(tf_find_lowest_bit(0b1u64), Some(0));
/// assert_eq!(tf_find_lowest_bit(0u64), None);
/// ```
#[inline]
pub fn tf_find_lowest_bit(value: u64) -> Option<u32> {
    if value == 0 {
        None
    } else {
        Some(value.trailing_zeros())
    }
}

/// Find the position of the lowest set bit in a u32.
#[inline]
pub fn tf_find_lowest_bit32(value: u32) -> Option<u32> {
    if value == 0 {
        None
    } else {
        Some(value.trailing_zeros())
    }
}

/// Find the position of the highest set bit (0-indexed).
///
/// Returns `None` if no bits are set.
///
/// # Examples
///
/// ```
/// use usd_tf::bit_utils::tf_find_highest_bit;
///
/// assert_eq!(tf_find_highest_bit(0b10100u64), Some(4));
/// assert_eq!(tf_find_highest_bit(0b1u64), Some(0));
/// assert_eq!(tf_find_highest_bit(0u64), None);
/// ```
#[inline]
pub fn tf_find_highest_bit(value: u64) -> Option<u32> {
    if value == 0 {
        None
    } else {
        Some(63 - value.leading_zeros())
    }
}

/// Find the position of the highest set bit in a u32.
#[inline]
pub fn tf_find_highest_bit32(value: u32) -> Option<u32> {
    if value == 0 {
        None
    } else {
        Some(31 - value.leading_zeros())
    }
}

/// Check if a value is a power of two.
///
/// # Examples
///
/// ```
/// use usd_tf::bit_utils::tf_is_power_of_two;
///
/// assert!(tf_is_power_of_two(1));
/// assert!(tf_is_power_of_two(2));
/// assert!(tf_is_power_of_two(1024));
/// assert!(!tf_is_power_of_two(0));
/// assert!(!tf_is_power_of_two(3));
/// ```
#[inline]
pub fn tf_is_power_of_two(value: u64) -> bool {
    value != 0 && (value & (value - 1)) == 0
}

/// Check if a u32 value is a power of two.
#[inline]
pub fn tf_is_power_of_two32(value: u32) -> bool {
    value != 0 && (value & (value - 1)) == 0
}

/// Round up to the next power of two.
///
/// Returns the input if it's already a power of two.
/// Returns 0 on overflow.
///
/// # Examples
///
/// ```
/// use usd_tf::bit_utils::tf_next_power_of_two;
///
/// assert_eq!(tf_next_power_of_two(0), 1);
/// assert_eq!(tf_next_power_of_two(1), 1);
/// assert_eq!(tf_next_power_of_two(5), 8);
/// assert_eq!(tf_next_power_of_two(1024), 1024);
/// ```
#[inline]
pub fn tf_next_power_of_two(value: u64) -> u64 {
    if value == 0 {
        return 1;
    }
    value.next_power_of_two()
}

/// Round up to the next power of two for u32.
#[inline]
pub fn tf_next_power_of_two32(value: u32) -> u32 {
    if value == 0 {
        return 1;
    }
    value.next_power_of_two()
}

/// Get a bitmask with the lowest n bits set.
///
/// # Examples
///
/// ```
/// use usd_tf::bit_utils::tf_low_bits_mask;
///
/// assert_eq!(tf_low_bits_mask(0), 0);
/// assert_eq!(tf_low_bits_mask(1), 0b1);
/// assert_eq!(tf_low_bits_mask(4), 0b1111);
/// assert_eq!(tf_low_bits_mask(64), u64::MAX);
/// ```
#[inline]
pub fn tf_low_bits_mask(n: u32) -> u64 {
    if n >= 64 {
        u64::MAX
    } else if n == 0 {
        0
    } else {
        (1u64 << n) - 1
    }
}

/// Get a bitmask with the lowest n bits set for u32.
#[inline]
pub fn tf_low_bits_mask32(n: u32) -> u32 {
    if n >= 32 {
        u32::MAX
    } else if n == 0 {
        0
    } else {
        (1u32 << n) - 1
    }
}

/// Extract bits from a value.
///
/// Returns the bits from position `start` to `start + count - 1`.
///
/// # Examples
///
/// ```
/// use usd_tf::bit_utils::tf_extract_bits;
///
/// assert_eq!(tf_extract_bits(0b11010110u64, 1, 4), 0b1011);
/// ```
#[inline]
pub fn tf_extract_bits(value: u64, start: u32, count: u32) -> u64 {
    if count == 0 || start >= 64 {
        return 0;
    }
    let mask = tf_low_bits_mask(count);
    (value >> start) & mask
}

/// Extract bits from a u32 value.
#[inline]
pub fn tf_extract_bits32(value: u32, start: u32, count: u32) -> u32 {
    if count == 0 || start >= 32 {
        return 0;
    }
    let mask = tf_low_bits_mask32(count);
    (value >> start) & mask
}

/// Reverse the bits in a u64.
#[inline]
pub fn tf_reverse_bits(value: u64) -> u64 {
    value.reverse_bits()
}

/// Reverse the bits in a u32.
#[inline]
pub fn tf_reverse_bits32(value: u32) -> u32 {
    value.reverse_bits()
}

/// Rotate bits left.
#[inline]
pub fn tf_rotate_left(value: u64, n: u32) -> u64 {
    value.rotate_left(n)
}

/// Rotate bits left for u32.
#[inline]
pub fn tf_rotate_left32(value: u32, n: u32) -> u32 {
    value.rotate_left(n)
}

/// Rotate bits right.
#[inline]
pub fn tf_rotate_right(value: u64, n: u32) -> u64 {
    value.rotate_right(n)
}

/// Rotate bits right for u32.
#[inline]
pub fn tf_rotate_right32(value: u32, n: u32) -> u32 {
    value.rotate_right(n)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_bits() {
        assert_eq!(tf_count_bits(0), 0);
        assert_eq!(tf_count_bits(1), 1);
        assert_eq!(tf_count_bits(0b1111), 4);
        assert_eq!(tf_count_bits(0b10101010), 4);
        assert_eq!(tf_count_bits(u64::MAX), 64);
    }

    #[test]
    fn test_count_bits32() {
        assert_eq!(tf_count_bits32(0), 0);
        assert_eq!(tf_count_bits32(0b1111), 4);
        assert_eq!(tf_count_bits32(u32::MAX), 32);
    }

    #[test]
    fn test_find_lowest_bit() {
        assert_eq!(tf_find_lowest_bit(0), None);
        assert_eq!(tf_find_lowest_bit(1), Some(0));
        assert_eq!(tf_find_lowest_bit(0b100), Some(2));
        assert_eq!(tf_find_lowest_bit(0b1010), Some(1));
    }

    #[test]
    fn test_find_highest_bit() {
        assert_eq!(tf_find_highest_bit(0), None);
        assert_eq!(tf_find_highest_bit(1), Some(0));
        assert_eq!(tf_find_highest_bit(0b100), Some(2));
        assert_eq!(tf_find_highest_bit(0b1010), Some(3));
        assert_eq!(tf_find_highest_bit(1u64 << 63), Some(63));
    }

    #[test]
    fn test_is_power_of_two() {
        assert!(!tf_is_power_of_two(0));
        assert!(tf_is_power_of_two(1));
        assert!(tf_is_power_of_two(2));
        assert!(!tf_is_power_of_two(3));
        assert!(tf_is_power_of_two(4));
        assert!(tf_is_power_of_two(1 << 30));
    }

    #[test]
    fn test_next_power_of_two() {
        assert_eq!(tf_next_power_of_two(0), 1);
        assert_eq!(tf_next_power_of_two(1), 1);
        assert_eq!(tf_next_power_of_two(2), 2);
        assert_eq!(tf_next_power_of_two(3), 4);
        assert_eq!(tf_next_power_of_two(5), 8);
        assert_eq!(tf_next_power_of_two(1000), 1024);
    }

    #[test]
    fn test_low_bits_mask() {
        assert_eq!(tf_low_bits_mask(0), 0);
        assert_eq!(tf_low_bits_mask(1), 1);
        assert_eq!(tf_low_bits_mask(4), 0b1111);
        assert_eq!(tf_low_bits_mask(8), 0xFF);
        assert_eq!(tf_low_bits_mask(64), u64::MAX);
    }

    #[test]
    fn test_extract_bits() {
        // 0b11010110 = 214
        assert_eq!(tf_extract_bits(0b11010110, 0, 4), 0b0110);
        assert_eq!(tf_extract_bits(0b11010110, 1, 4), 0b1011);
        assert_eq!(tf_extract_bits(0b11010110, 4, 4), 0b1101);
    }

    #[test]
    fn test_reverse_bits() {
        assert_eq!(tf_reverse_bits32(0b00000001), 0x80000000);
        assert_eq!(tf_reverse_bits32(0b10000000), 0x01000000);
    }

    #[test]
    fn test_rotate() {
        assert_eq!(tf_rotate_left32(0b0001, 1), 0b0010);
        assert_eq!(tf_rotate_left32(0x80000000, 1), 0x00000001);
        assert_eq!(tf_rotate_right32(0b0010, 1), 0b0001);
        assert_eq!(tf_rotate_right32(0x00000001, 1), 0x80000000);
    }
}
