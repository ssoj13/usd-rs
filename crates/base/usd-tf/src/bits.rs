//! Fast bit array with tracking of set bits.
//!
//! This module provides:
//! - [`Bits`] - A fast bit array that tracks the number of bits set
//!   and can efficiently find the next/previous set bit.
//! - [`bits_for_values`] - Compute the number of bits required to store N values.
//!
//! # Thread Safety
//!
//! `Bits` supports only basic thread safety: multiple threads may safely
//! call const methods concurrently. A thread must not invoke any non-const
//! method while any other thread is accessing it.
//!
//! # Examples
//!
//! ```
//! use usd_tf::bits::Bits;
//!
//! let mut bits = Bits::new(100);
//! bits.set(10);
//! bits.set(50);
//! bits.set(90);
//!
//! assert_eq!(bits.get_num_set(), 3);
//! assert_eq!(bits.get_first_set(), 10);
//! assert_eq!(bits.get_last_set(), 90);
//!
//! // Iterate over set bits
//! for idx in bits.iter_set() {
//!     println!("Bit {} is set", idx);
//! }
//! ```
//!
//! # Bit Utilities
//!
//! ```
//! use usd_tf::bits::{bits_for_values, bits_for_enum_values};
//!
//! // 8 values (0-7) require 3 bits
//! assert_eq!(bits_for_values(8), 3);
//!
//! // For signed enum storage, add 1 bit for sign
//! assert_eq!(bits_for_enum_values(8), 4);
//! ```

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Index, Sub, SubAssign,
};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::hash::TfHasher;

// ============================================================================
// Bit Utilities (from bitUtils.h)
// ============================================================================

/// Compute the number of bits required to store the given number of values.
///
/// For example, `bits_for_values(8)` returns 3, since 3 bits can store
/// values 0-7 (8 possible values).
///
/// # Panics
///
/// Panics if `n` is 0.
///
/// # Examples
///
/// ```
/// use usd_tf::bits::bits_for_values;
///
/// assert_eq!(bits_for_values(1), 0);  // 0 bits for 1 value (just 0)
/// assert_eq!(bits_for_values(2), 1);  // 1 bit for 2 values (0, 1)
/// assert_eq!(bits_for_values(4), 2);  // 2 bits for 4 values (0-3)
/// assert_eq!(bits_for_values(8), 3);  // 3 bits for 8 values (0-7)
/// assert_eq!(bits_for_values(100), 7); // 7 bits for 100 values
/// ```
#[inline]
pub const fn bits_for_values(n: usize) -> usize {
    assert!(n > 0, "bits_for_values: n must be positive");
    if n == 1 {
        0
    } else {
        // Number of bits = floor(log2(n-1)) + 1
        usize::BITS as usize - (n - 1).leading_zeros() as usize
    }
}

/// Compute the number of bits required to store the given number of
/// signed enum values.
///
/// This adds 1 bit to [`bits_for_values`] to account for the sign bit
/// that some compilers use for enum bitfields.
///
/// # Panics
///
/// Panics if `n` is 0.
///
/// # Examples
///
/// ```
/// use usd_tf::bits::bits_for_enum_values;
///
/// assert_eq!(bits_for_enum_values(8), 4);  // 3 bits + 1 sign bit
/// assert_eq!(bits_for_enum_values(4), 3);  // 2 bits + 1 sign bit
/// ```
#[inline]
pub const fn bits_for_enum_values(n: usize) -> usize {
    bits_for_values(n) + 1
}

// ============================================================================
// Bits - Fast bit array
// ============================================================================

/// Sentinel value indicating an uninitialized cache.
const INVALID: usize = usize::MAX;

/// A fast bit array with tracking of set bits.
///
/// This structure maintains a bit array with:
/// - Cached count of set bits
/// - Cached first and last set bit indices
/// - Inline storage for arrays up to 64 bits
/// - Efficient iteration over set/unset bits
///
/// # Examples
///
/// ```
/// use usd_tf::bits::Bits;
///
/// let mut bits = Bits::new(64);
/// bits.set_all();
/// assert!(bits.are_all_set());
///
/// bits.clear(0);
/// assert!(!bits.are_all_set());
/// assert_eq!(bits.get_first_set(), 1);
/// ```
pub struct Bits {
    /// Total number of bits.
    num: usize,
    /// Cached number of set bits (INVALID when unknown).
    num_set: AtomicUsize,
    /// Cached index of first set bit (INVALID when unknown).
    first_set: AtomicUsize,
    /// Cached index of last set bit (INVALID when unknown).
    last_set: AtomicUsize,
    /// Number of u64 words.
    num_words: usize,
    /// The bit storage - either inline or heap.
    storage: Storage,
}

/// Storage for bit data - either inline (up to 64 bits) or heap-allocated.
enum Storage {
    /// Inline storage for up to 64 bits.
    Inline(u64),
    /// Heap-allocated storage for more than 64 bits.
    Heap(Box<[u64]>),
}

impl Bits {
    /// Creates a new bit array with all bits cleared.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::bits::Bits;
    ///
    /// let bits = Bits::new(100);
    /// assert_eq!(bits.get_size(), 100);
    /// assert!(bits.are_all_unset());
    /// ```
    pub fn new(num: usize) -> Self {
        let mut bits = Self {
            num: 0,
            num_set: AtomicUsize::new(INVALID),
            first_set: AtomicUsize::new(INVALID),
            last_set: AtomicUsize::new(INVALID),
            num_words: 0,
            storage: Storage::Inline(0),
        };
        bits.resize(num);
        bits.clear_all();
        bits
    }

    /// Creates a new bit array with a range of bits set.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::bits::Bits;
    ///
    /// let bits = Bits::with_range(10, 2, 5);
    /// assert!(bits.is_set(2));
    /// assert!(bits.is_set(5));
    /// assert!(!bits.is_set(1));
    /// assert!(!bits.is_set(6));
    /// ```
    pub fn with_range(num: usize, first: usize, last: usize) -> Self {
        let mut bits = Self::new(num);
        if num == 0 {
            return bits;
        }
        if first == 0 && last >= num - 1 {
            bits.set_all();
        } else {
            for i in first..=last.min(num - 1) {
                bits.set(i);
            }
        }
        bits
    }

    /// Resizes the bit array. Bits are left uninitialized.
    ///
    /// After calling this, you should typically call `clear_all()` or `set_all()`.
    pub fn resize(&mut self, num: usize) {
        if self.num == num {
            return;
        }

        self.num = num;
        self.num_set.store(INVALID, Ordering::Relaxed);
        self.first_set.store(INVALID, Ordering::Relaxed);
        self.last_set.store(INVALID, Ordering::Relaxed);
        self.num_words = (num + 63) >> 6;

        if self.num_words <= 1 {
            self.storage = Storage::Inline(0);
        } else {
            self.storage = Storage::Heap(vec![0u64; self.num_words].into_boxed_slice());
        }
    }

    /// Resizes while keeping content. New bits are cleared.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::bits::Bits;
    ///
    /// let mut bits = Bits::new(64);
    /// bits.set(10);
    /// bits.set(20);
    ///
    /// bits.resize_keep_content(128);
    /// assert!(bits.is_set(10));
    /// assert!(bits.is_set(20));
    /// assert!(!bits.is_set(100));
    /// ```
    pub fn resize_keep_content(&mut self, num: usize) {
        if num == self.num {
            return;
        }

        let mut temp = Self::new(num);
        let words_to_copy = temp.num_words.min(self.num_words);

        // Copy word data
        for i in 0..words_to_copy {
            temp.set_word(i, self.get_word(i));
        }

        if num < self.num {
            // Clear trailing bits in new array
            temp.clear_trailing_bits();
            temp.num_set.store(INVALID, Ordering::Relaxed);
            temp.first_set.store(INVALID, Ordering::Relaxed);
            temp.last_set.store(INVALID, Ordering::Relaxed);
        } else {
            // Keep cached info if valid
            let num_set = self.num_set.load(Ordering::Relaxed);
            temp.num_set.store(num_set, Ordering::Relaxed);

            let first = self.first_set.load(Ordering::Relaxed);
            if first != INVALID && first < self.num {
                temp.first_set.store(first, Ordering::Relaxed);
            } else {
                temp.first_set.store(num, Ordering::Relaxed);
            }

            let last = self.last_set.load(Ordering::Relaxed);
            if last != INVALID && last < self.num {
                temp.last_set.store(last, Ordering::Relaxed);
            } else {
                temp.last_set.store(num, Ordering::Relaxed);
            }
        }

        *self = temp;
    }

    /// Swaps contents with another `Bits`.
    pub fn swap(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.num, &mut other.num);
        std::mem::swap(&mut self.num_words, &mut other.num_words);
        std::mem::swap(&mut self.storage, &mut other.storage);

        // Swap atomic values non-atomically (only safe when no concurrent access)
        let ns = self.num_set.load(Ordering::Relaxed);
        let ons = other.num_set.load(Ordering::Relaxed);
        self.num_set.store(ons, Ordering::Relaxed);
        other.num_set.store(ns, Ordering::Relaxed);

        let fs = self.first_set.load(Ordering::Relaxed);
        let ofs = other.first_set.load(Ordering::Relaxed);
        self.first_set.store(ofs, Ordering::Relaxed);
        other.first_set.store(fs, Ordering::Relaxed);

        let ls = self.last_set.load(Ordering::Relaxed);
        let ols = other.last_set.load(Ordering::Relaxed);
        self.last_set.store(ols, Ordering::Relaxed);
        other.last_set.store(ls, Ordering::Relaxed);
    }

    /// Clears all bits to zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::bits::Bits;
    ///
    /// let mut bits = Bits::new(100);
    /// bits.set_all();
    /// bits.clear_all();
    /// assert!(bits.are_all_unset());
    /// ```
    pub fn clear_all(&mut self) {
        match &mut self.storage {
            Storage::Inline(data) => *data = 0,
            Storage::Heap(data) => data.fill(0),
        }
        self.num_set.store(0, Ordering::Relaxed);
        self.first_set.store(self.num, Ordering::Relaxed);
        self.last_set.store(self.num, Ordering::Relaxed);
    }

    /// Sets all bits to one.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::bits::Bits;
    ///
    /// let mut bits = Bits::new(100);
    /// bits.set_all();
    /// assert!(bits.are_all_set());
    /// ```
    pub fn set_all(&mut self) {
        match &mut self.storage {
            Storage::Inline(data) => *data = u64::MAX,
            Storage::Heap(data) => data.fill(u64::MAX),
        }
        self.num_set.store(self.num, Ordering::Relaxed);
        self.first_set.store(0, Ordering::Relaxed);
        self.last_set.store(
            if self.num > 0 { self.num - 1 } else { 0 },
            Ordering::Relaxed,
        );
        self.clear_trailing_bits();
    }

    /// Clears the bit at the given index.
    ///
    /// # Panics
    ///
    /// Panics if `index >= get_size()`.
    pub fn clear(&mut self, index: usize) {
        assert!(
            index < self.num,
            "index {} out of bounds (size {})",
            index,
            self.num
        );

        let word_idx = index >> 6;
        let bit_idx = index & 63;
        let mask = 1u64 << bit_idx;

        let word = self.get_word(word_idx);
        if word & mask != 0 {
            // Bit is set, clear it
            let num_set = self.num_set.load(Ordering::Relaxed);
            if num_set != INVALID && num_set > 0 {
                self.num_set.fetch_sub(1, Ordering::Relaxed);
            } else if num_set == INVALID {
                // Leave invalid
            }

            if index == self.first_set.load(Ordering::Relaxed) {
                self.first_set.store(INVALID, Ordering::Relaxed);
            }
            if index == self.last_set.load(Ordering::Relaxed) {
                self.last_set.store(INVALID, Ordering::Relaxed);
            }

            self.set_word(word_idx, word ^ mask);
        }
    }

    /// Sets the bit at the given index.
    ///
    /// # Panics
    ///
    /// Panics if `index >= get_size()`.
    pub fn set(&mut self, index: usize) {
        assert!(
            index < self.num,
            "index {} out of bounds (size {})",
            index,
            self.num
        );

        let word_idx = index >> 6;
        let bit_idx = index & 63;
        let mask = 1u64 << bit_idx;

        let word = self.get_word(word_idx);
        if word & mask == 0 {
            // Bit is not set, set it
            let num_set = self.num_set.load(Ordering::Relaxed);
            if num_set != INVALID {
                self.num_set.fetch_add(1, Ordering::Relaxed);
            }

            let first = self.first_set.load(Ordering::Relaxed);
            if index < first || first == INVALID {
                self.first_set.store(index, Ordering::Relaxed);
            }

            let last = self.last_set.load(Ordering::Relaxed);
            if index > last || last == self.num || last == INVALID {
                self.last_set.store(index, Ordering::Relaxed);
            }

            self.set_word(word_idx, word | mask);
        }
    }

    /// Assigns a value to the bit at the given index.
    pub fn assign(&mut self, index: usize, val: bool) {
        if val {
            self.set(index);
        } else {
            self.clear(index);
        }
    }

    /// Returns true if the bit at the given index is set.
    ///
    /// # Panics
    ///
    /// Panics if `index >= get_size()`.
    pub fn is_set(&self, index: usize) -> bool {
        assert!(
            index < self.num,
            "index {} out of bounds (size {})",
            index,
            self.num
        );
        let word = self.get_word(index >> 6);
        (word & (1u64 << (index & 63))) != 0
    }

    /// Finds the next set bit at or after the given index.
    ///
    /// Returns `get_size()` if no more set bits are found.
    pub fn find_next_set(&self, index: usize) -> usize {
        if index >= self.num {
            return self.num;
        }

        let start_bit = index & 63;
        let word = self.get_word(index >> 6);

        // Check if current bit is set
        if word & (1u64 << start_bit) != 0 {
            return index;
        }

        self.find_next_set_impl(index, start_bit)
    }

    /// Internal implementation of find_next_set using trailing_zeros for O(1) per-word scan.
    fn find_next_set_impl(&self, index: usize, start_bit: usize) -> usize {
        let first_word = index >> 6;

        // Mask off bits below start_bit in the first word
        let first_bits = self.get_word(first_word) >> start_bit;
        if first_bits != 0 {
            let bit = first_bits.trailing_zeros() as usize + start_bit + (first_word << 6);
            return if bit >= self.num { self.num } else { bit };
        }

        // Scan subsequent words
        for w in (first_word + 1)..self.num_words {
            let bits = self.get_word(w);
            if bits != 0 {
                let bit = bits.trailing_zeros() as usize + (w << 6);
                return if bit >= self.num { self.num } else { bit };
            }
        }
        self.num
    }

    /// Finds the previous set bit at or before the given index.
    ///
    /// Returns `get_size()` if no set bits are found.
    pub fn find_prev_set(&self, index: usize) -> usize {
        if index >= self.num {
            return self.num;
        }

        let start_bit = index & 63;
        let word = self.get_word(index >> 6);

        // Check if current bit is set
        if word & (1u64 << start_bit) != 0 {
            return index;
        }

        self.find_prev_set_impl(index, start_bit)
    }

    /// Internal implementation of find_prev_set using leading_zeros for O(1) per-word scan.
    fn find_prev_set_impl(&self, index: usize, start_bit: usize) -> usize {
        let first_word = index >> 6;

        // Mask off bits above start_bit in the first word
        let mask = if start_bit < 63 {
            (1u64 << (start_bit + 1)) - 1
        } else {
            u64::MAX
        };
        let first_bits = self.get_word(first_word) & mask;
        if first_bits != 0 {
            let bit = 63 - first_bits.leading_zeros() as usize + (first_word << 6);
            return bit;
        }

        // Scan preceding words
        for w in (0..first_word).rev() {
            let bits = self.get_word(w);
            if bits != 0 {
                let bit = 63 - bits.leading_zeros() as usize + (w << 6);
                return bit;
            }
        }
        self.num
    }

    /// Finds the next unset bit at or after the given index.
    ///
    /// Returns `get_size()` if no more unset bits are found.
    pub fn find_next_unset(&self, index: usize) -> usize {
        if index >= self.num {
            return self.num;
        }

        let start_bit = index & 63;
        let word = self.get_word(index >> 6);

        // Check if current bit is unset
        if word & (1u64 << start_bit) == 0 {
            return index;
        }

        self.find_next_unset_impl(index, start_bit)
    }

    /// Internal implementation of find_next_unset using trailing_zeros for O(1) per-word scan.
    fn find_next_unset_impl(&self, index: usize, start_bit: usize) -> usize {
        let first_word = index >> 6;

        // Mask off bits below start_bit in the inverted first word
        let first_bits = (!self.get_word(first_word)) >> start_bit;
        if first_bits != 0 {
            let bit = first_bits.trailing_zeros() as usize + start_bit + (first_word << 6);
            return if bit >= self.num { self.num } else { bit };
        }

        // Scan subsequent words
        for w in (first_word + 1)..self.num_words {
            let bits = !self.get_word(w);
            if bits != 0 {
                let bit = bits.trailing_zeros() as usize + (w << 6);
                return if bit >= self.num { self.num } else { bit };
            }
        }
        self.num
    }

    /// Returns the size of the bit array.
    #[inline]
    pub fn get_size(&self) -> usize {
        self.num
    }

    /// Returns true if the bit array is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.num == 0
    }

    /// Returns the index of the first set bit, or `get_size()` if none.
    pub fn get_first_set(&self) -> usize {
        let first = self.first_set.load(Ordering::Relaxed);
        if first == INVALID {
            let computed = self.find_next_set(0);
            self.first_set.store(computed, Ordering::Relaxed);
            computed
        } else {
            first
        }
    }

    /// Returns the index of the last set bit, or `get_size()` if none.
    pub fn get_last_set(&self) -> usize {
        let last = self.last_set.load(Ordering::Relaxed);
        if last == INVALID {
            let computed = if self.num == 0 {
                self.num
            } else {
                self.find_prev_set(self.num - 1)
            };
            self.last_set.store(computed, Ordering::Relaxed);
            computed
        } else {
            last
        }
    }

    /// Returns the number of bits currently set.
    pub fn get_num_set(&self) -> usize {
        let num_set = self.num_set.load(Ordering::Relaxed);
        if num_set == INVALID {
            let computed = self.count_num_set();
            self.num_set.store(computed, Ordering::Relaxed);
            computed
        } else {
            num_set
        }
    }

    /// Counts the number of set bits.
    fn count_num_set(&self) -> usize {
        let first = self.get_first_set();
        let last = self.get_last_set();

        if first >= self.num {
            return 0;
        }

        let offset = first >> 6;
        let end_word = (last >> 6) + 1;

        let mut count = 0usize;
        for w in offset..end_word {
            count += self.get_word(w).count_ones() as usize;
        }
        count
    }

    /// Returns true if all bits are set.
    #[inline]
    pub fn are_all_set(&self) -> bool {
        self.get_num_set() == self.get_size()
    }

    /// Returns true if all bits are unset.
    #[inline]
    pub fn are_all_unset(&self) -> bool {
        !self.is_any_set()
    }

    /// Returns true if at least one bit is set.
    #[inline]
    pub fn is_any_set(&self) -> bool {
        self.get_first_set() < self.get_size()
    }

    /// Returns true if at least one bit is unset.
    #[inline]
    pub fn is_any_unset(&self) -> bool {
        !self.are_all_set()
    }

    /// Returns true if the set bits form a contiguous range.
    ///
    /// Returns false if no bits are set.
    pub fn are_contiguously_set(&self) -> bool {
        let num_set = self.get_num_set();
        if num_set == 0 {
            return false;
        }
        num_set == self.get_last_set() - self.get_first_set() + 1
    }

    /// Returns the amount of memory this object holds on to.
    pub fn get_allocated_size(&self) -> usize {
        let mut size = std::mem::size_of::<Self>();
        if self.num_words > 1 {
            size += self.num_words * 8;
        }
        size
    }

    /// Computes a hash of the bit array.
    pub fn get_hash(&self) -> u64 {
        let first = self.get_first_set();
        if first == self.num {
            return first as u64;
        }

        let last = self.get_last_set();
        let offset = first >> 6;
        let end = (last >> 6) + 1;

        let mut hasher = TfHasher::new();
        for w in offset..end {
            hasher.write_u64(self.get_word(w));
        }
        hasher.finish()
    }

    /// Returns a fast hash based on size, count, first and last.
    pub fn fast_hash(&self) -> u64 {
        let mut h = TfHasher::new();
        h.write_usize(self.get_size());
        h.write_usize(self.get_first_set());
        h.write_usize(self.get_last_set());
        h.write_usize(self.get_num_set());
        h.finish()
    }

    /// Returns true if the intersection with `rhs` would be non-empty.
    ///
    /// This is more efficient than computing the full AND operation.
    pub fn has_non_empty_intersection(&self, rhs: &Self) -> bool {
        assert_eq!(self.num, rhs.num, "bit arrays must have same size");

        let first = self.get_first_set();
        let rhs_first = rhs.get_first_set();

        // Empty check
        if first >= self.num || rhs_first >= self.num {
            return false;
        }

        let start = first.max(rhs_first);
        let end = self.get_last_set().min(rhs.get_last_set());

        if start > end {
            return false;
        }

        let offset = start >> 6;
        let end_word = (end >> 6) + 1;

        for w in offset..end_word {
            let word = self.get_word(w);
            if word != 0 && (word & rhs.get_word(w)) != 0 {
                return true;
            }
        }
        false
    }

    /// Returns true if `self - rhs` would be non-empty.
    pub fn has_non_empty_difference(&self, rhs: &Self) -> bool {
        assert_eq!(self.num, rhs.num, "bit arrays must have same size");

        let first = self.get_first_set();
        if first >= self.num {
            return false;
        }

        let last = self.get_last_set();
        let rhs_first = rhs.get_first_set();
        let rhs_last = rhs.get_last_set();

        // Quick checks
        if first < rhs_first
            || last > rhs_last
            || first > rhs_last
            || last < rhs_first
            || self.get_num_set() > rhs.get_num_set()
        {
            return true;
        }

        let offset = first >> 6;
        let end_word = (last >> 6) + 1;

        for w in offset..end_word {
            let word = self.get_word(w);
            if word != 0 && (word & !rhs.get_word(w)) != 0 {
                return true;
            }
        }
        false
    }

    /// Returns true if this bit array contains all bits set in `rhs`.
    pub fn contains(&self, rhs: &Self) -> bool {
        !rhs.has_non_empty_difference(self)
    }

    /// Flips all bits (complement).
    pub fn complement(&mut self) {
        for w in 0..self.num_words {
            self.set_word(w, !self.get_word(w));
        }
        self.clear_trailing_bits();

        let num_set = self.num_set.load(Ordering::Relaxed);
        if num_set != INVALID {
            self.num_set.store(self.num - num_set, Ordering::Relaxed);
        }
        self.first_set.store(INVALID, Ordering::Relaxed);
        self.last_set.store(INVALID, Ordering::Relaxed);
    }

    /// ORs a subset into this bit array. `rhs.get_size() <= self.get_size()`.
    pub fn or_subset(&mut self, rhs: &Self) {
        assert!(self.num >= rhs.num, "rhs must be smaller or equal in size");
        self.or_impl(rhs);
    }

    /// Returns a string representation with bits left-to-right.
    pub fn as_string_left_to_right(&self) -> String {
        (0..self.num)
            .map(|i| if self.is_set(i) { '1' } else { '0' })
            .collect()
    }

    /// Returns a string representation with bits right-to-left.
    pub fn as_string_right_to_left(&self) -> String {
        (0..self.num)
            .rev()
            .map(|i| if self.is_set(i) { '1' } else { '0' })
            .collect()
    }

    /// Returns an iterator over all bit indices.
    pub fn iter(&self) -> impl Iterator<Item = usize> {
        0..self.num
    }

    /// Returns an iterator over set bit indices.
    pub fn iter_set(&self) -> SetBitsIter<'_> {
        SetBitsIter {
            bits: self,
            index: self.get_first_set(),
        }
    }

    /// Returns an iterator over unset bit indices.
    pub fn iter_unset(&self) -> UnsetBitsIter<'_> {
        UnsetBitsIter {
            bits: self,
            index: self.find_next_unset(0),
        }
    }

    // --- Internal helpers ---

    /// Gets a word from storage.
    #[inline]
    fn get_word(&self, index: usize) -> u64 {
        match &self.storage {
            Storage::Inline(data) => {
                if index == 0 {
                    *data
                } else {
                    0
                }
            }
            Storage::Heap(data) => data.get(index).copied().unwrap_or(0),
        }
    }

    /// Sets a word in storage.
    #[inline]
    fn set_word(&mut self, index: usize, value: u64) {
        match &mut self.storage {
            Storage::Inline(data) => {
                if index == 0 {
                    *data = value;
                }
            }
            Storage::Heap(data) => {
                if let Some(word) = data.get_mut(index) {
                    *word = value;
                }
            }
        }
    }

    /// Clears unused trailing bits in the last word.
    fn clear_trailing_bits(&mut self) {
        if self.num_words > 0 && (self.num & 63) != 0 {
            let used_bits = 64 - ((self.num_words << 6) - self.num);
            debug_assert!(used_bits > 0 && used_bits <= 63);
            let mask = (1u64 << used_bits) - 1;
            let word = self.get_word(self.num_words - 1);
            self.set_word(self.num_words - 1, word & mask);
        }
    }

    /// Internal OR implementation.
    fn or_impl(&mut self, rhs: &Self) {
        let rhs_first = rhs.get_first_set();
        if rhs_first >= rhs.num {
            return; // Nothing to OR
        }

        let rhs_last = rhs.get_last_set();
        let lhs_first = self.get_first_set();
        let lhs_last = self.get_last_set();

        let first = lhs_first.min(rhs_first);
        let last = if lhs_last < self.num {
            lhs_last.max(rhs_last)
        } else {
            rhs_last
        };

        // Early out if RHS is contained in LHS
        let num_set = self.num_set.load(Ordering::Relaxed);
        if num_set != INVALID
            && num_set == last - first + 1
            && first == lhs_first
            && last == lhs_last
        {
            return;
        }

        let offset = rhs_first >> 6;
        let end_word = (rhs_last >> 6) + 1;

        for w in offset..end_word {
            let combined = self.get_word(w) | rhs.get_word(w);
            self.set_word(w, combined);
        }

        self.num_set.store(INVALID, Ordering::Relaxed);
        self.first_set.store(first, Ordering::Relaxed);
        self.last_set.store(last, Ordering::Relaxed);
    }
}

impl Clone for Bits {
    fn clone(&self) -> Self {
        Self {
            num: self.num,
            num_set: AtomicUsize::new(self.num_set.load(Ordering::Relaxed)),
            first_set: AtomicUsize::new(self.first_set.load(Ordering::Relaxed)),
            last_set: AtomicUsize::new(self.last_set.load(Ordering::Relaxed)),
            num_words: self.num_words,
            storage: self.storage.clone(),
        }
    }
}

impl Clone for Storage {
    fn clone(&self) -> Self {
        match self {
            Storage::Inline(data) => Storage::Inline(*data),
            Storage::Heap(data) => Storage::Heap(data.clone()),
        }
    }
}

impl Default for Bits {
    fn default() -> Self {
        Self::new(0)
    }
}

impl PartialEq for Bits {
    fn eq(&self, other: &Self) -> bool {
        if self.num != other.num {
            return false;
        }

        // Quick checks using cached data
        let ns = self.num_set.load(Ordering::Relaxed);
        let ons = other.num_set.load(Ordering::Relaxed);
        if ns != INVALID && ons != INVALID {
            if ns != ons {
                return false;
            }
            if ns == 0 || ns == self.num {
                return true;
            }
        }

        let first = self.get_first_set();
        let last = self.get_last_set();
        let other_first = other.get_first_set();
        let other_last = other.get_last_set();

        if first != other_first || last != other_last {
            return false;
        }

        if first >= self.num {
            return true;
        }

        let offset = first >> 6;
        let end_word = (last >> 6) + 1;

        for w in offset..end_word {
            if self.get_word(w) != other.get_word(w) {
                return false;
            }
        }
        true
    }
}

impl Eq for Bits {}

impl Hash for Bits {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.get_hash());
    }
}

impl Index<usize> for Bits {
    type Output = bool;

    fn index(&self, index: usize) -> &Self::Output {
        static TRUE: bool = true;
        static FALSE: bool = false;
        if self.is_set(index) { &TRUE } else { &FALSE }
    }
}

impl BitAndAssign<&Bits> for Bits {
    fn bitand_assign(&mut self, rhs: &Bits) {
        assert_eq!(self.num, rhs.num, "bit arrays must have same size");

        let first = self.get_first_set();
        let last = self.get_last_set();

        if first >= self.num {
            return; // Already all zeros
        }

        let offset = first >> 6;
        let end_word = (last >> 6) + 1;

        for w in offset..end_word {
            let combined = self.get_word(w) & rhs.get_word(w);
            self.set_word(w, combined);
        }

        self.num_set.store(INVALID, Ordering::Relaxed);
        self.first_set
            .store(self.find_next_set(first), Ordering::Relaxed);
        self.last_set
            .store(self.find_prev_set(last), Ordering::Relaxed);
    }
}

impl BitAnd<&Bits> for &Bits {
    type Output = Bits;

    fn bitand(self, rhs: &Bits) -> Self::Output {
        let mut result = self.clone();
        result &= rhs;
        result
    }
}

impl BitOrAssign<&Bits> for Bits {
    fn bitor_assign(&mut self, rhs: &Bits) {
        assert_eq!(self.num, rhs.num, "bit arrays must have same size");
        self.or_impl(rhs);
    }
}

impl BitOr<&Bits> for &Bits {
    type Output = Bits;

    fn bitor(self, rhs: &Bits) -> Self::Output {
        let mut result = self.clone();
        result |= rhs;
        result
    }
}

impl BitXorAssign<&Bits> for Bits {
    fn bitxor_assign(&mut self, rhs: &Bits) {
        assert_eq!(self.num, rhs.num, "bit arrays must have same size");

        let i0 = self.get_first_set();
        let i1 = rhs.get_first_set();

        if i1 >= self.num {
            return; // Nothing to XOR
        }

        let first = i0.min(i1);
        let last = if i0 < self.num {
            self.get_last_set().max(rhs.get_last_set())
        } else {
            rhs.get_last_set()
        };

        let offset = first >> 6;
        let end_word = (last >> 6) + 1;

        for w in offset..end_word {
            let combined = self.get_word(w) ^ rhs.get_word(w);
            self.set_word(w, combined);
        }

        self.num_set.store(INVALID, Ordering::Relaxed);
        self.first_set
            .store(self.find_next_set(first), Ordering::Relaxed);
        self.last_set
            .store(self.find_prev_set(last), Ordering::Relaxed);
    }
}

impl BitXor<&Bits> for &Bits {
    type Output = Bits;

    fn bitxor(self, rhs: &Bits) -> Self::Output {
        let mut result = self.clone();
        result ^= rhs;
        result
    }
}

impl SubAssign<&Bits> for Bits {
    fn sub_assign(&mut self, rhs: &Bits) {
        assert_eq!(self.num, rhs.num, "bit arrays must have same size");

        let lhs_first = self.get_first_set();
        let lhs_last = self.get_last_set();

        if lhs_first >= self.num {
            return; // Nothing to subtract from
        }

        let rhs_first = rhs.get_first_set();
        if rhs_first >= self.num {
            return; // Nothing to subtract
        }

        let first = lhs_first.max(rhs_first);
        let last = lhs_last.min(rhs.get_last_set());

        if first > last {
            return;
        }

        let offset = first >> 6;
        let end_word = (last >> 6) + 1;

        for w in offset..end_word {
            let combined = self.get_word(w) & !rhs.get_word(w);
            self.set_word(w, combined);
        }

        self.num_set.store(INVALID, Ordering::Relaxed);
        self.first_set
            .store(self.find_next_set(lhs_first), Ordering::Relaxed);
        self.last_set
            .store(self.find_prev_set(lhs_last), Ordering::Relaxed);
    }
}

impl Sub<&Bits> for &Bits {
    type Output = Bits;

    fn sub(self, rhs: &Bits) -> Self::Output {
        let mut result = self.clone();
        result -= rhs;
        result
    }
}

impl fmt::Debug for Bits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bits")
            .field("size", &self.num)
            .field("num_set", &self.get_num_set())
            .field("first_set", &self.get_first_set())
            .field("last_set", &self.get_last_set())
            .finish()
    }
}

impl fmt::Display for Bits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_string_left_to_right())
    }
}

/// Iterator over set bits.
pub struct SetBitsIter<'a> {
    bits: &'a Bits,
    index: usize,
}

impl Iterator for SetBitsIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.bits.num {
            return None;
        }
        let current = self.index;
        self.index = self.bits.find_next_set(self.index + 1);
        Some(current)
    }
}

/// Iterator over unset bits.
pub struct UnsetBitsIter<'a> {
    bits: &'a Bits,
    index: usize,
}

impl Iterator for UnsetBitsIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.bits.num {
            return None;
        }
        let current = self.index;
        self.index = self.bits.find_next_unset(self.index + 1);
        Some(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_cleared() {
        let bits = Bits::new(100);
        assert_eq!(bits.get_size(), 100);
        assert!(bits.are_all_unset());
        assert_eq!(bits.get_num_set(), 0);
        assert_eq!(bits.get_first_set(), 100);
        assert_eq!(bits.get_last_set(), 100);
    }

    #[test]
    fn test_set_clear() {
        let mut bits = Bits::new(100);
        bits.set(10);
        assert!(bits.is_set(10));
        assert_eq!(bits.get_num_set(), 1);
        assert_eq!(bits.get_first_set(), 10);
        assert_eq!(bits.get_last_set(), 10);

        bits.set(50);
        bits.set(90);
        assert_eq!(bits.get_num_set(), 3);
        assert_eq!(bits.get_first_set(), 10);
        assert_eq!(bits.get_last_set(), 90);

        bits.clear(10);
        assert!(!bits.is_set(10));
        assert_eq!(bits.get_num_set(), 2);
        assert_eq!(bits.get_first_set(), 50);
    }

    #[test]
    fn test_set_all_clear_all() {
        let mut bits = Bits::new(100);
        bits.set_all();
        assert!(bits.are_all_set());
        assert_eq!(bits.get_num_set(), 100);

        bits.clear_all();
        assert!(bits.are_all_unset());
        assert_eq!(bits.get_num_set(), 0);
    }

    #[test]
    fn test_with_range() {
        let bits = Bits::with_range(100, 20, 30);
        assert_eq!(bits.get_num_set(), 11);
        assert_eq!(bits.get_first_set(), 20);
        assert_eq!(bits.get_last_set(), 30);

        for i in 0..100 {
            assert_eq!(bits.is_set(i), (20..=30).contains(&i));
        }
    }

    #[test]
    fn test_find_next_set() {
        let mut bits = Bits::new(100);
        bits.set(10);
        bits.set(50);
        bits.set(90);

        assert_eq!(bits.find_next_set(0), 10);
        assert_eq!(bits.find_next_set(10), 10);
        assert_eq!(bits.find_next_set(11), 50);
        assert_eq!(bits.find_next_set(51), 90);
        assert_eq!(bits.find_next_set(91), 100);
    }

    #[test]
    fn test_find_prev_set() {
        let mut bits = Bits::new(100);
        bits.set(10);
        bits.set(50);
        bits.set(90);

        assert_eq!(bits.find_prev_set(99), 90);
        assert_eq!(bits.find_prev_set(90), 90);
        assert_eq!(bits.find_prev_set(89), 50);
        assert_eq!(bits.find_prev_set(49), 10);
        assert_eq!(bits.find_prev_set(9), 100);
    }

    #[test]
    fn test_find_next_unset() {
        let mut bits = Bits::new(100);
        bits.set_all();
        bits.clear(10);
        bits.clear(50);

        assert_eq!(bits.find_next_unset(0), 10);
        assert_eq!(bits.find_next_unset(10), 10);
        assert_eq!(bits.find_next_unset(11), 50);
        assert_eq!(bits.find_next_unset(51), 100);
    }

    #[test]
    fn test_bitwise_and() {
        let mut a = Bits::new(64);
        let mut b = Bits::new(64);

        a.set(10);
        a.set(20);
        a.set(30);

        b.set(20);
        b.set(30);
        b.set(40);

        let result = &a & &b;
        assert_eq!(result.get_num_set(), 2);
        assert!(result.is_set(20));
        assert!(result.is_set(30));
        assert!(!result.is_set(10));
        assert!(!result.is_set(40));
    }

    #[test]
    fn test_bitwise_or() {
        let mut a = Bits::new(64);
        let mut b = Bits::new(64);

        a.set(10);
        a.set(20);

        b.set(30);
        b.set(40);

        let result = &a | &b;
        assert_eq!(result.get_num_set(), 4);
        assert!(result.is_set(10));
        assert!(result.is_set(20));
        assert!(result.is_set(30));
        assert!(result.is_set(40));
    }

    #[test]
    fn test_bitwise_xor() {
        let mut a = Bits::new(64);
        let mut b = Bits::new(64);

        a.set(10);
        a.set(20);
        a.set(30);

        b.set(20);
        b.set(30);
        b.set(40);

        let result = &a ^ &b;
        assert_eq!(result.get_num_set(), 2);
        assert!(result.is_set(10));
        assert!(result.is_set(40));
        assert!(!result.is_set(20));
        assert!(!result.is_set(30));
    }

    #[test]
    fn test_subtract() {
        let mut a = Bits::new(64);
        let mut b = Bits::new(64);

        a.set(10);
        a.set(20);
        a.set(30);

        b.set(20);
        b.set(30);

        let result = &a - &b;
        assert_eq!(result.get_num_set(), 1);
        assert!(result.is_set(10));
        assert!(!result.is_set(20));
        assert!(!result.is_set(30));
    }

    #[test]
    fn test_complement() {
        let mut bits = Bits::new(64);
        bits.set(10);
        bits.set(20);
        bits.complement();

        assert_eq!(bits.get_num_set(), 62);
        assert!(!bits.is_set(10));
        assert!(!bits.is_set(20));
        assert!(bits.is_set(0));
        assert!(bits.is_set(30));
    }

    #[test]
    fn test_equality() {
        let mut a = Bits::new(64);
        let mut b = Bits::new(64);

        a.set(10);
        a.set(20);

        b.set(10);
        b.set(20);

        assert_eq!(a, b);

        b.set(30);
        assert_ne!(a, b);
    }

    #[test]
    fn test_iter_set() {
        let mut bits = Bits::new(100);
        bits.set(10);
        bits.set(50);
        bits.set(90);

        let set_bits: Vec<_> = bits.iter_set().collect();
        assert_eq!(set_bits, vec![10, 50, 90]);
    }

    #[test]
    fn test_iter_unset() {
        let mut bits = Bits::new(10);
        bits.set_all();
        bits.clear(3);
        bits.clear(7);

        let unset_bits: Vec<_> = bits.iter_unset().collect();
        assert_eq!(unset_bits, vec![3, 7]);
    }

    #[test]
    fn test_has_non_empty_intersection() {
        let mut a = Bits::new(64);
        let mut b = Bits::new(64);

        a.set(10);
        a.set(20);

        b.set(30);
        b.set(40);

        assert!(!a.has_non_empty_intersection(&b));

        b.set(20);
        assert!(a.has_non_empty_intersection(&b));
    }

    #[test]
    fn test_has_non_empty_difference() {
        let mut a = Bits::new(64);
        let mut b = Bits::new(64);

        a.set(10);
        a.set(20);

        b.set(10);
        b.set(20);
        b.set(30);

        assert!(!a.has_non_empty_difference(&b)); // a is subset of b

        a.set(40);
        assert!(a.has_non_empty_difference(&b)); // a has 40 which b doesn't
    }

    #[test]
    fn test_contains() {
        let mut a = Bits::new(64);
        let mut b = Bits::new(64);

        a.set(10);
        a.set(20);
        a.set(30);

        b.set(10);
        b.set(20);

        assert!(a.contains(&b));
        assert!(!b.contains(&a));
    }

    #[test]
    fn test_are_contiguously_set() {
        let mut bits = Bits::new(100);
        bits.set(10);
        bits.set(11);
        bits.set(12);

        assert!(bits.are_contiguously_set());

        bits.set(20);
        assert!(!bits.are_contiguously_set());
    }

    #[test]
    fn test_resize_keep_content() {
        let mut bits = Bits::new(64);
        bits.set(10);
        bits.set(20);

        bits.resize_keep_content(128);
        assert!(bits.is_set(10));
        assert!(bits.is_set(20));
        assert_eq!(bits.get_size(), 128);

        bits.resize_keep_content(32);
        assert!(bits.is_set(10));
        assert!(bits.is_set(20));
        assert_eq!(bits.get_size(), 32);
    }

    #[test]
    fn test_inline_storage() {
        // Up to 64 bits should use inline storage
        let mut bits = Bits::new(64);
        bits.set(0);
        bits.set(63);
        assert!(bits.is_set(0));
        assert!(bits.is_set(63));
    }

    #[test]
    fn test_heap_storage() {
        // More than 64 bits should use heap storage
        let mut bits = Bits::new(128);
        bits.set(0);
        bits.set(127);
        assert!(bits.is_set(0));
        assert!(bits.is_set(127));
    }

    #[test]
    fn test_as_string() {
        let mut bits = Bits::new(8);
        bits.set(0);
        bits.set(2);
        bits.set(7);

        assert_eq!(bits.as_string_left_to_right(), "10100001");
        assert_eq!(bits.as_string_right_to_left(), "10000101");
    }

    #[test]
    fn test_swap() {
        let mut a = Bits::new(64);
        let mut b = Bits::new(64);

        a.set(10);
        b.set(20);

        a.swap(&mut b);

        assert!(a.is_set(20));
        assert!(!a.is_set(10));
        assert!(b.is_set(10));
        assert!(!b.is_set(20));
    }

    #[test]
    fn test_empty_bits() {
        let bits = Bits::new(0);
        assert!(bits.is_empty());
        assert_eq!(bits.get_num_set(), 0);
        assert!(bits.are_all_unset());
    }

    #[test]
    fn test_display() {
        let mut bits = Bits::new(4);
        bits.set(0);
        bits.set(3);
        assert_eq!(format!("{}", bits), "1001");
    }

    #[test]
    fn test_index() {
        let mut bits = Bits::new(10);
        bits.set(5);
        assert!(bits[5]);
        assert!(!bits[4]);
    }

    #[test]
    fn test_hash() {
        let mut a = Bits::new(64);
        let mut b = Bits::new(64);

        a.set(10);
        b.set(10);

        assert_eq!(a.get_hash(), b.get_hash());

        b.set(20);
        assert_ne!(a.get_hash(), b.get_hash());
    }

    // --- Bit utilities tests ---

    #[test]
    fn test_bits_for_values() {
        assert_eq!(bits_for_values(1), 0); // 0 bits for 1 value
        assert_eq!(bits_for_values(2), 1); // 1 bit for 2 values
        assert_eq!(bits_for_values(3), 2); // 2 bits for 3 values
        assert_eq!(bits_for_values(4), 2); // 2 bits for 4 values
        assert_eq!(bits_for_values(5), 3); // 3 bits for 5 values
        assert_eq!(bits_for_values(8), 3); // 3 bits for 8 values
        assert_eq!(bits_for_values(9), 4); // 4 bits for 9 values
        assert_eq!(bits_for_values(16), 4); // 4 bits for 16 values
        assert_eq!(bits_for_values(100), 7); // 7 bits for 100 values
        assert_eq!(bits_for_values(256), 8); // 8 bits for 256 values
    }

    #[test]
    fn test_bits_for_enum_values() {
        assert_eq!(bits_for_enum_values(2), 2); // 1 + 1
        assert_eq!(bits_for_enum_values(4), 3); // 2 + 1
        assert_eq!(bits_for_enum_values(8), 4); // 3 + 1
        assert_eq!(bits_for_enum_values(16), 5); // 4 + 1
    }
}
