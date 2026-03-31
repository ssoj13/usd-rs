//! Compressed bit array with RLE encoding.
//!
//! This container provides fast, compressed bit array operations using run-length
//! encoding (RLE). Logical operations (AND, OR, XOR) can be performed without
//! first decompressing the internal data representation.
//!
//! The internal data compression represents consecutive runs of bits (called
//! "platforms"). A `running_bit` indicates the value of the first platform,
//! and subsequent platforms alternate between 0 and 1.
//!
//! For example, the bitset `111000101000` is represented as:
//! - `running_bit = 1`
//! - `platforms = [3, 3, 1, 1, 1, 3]` (3 ones, 3 zeros, 1 one, 1 zero, 1 one, 3 zeros)
//!
//! # Examples
//!
//! ```
//! use usd_tf::compressed_bits::CompressedBits;
//!
//! let mut bits = CompressedBits::new(12);
//! bits.set_range(0, 2);  // Set bits 0, 1, 2
//! bits.set(6);           // Set bit 6
//! bits.set(8);           // Set bit 8
//!
//! assert_eq!(bits.get_num_set(), 5);
//! ```

use std::fmt;
use std::hash::{Hash, Hasher};
use usd_arch::hash::hash64_with_seed;

/// Local storage size for small platform arrays (6 u32s = 24 bytes).
const LOCAL_SIZE: usize = 6;

/// Word array with small buffer optimization.
///
/// Stores up to `LOCAL_SIZE` words inline, allocating on heap only when needed.
#[derive(Clone)]
struct WordArray {
    /// Storage: either inline or heap-allocated.
    data: WordStorage,
    /// Number of words currently stored.
    len: u32,
}

/// Storage for word array - either inline or heap.
#[derive(Clone)]
enum WordStorage {
    /// Inline storage for small arrays.
    Local([u32; LOCAL_SIZE]),
    /// Heap storage for larger arrays.
    Heap(Vec<u32>),
}

impl WordArray {
    /// Creates a new empty word array.
    #[inline]
    fn new() -> Self {
        Self {
            data: WordStorage::Local([0; LOCAL_SIZE]),
            len: 0,
        }
    }

    /// Returns the number of words stored.
    #[inline]
    fn len(&self) -> u32 {
        self.len
    }

    /// Clears all words.
    #[inline]
    fn clear(&mut self) {
        self.len = 0;
    }

    /// Adds a word to the end (may cause reallocation).
    fn push(&mut self, value: u32) {
        let idx = self.len as usize;
        match &mut self.data {
            WordStorage::Local(arr) => {
                if idx < LOCAL_SIZE {
                    arr[idx] = value;
                } else {
                    // Need to switch to heap storage
                    let mut vec = arr.to_vec();
                    vec.push(value);
                    self.data = WordStorage::Heap(vec);
                }
            }
            WordStorage::Heap(vec) => {
                vec.push(value);
            }
        }
        self.len += 1;
    }

    /// Removes the last word.
    #[inline]
    fn pop(&mut self) {
        if self.len > 0 {
            self.len -= 1;
        }
    }

    /// Removes multiple words from the end.
    #[inline]
    fn pop_n(&mut self, n: u32) {
        self.len = self.len.saturating_sub(n);
    }

    /// Returns a reference to a word at the given index.
    #[inline]
    fn get(&self, index: usize) -> u32 {
        match &self.data {
            WordStorage::Local(arr) => arr[index],
            WordStorage::Heap(vec) => vec[index],
        }
    }

    /// Returns a mutable reference to a word at the given index.
    #[inline]
    fn get_mut(&mut self, index: usize) -> &mut u32 {
        match &mut self.data {
            WordStorage::Local(arr) => &mut arr[index],
            WordStorage::Heap(vec) => &mut vec[index],
        }
    }

    /// Returns the first word.
    #[inline]
    fn front(&self) -> u32 {
        self.get(0)
    }

    /// Returns a mutable reference to the first word.
    #[inline]
    fn front_mut(&mut self) -> &mut u32 {
        self.get_mut(0)
    }

    /// Returns the last word.
    #[inline]
    fn back(&self) -> u32 {
        self.get((self.len - 1) as usize)
    }

    /// Returns a mutable reference to the last word.
    #[inline]
    fn back_mut(&mut self) -> &mut u32 {
        let idx = (self.len - 1) as usize;
        self.get_mut(idx)
    }

    /// Returns an iterator over the words.
    fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.len as usize).map(|i| self.get(i))
    }

    /// Swaps contents with another WordArray.
    fn swap(&mut self, other: &mut WordArray) {
        std::mem::swap(&mut self.data, &mut other.data);
        std::mem::swap(&mut self.len, &mut other.len);
    }
}

impl std::ops::Index<usize> for WordArray {
    type Output = u32;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        match &self.data {
            WordStorage::Local(arr) => &arr[index],
            WordStorage::Heap(vec) => &vec[index],
        }
    }
}

impl std::ops::IndexMut<usize> for WordArray {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match &mut self.data {
            WordStorage::Local(arr) => &mut arr[index],
            WordStorage::Heap(vec) => &mut vec[index],
        }
    }
}

/// Mode for iterating over compressed bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Iterate over all bits.
    All,
    /// Iterate over only set bits.
    AllSet,
    /// Iterate over only unset bits.
    AllUnset,
    /// Iterate over platforms (returns platform start index and size).
    Platforms,
}

/// Fast, compressed bit array using run-length encoding.
///
/// Logical operations can be performed without decompression, making this
/// efficient for set operations on large, sparse bit arrays.
#[derive(Clone)]
pub struct CompressedBits {
    /// Platform sizes (RLE-encoded runs of bits).
    platforms: WordArray,
    /// Total number of bits.
    num: u32,
    /// Value of the first platform (0 or 1).
    running_bit: u8,
}

impl Default for CompressedBits {
    fn default() -> Self {
        Self::new(0)
    }
}

impl CompressedBits {
    /// Creates a new compressed bit array of the given size with all bits clear.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::compressed_bits::CompressedBits;
    ///
    /// let bits = CompressedBits::new(100);
    /// assert_eq!(bits.size(), 100);
    /// assert!(!bits.is_any_set());
    /// ```
    pub fn new(num: usize) -> Self {
        let mut platforms = WordArray::new();
        platforms.push(num as u32);
        Self {
            platforms,
            num: num as u32,
            running_bit: 0,
        }
    }

    /// Creates a compressed bit array with a range of bits set.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::compressed_bits::CompressedBits;
    ///
    /// let bits = CompressedBits::with_range(10, 2, 5);
    /// assert!(bits.is_set(2));
    /// assert!(bits.is_set(5));
    /// assert!(!bits.is_set(1));
    /// assert!(!bits.is_set(6));
    /// ```
    pub fn with_range(num: usize, first: usize, last: usize) -> Self {
        let num = num as u32;
        let first = first as u32;
        let last = last as u32;

        // Empty bitset
        if num == 0 {
            let mut platforms = WordArray::new();
            platforms.push(0);
            return Self {
                platforms,
                num: 0,
                running_bit: 0,
            };
        }

        // Range error: clear the whole bitset
        if first >= num || last >= num || first > last {
            let mut platforms = WordArray::new();
            platforms.push(num);
            return Self {
                platforms,
                num,
                running_bit: 0,
            };
        }

        let range = last - first + 1;
        let mut platforms = WordArray::new();
        let running_bit;
        let trailing_zeros;

        if first == 0 {
            running_bit = 1;
            platforms.push(range);
            trailing_zeros = num - range;
        } else {
            running_bit = 0;
            platforms.push(first);
            platforms.push(range);
            trailing_zeros = num - last - 1;
        }

        // Only push trailing zeros if there are any
        if trailing_zeros != 0 {
            platforms.push(trailing_zeros);
        }

        Self {
            platforms,
            num,
            running_bit,
        }
    }

    /// Creates a compressed bit array from the complement of another.
    pub fn complement_of(other: &CompressedBits) -> Self {
        let mut result = other.clone();
        if result.num != 0 {
            result.running_bit = 1 - result.running_bit;
        }
        result
    }

    /// Returns the size of the bit array.
    #[inline]
    pub fn size(&self) -> usize {
        self.num as usize
    }

    /// Returns true if the bit array is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.num == 0
    }

    /// Returns the number of bits currently set.
    pub fn get_num_set(&self) -> usize {
        let mut num_set = 0usize;
        let start = (1 - self.running_bit) as usize;
        let mut i = start;
        while i < self.platforms.len() as usize {
            num_set += self.platforms[i] as usize;
            i += 2;
        }
        num_set
    }

    /// Returns the number of platforms (runs) in this bitset.
    #[inline]
    pub fn get_num_platforms(&self) -> usize {
        if self.num == 0 {
            0
        } else {
            self.platforms.len() as usize
        }
    }

    /// Clears all bits to zero.
    pub fn clear_all(&mut self) {
        if self.num == 0 || (self.running_bit == 0 && self.platforms.len() == 1) {
            return;
        }
        self.running_bit = 0;
        self.platforms.clear();
        self.platforms.push(self.num);
    }

    /// Sets all bits to one.
    pub fn set_all(&mut self) {
        if self.num == 0 || (self.running_bit == 1 && self.platforms.len() == 1) {
            return;
        }
        self.running_bit = 1;
        self.platforms.clear();
        self.platforms.push(self.num);
    }

    /// Returns true if bit at index is set.
    ///
    /// Note: This is O(n) in the number of platforms. Use iterators for bulk access.
    pub fn is_set(&self, index: usize) -> bool {
        if index >= self.num as usize {
            return false;
        }
        let (_, _, bit) = self.linear_search(index);
        bit == 1
    }

    /// Sets bit at index.
    ///
    /// Note: This is a slow operation. Consider using set_range for bulk operations.
    pub fn set(&mut self, index: usize) {
        if index >= self.num as usize {
            return;
        }
        let tmp = CompressedBits::with_range(self.num as usize, index, index);
        *self |= &tmp;
    }

    /// Clears bit at index.
    ///
    /// Note: This is a slow operation. Consider using clear_all for bulk operations.
    pub fn clear(&mut self, index: usize) {
        if index >= self.num as usize {
            return;
        }
        let mut tmp = CompressedBits::with_range(self.num as usize, index, index);
        tmp.complement();
        *self &= &tmp;
    }

    /// Sets bits in the range [first, last] inclusive.
    pub fn set_range(&mut self, first: usize, last: usize) {
        let tmp = CompressedBits::with_range(self.num as usize, first, last);
        *self |= &tmp;
    }

    /// Assigns a value to bit at index.
    #[inline]
    pub fn assign(&mut self, index: usize, value: bool) {
        if value {
            self.set(index);
        } else {
            self.clear(index);
        }
    }

    /// Appends bits with the given value.
    pub fn append(&mut self, num: usize, value: bool) {
        if num == 0 {
            return;
        }

        let num = num as u32;

        if self.num == 0 {
            self.platforms[0] = num;
            self.running_bit = value as u8;
            self.num = num;
            return;
        }

        let last_value = self.running_bit == (self.platforms.len() & 1) as u8;
        if value == last_value {
            *self.platforms.back_mut() += num;
        } else {
            self.platforms.push(num);
        }

        self.num += num;
    }

    /// Flips all bits (complement).
    pub fn complement(&mut self) -> &mut Self {
        if self.num != 0 {
            self.running_bit = 1 - self.running_bit;
        }
        self
    }

    /// Returns true if all bits are set.
    #[inline]
    pub fn are_all_set(&self) -> bool {
        self.num == 0 || (self.running_bit == 1 && self.platforms.len() == 1)
    }

    /// Returns true if all bits are unset.
    #[inline]
    pub fn are_all_unset(&self) -> bool {
        !self.is_any_set()
    }

    /// Returns true if at least one bit is set.
    #[inline]
    pub fn is_any_set(&self) -> bool {
        self.num > 0 && (self.running_bit == 1 || self.platforms.len() > 1)
    }

    /// Returns true if at least one bit is unset.
    #[inline]
    pub fn is_any_unset(&self) -> bool {
        self.num > 0 && (self.running_bit == 0 || self.platforms.len() > 1)
    }

    /// Returns true if the set bits are contiguous.
    pub fn are_contiguously_set(&self) -> bool {
        let num_p = self.platforms.len();
        self.num > 0
            && num_p <= 3
            && (num_p == 2
                || (self.running_bit == 1 && num_p == 1)
                || (self.running_bit == 0 && num_p == 3))
    }

    /// Returns the index of the first set bit, or size() if none.
    pub fn get_first_set(&self) -> usize {
        if self.num == 0 || self.running_bit == 1 {
            return 0;
        }
        self.platforms.front() as usize
    }

    /// Returns the index of the last set bit, or size() if none.
    pub fn get_last_set(&self) -> usize {
        // Zero size or all zeros case
        if self.num == 0 || (self.running_bit == 0 && self.platforms.len() == 1) {
            return self.num as usize;
        }

        // If running_bit == 1 and number of words is odd or
        //    running_bit == 0 and number of words is even
        if self.running_bit == (self.platforms.len() & 1) as u8 {
            return (self.num - 1) as usize;
        }

        (self.num - 1 - self.platforms.back()) as usize
    }

    /// Finds the next set bit starting from index, or returns size().
    pub fn find_next_set(&self, index: usize) -> usize {
        if index >= self.num as usize {
            return self.num as usize;
        }

        let (_, bit_count, bit) = self.linear_search(index);
        if bit == 1 {
            return index;
        }
        bit_count
    }

    /// Finds the next unset bit starting from index, or returns size().
    pub fn find_next_unset(&self, index: usize) -> usize {
        if index >= self.num as usize {
            return self.num as usize;
        }

        let (_, bit_count, bit) = self.linear_search(index);
        if bit == 0 {
            return index;
        }
        bit_count
    }

    /// Finds the last set bit with index <= `index`, or `None` if none exists.
    ///
    /// This is a slow O(n) operation. Use iterators when possible.
    pub fn find_prev_set(&self, index: usize) -> Option<usize> {
        if index >= self.num as usize {
            return None;
        }

        let (platform_index, bit_count, bit) = self.linear_search(index);

        if bit == 1 {
            // index itself is set
            return Some(index);
        }

        // Start of the current (unset) platform
        let first = bit_count - self.platforms[platform_index] as usize;
        if first > 0 {
            // The previous platform is set and ends at first-1
            Some(first - 1)
        } else {
            None
        }
    }

    /// Decompresses this `CompressedBits` into a flat `Bits` array.
    pub fn to_bits(&self) -> crate::bits::Bits {
        let mut bits = crate::bits::Bits::new(self.num as usize);
        bits.clear_all();

        let mut bit_index = 0usize;
        let mut bit_value = self.running_bit == 1;
        for i in 0..self.platforms.len() as usize {
            let num_bits = self.platforms[i] as usize;
            for _ in 0..num_bits {
                bits.assign(bit_index, bit_value);
                bit_index += 1;
            }
            bit_value = !bit_value;
        }
        bits
    }

    /// Returns `(num_set, max_gap)` in a single pass over platforms.
    ///
    /// `max_gap` is the largest unset run that is not the leading or trailing
    /// run of zeros (matching C++ `Count()` semantics).
    pub fn count(&self) -> (usize, usize) {
        let num_platforms = self.platforms.len() as usize;
        if num_platforms == 0 {
            return (0, 0);
        }
        let last_index = num_platforms - 1;
        let mut num = 0usize;
        let mut max = 0usize;
        let mut bit = self.running_bit;
        for i in 0..num_platforms {
            if bit == 1 {
                num += self.platforms[i] as usize;
            } else if i > 0 && i < last_index {
                // Only interior unset runs count as gaps
                let gap = self.platforms[i] as usize;
                if gap > max {
                    max = gap;
                }
            }
            bit = 1 - bit;
        }
        (num, max)
    }

    /// Returns the number of set (ones) platforms.
    pub fn get_num_set_platforms(&self) -> usize {
        if self.num == 0 {
            return 0;
        }
        let num_p = self.platforms.len() as usize;
        (num_p / 2) + (num_p & self.running_bit as usize)
    }

    /// Returns the number of unset (zeros) platforms.
    pub fn get_num_unset_platforms(&self) -> usize {
        if self.num == 0 {
            return 0;
        }
        let num_p = self.platforms.len() as usize;
        (num_p / 2) + (num_p & (1 - self.running_bit as usize))
    }

    /// Returns approximate heap memory used by the platforms array, in bytes.
    pub fn get_allocated_size(&self) -> usize {
        match &self.platforms.data {
            WordStorage::Heap(vec) => vec.capacity() * std::mem::size_of::<u32>(),
            WordStorage::Local(_) => 0,
        }
    }

    /// Swaps contents with another CompressedBits.
    pub fn swap(&mut self, other: &mut Self) {
        std::mem::swap(self, other);
    }

    /// Returns the index of the n-th set bit, or size() if not enough bits.
    pub fn find_nth_set(&self, nth: usize) -> usize {
        let mut index = 0usize;
        let mut count = 0usize;
        let mut bit = self.running_bit;

        for i in 0..self.platforms.len() as usize {
            let platform = self.platforms[i] as usize;

            // Using multiplication instead of conditional to avoid branch misprediction
            if (count + platform) * (bit as usize) > nth {
                return index + (nth - count);
            }

            index += platform;
            count += platform * (bit as usize);
            bit = 1 - bit;
        }

        self.num as usize
    }

    /// Resizes the bitset while keeping contents (truncated if shrinking).
    pub fn resize_keep_contents(&mut self, num: usize) {
        let num = num as u32;
        if self.num == num {
            return;
        }

        // Reduce size to 0
        if num == 0 {
            self.platforms.clear();
            self.platforms.push(0);
            self.running_bit = 0;
            self.num = 0;
            return;
        }

        // Grow
        if self.num < num {
            if (1 - self.running_bit) == (self.platforms.len() & 1) as u8 {
                *self.platforms.back_mut() += num - self.num;
            } else {
                self.platforms.push(num - self.num);
            }
        }
        // Shrink
        else {
            let mut diff = self.num - num;
            while self.platforms.back() <= diff {
                diff -= self.platforms.back();
                self.platforms.pop();
            }
            *self.platforms.back_mut() -= diff;
        }

        self.num = num;
    }

    /// Shifts bits to the right (prepends zeros).
    pub fn shift_right(&mut self, bits: usize) {
        if self.num == 0 || bits == 0 {
            return;
        }

        let bits = bits as u32;

        if self.running_bit == 0 {
            *self.platforms.front_mut() += bits;
        } else {
            // Shift all platforms to the right and flip running bit
            self.running_bit = 0;
            self.platforms.push(0);
            for i in (1..self.platforms.len() as usize).rev() {
                self.platforms[i] = self.platforms[i - 1];
            }
            self.platforms[0] = bits;
        }

        // Trim platforms on the right
        let mut remaining = bits;
        while self.platforms.back() <= remaining {
            remaining -= self.platforms.back();
            self.platforms.pop();
        }
        *self.platforms.back_mut() -= remaining;
    }

    /// Shifts bits to the left (appends zeros).
    pub fn shift_left(&mut self, bits: usize) {
        if self.num == 0 || bits == 0 {
            return;
        }

        let bits = bits as u32;

        // How many platforms to trim on the left?
        let mut trim_bits = bits;
        let mut platform_index = 0usize;
        while platform_index < self.platforms.len() as usize
            && self.platforms[platform_index] <= trim_bits
        {
            trim_bits -= self.platforms[platform_index];
            platform_index += 1;
        }

        // Reduce the size of the first platform or clear all platforms
        if platform_index < self.platforms.len() as usize {
            self.platforms[platform_index] -= trim_bits;
        } else {
            self.platforms.clear();
            self.running_bit = 0;
            platform_index = 0;
        }

        // Shift platforms to the left
        if platform_index > 0 {
            let last = self.platforms.len() as usize - platform_index;
            for i in 0..last {
                self.platforms[i] = self.platforms[i + platform_index];
            }
            self.platforms.pop_n(platform_index as u32);

            // Flip running bit if necessary
            if platform_index & 1 != 0 {
                self.running_bit = 1 - self.running_bit;
            }
        }

        // Extend on the right with zeros
        if (1 - self.running_bit) == (self.platforms.len() & 1) as u8 {
            *self.platforms.back_mut() += bits;
        } else {
            self.platforms.push(bits.min(self.num));
        }
    }

    /// Returns true if this bitset contains all bits of rhs.
    pub fn contains(&self, rhs: &CompressedBits) -> bool {
        !rhs.has_non_empty_difference(self)
    }

    /// Returns true if the intersection with rhs is non-empty.
    pub fn has_non_empty_intersection(&self, rhs: &CompressedBits) -> bool {
        if self.num != rhs.num || self.num == 0 || rhs.num == 0 {
            return false;
        }

        let bit_a = self.running_bit;
        let bit_b = rhs.running_bit;
        if bit_a & bit_b != 0 {
            return true;
        }

        let num_a = self.platforms.len();
        let num_b = rhs.platforms.len();
        if num_a == 1 {
            return bit_a != 0 && rhs.is_any_set();
        }
        if num_b == 1 {
            return bit_b != 0 && self.is_any_set();
        }

        if self.are_bounds_disjoint(rhs) {
            return false;
        }

        self.has_logical::<And>(bit_b, &rhs.platforms)
    }

    /// Returns true if (self - rhs) is non-empty.
    pub fn has_non_empty_difference(&self, rhs: &CompressedBits) -> bool {
        if self.num != rhs.num || self.num == 0 || rhs.num == 0 {
            return false;
        }

        let bit_a = self.running_bit;
        let bit_b = rhs.running_bit;
        if bit_a != 0 && bit_b == 0 {
            return true;
        }

        let num_a = self.platforms.len();
        let num_b = rhs.platforms.len();
        if num_a == 1 {
            return bit_a != 0 && rhs.is_any_unset();
        }
        if num_b == 1 {
            if bit_b == 0 {
                return self.is_any_set();
            }
            return false;
        }

        // Check bounds
        let first_set = self.get_first_set();
        let rhs_first_set = rhs.get_first_set();
        if first_set < rhs_first_set {
            return true;
        }

        let last_set = self.get_last_set();
        let rhs_last_set = rhs.get_last_set();
        if last_set > rhs_last_set || first_set > rhs_last_set || last_set < rhs_first_set {
            return true;
        }

        self.has_logical::<And>(1 - bit_b, &rhs.platforms)
    }

    /// Computes a hash for this instance.
    pub fn get_hash(&self) -> u64 {
        if self.num == 0 {
            return 0;
        }

        // Collect platform data
        let data: Vec<u8> = self
            .platforms
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect();

        // Combine running_bit and platforms count into seed
        let seed = ((self.running_bit as u64) << 32) | (self.platforms.len() as u64);
        hash64_with_seed(&data, seed)
    }

    /// Returns a fast hash (constant time, uses limited data).
    pub fn get_fast_hash(&self) -> u64 {
        if self.num == 0 {
            return 0;
        }

        // Hash using only first cache line worth of data
        let n = (self.platforms.len() as usize).min(16);
        let data: Vec<u8> = self
            .platforms
            .iter()
            .take(n)
            .flat_map(|w| w.to_le_bytes())
            .collect();

        let seed = (self.num as u64)
            ^ ((self.running_bit as u64) << 32)
            ^ ((self.platforms.len() as u64) << 40);
        hash64_with_seed(&data, seed)
    }

    /// Returns string representation with bits left-to-right.
    pub fn as_string_left_to_right(&self) -> String {
        let mut res = String::new();
        let mut bit = self.running_bit;
        for i in 0..self.platforms.len() as usize {
            for _ in 0..self.platforms[i] {
                res.push(if bit == 1 { '1' } else { '0' });
            }
            bit = 1 - bit;
        }
        res
    }

    /// Returns string representation with bits right-to-left (reverse of left-to-right).
    pub fn as_string_right_to_left(&self) -> String {
        let s = self.as_string_left_to_right();
        s.chars().rev().collect()
    }

    /// Returns string representation in RLE format.
    pub fn as_rle_string(&self) -> String {
        if self.num == 0 {
            return String::new();
        }
        if self.num <= 4 {
            return self.as_string_left_to_right();
        }

        let mut res = String::new();
        let mut bit = self.running_bit;
        res.push_str(&format!("{}x{}", bit, self.platforms[0]));
        bit = 1 - bit;

        for i in 1..self.platforms.len() as usize {
            res.push('-');
            res.push_str(&format!("{}x{}", bit, self.platforms[i]));
            bit = 1 - bit;
        }
        res
    }

    /// Parses a compressed bits from string (RLE or binary format).
    pub fn from_string(source: &str) -> Self {
        // Try RLE format first
        if let Some(bits) = Self::from_rle_string(source) {
            return bits;
        }
        // Fall back to binary format
        Self::from_binary_string(source).unwrap_or_default()
    }

    /// Parses from RLE string format (e.g., "1x3-0x3-1x1").
    fn from_rle_string(source: &str) -> Option<Self> {
        let mut tokens = Vec::new();
        let mut current = 0u32;
        let mut expect_x = true;
        let mut has_digit = false;

        for c in source.chars() {
            if c.is_ascii_whitespace() {
                continue;
            }
            if c.is_ascii_digit() {
                current = current * 10 + (c as u32 - '0' as u32);
                has_digit = true;
            } else if c == 'x' && expect_x {
                if !has_digit {
                    return None;
                }
                tokens.push(current);
                current = 0;
                has_digit = false;
                expect_x = false;
            } else if c == '-' && !expect_x {
                if !has_digit {
                    return None;
                }
                tokens.push(current);
                current = 0;
                has_digit = false;
                expect_x = true;
            } else {
                return None;
            }
        }

        if has_digit {
            tokens.push(current);
        }

        // Must have even number of tokens (bit, length pairs)
        if tokens.is_empty() || tokens.len() & 1 != 0 {
            return None;
        }

        let mut result = CompressedBits::new(0);
        for i in (0..tokens.len()).step_by(2) {
            let bit = tokens[i];
            let length = tokens[i + 1];
            if bit > 1 || length == 0 {
                return None;
            }
            result.append(length as usize, bit == 1);
        }

        Some(result)
    }

    /// Parses from binary string format (e.g., "1110001").
    fn from_binary_string(source: &str) -> Option<Self> {
        let mut result = CompressedBits::new(0);
        for c in source.chars() {
            if c.is_ascii_whitespace() {
                continue;
            }
            match c {
                '0' => result.append(1, false),
                '1' => result.append(1, true),
                _ => return None,
            }
        }
        Some(result)
    }

    /// Returns an iterator over all set bit indices.
    pub fn iter_set(&self) -> SetBitsIter<'_> {
        SetBitsIter::new(self)
    }

    /// Returns an iterator over all unset bit indices.
    pub fn iter_unset(&self) -> UnsetBitsIter<'_> {
        UnsetBitsIter::new(self)
    }

    /// Returns an iterator over all bit indices.
    pub fn iter_all(&self) -> AllBitsIter<'_> {
        AllBitsIter::new(self)
    }

    /// Returns an iterator over platforms.
    pub fn iter_platforms(&self) -> PlatformsIter<'_> {
        PlatformsIter::new(self)
    }

    // ==================== Internal helpers ====================

    /// Linear search for bit at index.
    /// Returns (platform_index, bit_count, bit_value).
    fn linear_search(&self, index: usize) -> (usize, usize, u8) {
        let mut bit = self.running_bit;
        let mut count = 0usize;
        let mut i = 0usize;

        while i < self.platforms.len() as usize {
            count += self.platforms[i] as usize;
            if count > index {
                break;
            }
            bit = 1 - bit;
            i += 1;
        }

        (i, count, bit)
    }

    /// Returns true if bounds of this and rhs are disjoint.
    fn are_bounds_disjoint(&self, rhs: &CompressedBits) -> bool {
        self.get_last_set() < rhs.get_first_set() || self.get_first_set() > rhs.get_last_set()
    }

    /// Performs a logical operation and returns result.
    fn logical<Op: LogicalOp>(
        &mut self,
        rhs_running_bit: u8,
        rhs_platforms: &WordArray,
    ) -> &mut Self {
        let num_a = self.platforms.len();
        let num_b = rhs_platforms.len();
        let mut bit_a = self.running_bit;
        let mut bit_b = rhs_running_bit;

        let mut b = Op::apply(bit_a, bit_b);
        let mut result = WordArray::new();
        let new_running_bit = b;

        let mut index_a = 0usize;
        let mut index_b = 0usize;
        let mut platform_a = self.platforms[index_a];
        let mut platform_b = rhs_platforms[index_b];

        let mut new_total = 0u32;
        let mut new_platform = 0u32;

        loop {
            if platform_a < platform_b {
                new_total += platform_a;
                new_platform += platform_a;
                bit_a = 1 - bit_a;

                let new_bit = Op::apply(bit_a, bit_b);
                if new_bit != b {
                    result.push(new_platform);
                    new_platform = 0;
                    b = new_bit;
                }

                index_a += 1;
                platform_b -= platform_a;
                platform_a = if index_a >= num_a as usize {
                    self.num - new_total
                } else {
                    self.platforms[index_a]
                };
            } else if platform_a > platform_b {
                new_total += platform_b;
                new_platform += platform_b;
                bit_b = 1 - bit_b;

                let new_bit = Op::apply(bit_a, bit_b);
                if new_bit != b {
                    result.push(new_platform);
                    new_platform = 0;
                    b = new_bit;
                }

                index_b += 1;
                platform_a -= platform_b;
                platform_b = if index_b >= num_b as usize {
                    self.num - new_total
                } else {
                    rhs_platforms[index_b]
                };
            } else {
                new_total += platform_a;
                new_platform += platform_a;
                bit_a = 1 - bit_a;
                bit_b = 1 - bit_b;

                let new_bit = Op::apply(bit_a, bit_b);
                if new_bit != b || new_total >= self.num {
                    result.push(new_platform);
                    new_platform = 0;
                    b = new_bit;
                }

                if new_total >= self.num {
                    break;
                }

                index_a += 1;
                platform_a = if index_a >= num_a as usize {
                    self.num - new_total
                } else {
                    self.platforms[index_a]
                };

                index_b += 1;
                platform_b = if index_b >= num_b as usize {
                    self.num - new_total
                } else {
                    rhs_platforms[index_b]
                };
            }
        }

        self.platforms.swap(&mut result);
        self.running_bit = new_running_bit;
        self
    }

    /// Checks if logical operation would produce any set bits.
    fn has_logical<Op: LogicalOp>(&self, rhs_running_bit: u8, rhs_platforms: &WordArray) -> bool {
        let mut bit_a = self.running_bit;
        let mut bit_b = rhs_running_bit;
        let num_a = self.platforms.len() as usize;
        let num_b = rhs_platforms.len() as usize;

        let mut index_a = 0usize;
        let mut index_b = 0usize;
        let mut sum_platform_a = self.platforms[index_a];
        let mut sum_platform_b = rhs_platforms[index_b];

        while index_a < num_a && index_b < num_b {
            if Op::apply(bit_a, bit_b) != 0 {
                return true;
            }

            if sum_platform_a < sum_platform_b {
                bit_a = 1 - bit_a;
                index_a += 1;
                if index_a < num_a {
                    sum_platform_a += self.platforms[index_a];
                }
            } else if sum_platform_a > sum_platform_b {
                bit_b = 1 - bit_b;
                index_b += 1;
                if index_b < num_b {
                    sum_platform_b += rhs_platforms[index_b];
                }
            } else {
                bit_a = 1 - bit_a;
                bit_b = 1 - bit_b;
                index_a += 1;
                index_b += 1;

                if index_a >= num_a || index_b >= num_b {
                    return false;
                }

                sum_platform_a += self.platforms[index_a];
                sum_platform_b += rhs_platforms[index_b];
            }
        }

        false
    }
}

// ==================== Logical operation traits ====================

/// Trait for logical operations.
trait LogicalOp {
    fn apply(a: u8, b: u8) -> u8;
}

/// AND operation.
struct And;
impl LogicalOp for And {
    #[inline]
    fn apply(a: u8, b: u8) -> u8 {
        a & b
    }
}

/// OR operation.
struct Or;
impl LogicalOp for Or {
    #[inline]
    fn apply(a: u8, b: u8) -> u8 {
        a | b
    }
}

/// XOR operation.
struct Xor;
impl LogicalOp for Xor {
    #[inline]
    fn apply(a: u8, b: u8) -> u8 {
        a ^ b
    }
}

// ==================== Operator implementations ====================

impl std::ops::BitAndAssign<&CompressedBits> for CompressedBits {
    fn bitand_assign(&mut self, rhs: &CompressedBits) {
        if self.num != rhs.num || self.num == 0 || rhs.num == 0 {
            return;
        }

        let num_a = self.platforms.len();
        let num_b = rhs.platforms.len();
        let bit_a = self.running_bit;
        let bit_b = rhs.running_bit;

        // Early bailout: This is all zeros or all ones
        if num_a == 1 {
            if bit_a == 0 {
                return;
            }
            self.running_bit = bit_b;
            self.platforms = rhs.platforms.clone();
            return;
        }

        // Early bailout: Rhs is all zeros or all ones
        if num_b == 1 {
            if bit_b == 1 {
                return;
            }
            self.clear_all();
            return;
        }

        // Early bailout: No bits will overlap if sets are disjoint
        if self.are_bounds_disjoint(rhs) {
            self.clear_all();
            return;
        }

        self.logical::<And>(bit_b, &rhs.platforms);
    }
}

impl std::ops::BitAnd<&CompressedBits> for &CompressedBits {
    type Output = CompressedBits;

    fn bitand(self, rhs: &CompressedBits) -> Self::Output {
        let mut result = self.clone();
        result &= rhs;
        result
    }
}

impl std::ops::BitOrAssign<&CompressedBits> for CompressedBits {
    fn bitor_assign(&mut self, rhs: &CompressedBits) {
        if self.num != rhs.num || self.num == 0 || rhs.num == 0 {
            return;
        }

        let num_a = self.platforms.len();
        let num_b = rhs.platforms.len();
        let bit_a = self.running_bit;
        let bit_b = rhs.running_bit;

        // Early bailout: This is all zeros or all ones
        if num_a == 1 {
            if bit_a == 1 {
                return;
            }
            self.running_bit = bit_b;
            self.platforms = rhs.platforms.clone();
            return;
        }

        // Early bailout: Rhs is all zeros or all ones
        if num_b == 1 {
            if bit_b == 0 {
                return;
            }
            self.set_all();
            return;
        }

        // If this already contains all bits in rhs, skip
        if self.contains(rhs) {
            return;
        }

        self.logical::<Or>(bit_b, &rhs.platforms);
    }
}

impl std::ops::BitOr<&CompressedBits> for &CompressedBits {
    type Output = CompressedBits;

    fn bitor(self, rhs: &CompressedBits) -> Self::Output {
        let mut result = self.clone();
        result |= rhs;
        result
    }
}

impl std::ops::BitXorAssign<&CompressedBits> for CompressedBits {
    fn bitxor_assign(&mut self, rhs: &CompressedBits) {
        if self.num != rhs.num || self.num == 0 || rhs.num == 0 {
            return;
        }

        // Early bailout: This is all zeros
        if self.are_all_unset() {
            *self = rhs.clone();
            return;
        }

        // Early bailout: Rhs is all zeros
        if rhs.are_all_unset() {
            return;
        }

        self.logical::<Xor>(rhs.running_bit, &rhs.platforms);
    }
}

impl std::ops::BitXor<&CompressedBits> for &CompressedBits {
    type Output = CompressedBits;

    fn bitxor(self, rhs: &CompressedBits) -> Self::Output {
        let mut result = self.clone();
        result ^= rhs;
        result
    }
}

impl std::ops::SubAssign<&CompressedBits> for CompressedBits {
    fn sub_assign(&mut self, rhs: &CompressedBits) {
        if self.num != rhs.num || self.num == 0 || rhs.num == 0 {
            return;
        }

        let num_a = self.platforms.len();
        let num_b = rhs.platforms.len();
        let bit_a = self.running_bit;
        let bit_b = rhs.running_bit;

        // Early bailout: This is all zeros or all ones
        if num_a == 1 {
            if bit_a == 0 {
                return;
            }
            self.running_bit = 1 - bit_b;
            self.platforms = rhs.platforms.clone();
            return;
        }

        // Early bailout: Rhs is all zeros or all ones
        if num_b == 1 {
            if bit_b == 0 {
                return;
            }
            self.clear_all();
            return;
        }

        // Early bailout: No bits will be subtracted if sets are disjoint
        if self.are_bounds_disjoint(rhs) || !self.has_non_empty_intersection(rhs) {
            return;
        }

        self.logical::<And>(1 - bit_b, &rhs.platforms);
    }
}

impl std::ops::Sub<&CompressedBits> for &CompressedBits {
    type Output = CompressedBits;

    fn sub(self, rhs: &CompressedBits) -> Self::Output {
        let mut result = self.clone();
        result -= rhs;
        result
    }
}

impl std::ops::Not for &CompressedBits {
    type Output = CompressedBits;

    fn not(self) -> Self::Output {
        CompressedBits::complement_of(self)
    }
}

impl std::ops::ShrAssign<usize> for CompressedBits {
    fn shr_assign(&mut self, bits: usize) {
        self.shift_right(bits);
    }
}

impl std::ops::Shr<usize> for &CompressedBits {
    type Output = CompressedBits;

    fn shr(self, bits: usize) -> Self::Output {
        let mut result = self.clone();
        result >>= bits;
        result
    }
}

impl std::ops::ShlAssign<usize> for CompressedBits {
    fn shl_assign(&mut self, bits: usize) {
        self.shift_left(bits);
    }
}

impl std::ops::Shl<usize> for &CompressedBits {
    type Output = CompressedBits;

    fn shl(self, bits: usize) -> Self::Output {
        let mut result = self.clone();
        result <<= bits;
        result
    }
}

impl std::ops::Index<usize> for CompressedBits {
    type Output = bool;

    fn index(&self, index: usize) -> &Self::Output {
        // We can't return a reference to a computed bool, so use a static
        static TRUE: bool = true;
        static FALSE: bool = false;
        if self.is_set(index) { &TRUE } else { &FALSE }
    }
}

impl PartialEq for CompressedBits {
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) || (self.num == 0 && other.num == 0) {
            return true;
        }

        if self.num == other.num
            && self.running_bit == other.running_bit
            && self.platforms.len() == other.platforms.len()
        {
            for i in 0..self.platforms.len() as usize {
                if self.platforms[i] != other.platforms[i] {
                    return false;
                }
            }
            return true;
        }

        false
    }
}

impl Eq for CompressedBits {}

impl Hash for CompressedBits {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.get_hash());
    }
}

impl fmt::Debug for CompressedBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CompressedBits({})", self.as_rle_string())
    }
}

impl fmt::Display for CompressedBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_string_left_to_right())
    }
}

// ==================== Iterators ====================

/// Iterator over set bits.
pub struct SetBitsIter<'a> {
    bits: &'a CompressedBits,
    platform_index: usize,
    bit_index: u32,
    bit_counter: u32,
}

impl<'a> SetBitsIter<'a> {
    fn new(bits: &'a CompressedBits) -> Self {
        let bit = bits.running_bit;
        // Skip first platform if it's zeros
        if bit == 0 && bits.platforms.len() > 1 {
            Self {
                bits,
                platform_index: 1,
                bit_index: bits.platforms[0],
                bit_counter: 0,
            }
        } else {
            Self {
                bits,
                platform_index: 0,
                bit_index: 0,
                bit_counter: 0,
            }
        }
    }
}

impl<'a> Iterator for SetBitsIter<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bit_index >= self.bits.num {
            return None;
        }

        let result = self.bit_index as usize;

        // Advance
        self.bit_index += 1;
        self.bit_counter += 1;

        // Check if we need to move to next platform
        if self.bit_counter >= self.bits.platforms[self.platform_index] {
            let num_p = self.bits.platforms.len() as usize;
            if self.platform_index + 1 < num_p {
                // Skip the next platform (zeros)
                self.bit_index += self.bits.platforms[self.platform_index + 1];
                self.platform_index += 2;
            } else {
                // At end
                self.bit_index = self.bits.num;
            }
            self.bit_counter = 0;
        }

        Some(result)
    }
}

/// Iterator over unset bits.
pub struct UnsetBitsIter<'a> {
    bits: &'a CompressedBits,
    platform_index: usize,
    bit_index: u32,
    bit_counter: u32,
}

impl<'a> UnsetBitsIter<'a> {
    fn new(bits: &'a CompressedBits) -> Self {
        let bit = bits.running_bit;
        // Skip first platform if it's ones
        if bit == 1 && bits.platforms.len() > 1 {
            Self {
                bits,
                platform_index: 1,
                bit_index: bits.platforms[0],
                bit_counter: 0,
            }
        } else {
            Self {
                bits,
                platform_index: 0,
                bit_index: 0,
                bit_counter: 0,
            }
        }
    }
}

impl<'a> Iterator for UnsetBitsIter<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bit_index >= self.bits.num {
            return None;
        }

        let result = self.bit_index as usize;

        // Advance
        self.bit_index += 1;
        self.bit_counter += 1;

        // Check if we need to move to next platform
        if self.bit_counter >= self.bits.platforms[self.platform_index] {
            let num_p = self.bits.platforms.len() as usize;
            if self.platform_index + 1 < num_p {
                // Skip the next platform (ones)
                self.bit_index += self.bits.platforms[self.platform_index + 1];
                self.platform_index += 2;
            } else {
                // At end
                self.bit_index = self.bits.num;
            }
            self.bit_counter = 0;
        }

        Some(result)
    }
}

/// Iterator over all bits (returns (index, is_set) tuples).
pub struct AllBitsIter<'a> {
    bits: &'a CompressedBits,
    platform_index: usize,
    bit_index: u32,
    bit_counter: u32,
    value: u8,
}

impl<'a> AllBitsIter<'a> {
    fn new(bits: &'a CompressedBits) -> Self {
        Self {
            bits,
            platform_index: 0,
            bit_index: 0,
            bit_counter: 0,
            value: bits.running_bit,
        }
    }
}

impl<'a> Iterator for AllBitsIter<'a> {
    type Item = (usize, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.bit_index >= self.bits.num {
            return None;
        }

        let result = (self.bit_index as usize, self.value == 1);

        // Advance
        self.bit_index += 1;
        self.bit_counter += 1;

        // Check if we need to move to next platform
        if self.bit_counter >= self.bits.platforms[self.platform_index] {
            self.platform_index += 1;
            self.value = 1 - self.value;
            self.bit_counter = 0;
        }

        Some(result)
    }
}

/// Iterator over platforms (returns (start_index, size, is_set) tuples).
pub struct PlatformsIter<'a> {
    bits: &'a CompressedBits,
    platform_index: usize,
    bit_index: u32,
    value: u8,
}

impl<'a> PlatformsIter<'a> {
    fn new(bits: &'a CompressedBits) -> Self {
        Self {
            bits,
            platform_index: 0,
            bit_index: 0,
            value: bits.running_bit,
        }
    }
}

impl<'a> Iterator for PlatformsIter<'a> {
    type Item = (usize, usize, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.platform_index >= self.bits.platforms.len() as usize {
            return None;
        }

        let size = self.bits.platforms[self.platform_index] as usize;
        let result = (self.bit_index as usize, size, self.value == 1);

        self.bit_index += self.bits.platforms[self.platform_index];
        self.platform_index += 1;
        self.value = 1 - self.value;

        Some(result)
    }
}

impl From<&crate::bits::Bits> for CompressedBits {
    /// Compresses a flat `Bits` into `CompressedBits` using run-length encoding.
    fn from(bits: &crate::bits::Bits) -> Self {
        let size = bits.get_size();
        if size == 0 {
            let mut platforms = WordArray::new();
            platforms.push(0);
            return Self {
                platforms,
                num: 0,
                running_bit: 0,
            };
        }

        let set = bits.is_set(0);
        let running_bit = if set { 1u8 } else { 0u8 };
        let mut platforms = WordArray::new();

        let mut i = 0usize;
        let mut current_set = set;
        while i < size {
            // Find end of current run
            let next = if current_set {
                bits.find_next_unset(i + 1)
            } else {
                bits.find_next_set(i + 1)
            };
            platforms.push((next - i) as u32);
            current_set = !current_set;
            i = next;
        }

        Self {
            platforms,
            num: size as u32,
            running_bit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let bits = CompressedBits::new(100);
        assert_eq!(bits.size(), 100);
        assert!(!bits.is_any_set());
        assert!(bits.are_all_unset());
    }

    #[test]
    fn test_with_range() {
        let bits = CompressedBits::with_range(10, 2, 5);
        assert_eq!(bits.size(), 10);
        assert!(!bits.is_set(0));
        assert!(!bits.is_set(1));
        assert!(bits.is_set(2));
        assert!(bits.is_set(5));
        assert!(!bits.is_set(6));
        assert_eq!(bits.get_num_set(), 4);
    }

    #[test]
    fn test_set_clear() {
        let mut bits = CompressedBits::new(10);
        bits.set(3);
        bits.set(7);
        assert!(bits.is_set(3));
        assert!(bits.is_set(7));
        assert!(!bits.is_set(5));

        bits.clear(3);
        assert!(!bits.is_set(3));
    }

    #[test]
    fn test_set_all_clear_all() {
        let mut bits = CompressedBits::new(100);
        bits.set_all();
        assert!(bits.are_all_set());
        assert_eq!(bits.get_num_set(), 100);

        bits.clear_all();
        assert!(bits.are_all_unset());
        assert_eq!(bits.get_num_set(), 0);
    }

    #[test]
    fn test_complement() {
        let mut bits = CompressedBits::with_range(10, 2, 5);
        bits.complement();
        assert!(bits.is_set(0));
        assert!(bits.is_set(1));
        assert!(!bits.is_set(2));
        assert!(!bits.is_set(5));
        assert!(bits.is_set(6));
    }

    #[test]
    fn test_and() {
        let a = CompressedBits::with_range(10, 2, 7);
        let b = CompressedBits::with_range(10, 5, 9);
        let c = &a & &b;
        assert_eq!(c.get_first_set(), 5);
        assert_eq!(c.get_last_set(), 7);
        assert_eq!(c.get_num_set(), 3);
    }

    #[test]
    fn test_or() {
        let a = CompressedBits::with_range(10, 0, 2);
        let b = CompressedBits::with_range(10, 7, 9);
        let c = &a | &b;
        assert_eq!(c.get_num_set(), 6);
        assert!(c.is_set(0));
        assert!(c.is_set(2));
        assert!(!c.is_set(3));
        assert!(c.is_set(7));
        assert!(c.is_set(9));
    }

    #[test]
    fn test_xor() {
        let a = CompressedBits::with_range(10, 2, 7);
        let b = CompressedBits::with_range(10, 5, 9);
        let c = &a ^ &b;
        // XOR: bits in a or b but not both
        assert!(c.is_set(2)); // Only in a
        assert!(c.is_set(4)); // Only in a
        assert!(!c.is_set(5)); // In both
        assert!(!c.is_set(7)); // In both
        assert!(c.is_set(8)); // Only in b
    }

    #[test]
    fn test_difference() {
        let a = CompressedBits::with_range(10, 2, 7);
        let b = CompressedBits::with_range(10, 5, 9);
        let c = &a - &b;
        // Difference: bits in a but not in b
        assert!(c.is_set(2));
        assert!(c.is_set(4));
        assert!(!c.is_set(5));
        assert!(!c.is_set(7));
    }

    #[test]
    fn test_shift_right() {
        let mut bits = CompressedBits::with_range(10, 0, 2);
        bits.shift_right(3);
        assert!(!bits.is_set(0));
        assert!(!bits.is_set(2));
        assert!(bits.is_set(3));
        assert!(bits.is_set(5));
        assert!(!bits.is_set(6));
    }

    #[test]
    fn test_shift_left() {
        let mut bits = CompressedBits::with_range(10, 7, 9);
        bits.shift_left(3);
        assert!(bits.is_set(4));
        assert!(bits.is_set(6));
        assert!(!bits.is_set(7));
    }

    #[test]
    fn test_first_last_set() {
        let bits = CompressedBits::with_range(100, 25, 75);
        assert_eq!(bits.get_first_set(), 25);
        assert_eq!(bits.get_last_set(), 75);
    }

    #[test]
    fn test_find_next_set() {
        let bits = CompressedBits::with_range(20, 5, 10);
        assert_eq!(bits.find_next_set(0), 5);
        assert_eq!(bits.find_next_set(5), 5);
        assert_eq!(bits.find_next_set(7), 7);
        assert_eq!(bits.find_next_set(11), 20);
    }

    #[test]
    fn test_find_nth_set() {
        let mut bits = CompressedBits::new(20);
        bits.set(3);
        bits.set(7);
        bits.set(12);
        assert_eq!(bits.find_nth_set(0), 3);
        assert_eq!(bits.find_nth_set(1), 7);
        assert_eq!(bits.find_nth_set(2), 12);
        assert_eq!(bits.find_nth_set(3), 20);
    }

    #[test]
    fn test_append() {
        let mut bits = CompressedBits::new(0);
        bits.append(5, false);
        bits.append(3, true);
        bits.append(2, false);
        assert_eq!(bits.size(), 10);
        assert!(!bits.is_set(0));
        assert!(bits.is_set(5));
        assert!(bits.is_set(7));
        assert!(!bits.is_set(8));
    }

    #[test]
    fn test_resize() {
        let mut bits = CompressedBits::with_range(20, 5, 15);
        bits.resize_keep_contents(30);
        assert_eq!(bits.size(), 30);
        assert!(bits.is_set(10));
        assert!(!bits.is_set(20));

        bits.resize_keep_contents(10);
        assert_eq!(bits.size(), 10);
        assert!(bits.is_set(5));
        assert!(bits.is_set(9));
    }

    #[test]
    fn test_contains() {
        let a = CompressedBits::with_range(10, 2, 8);
        let b = CompressedBits::with_range(10, 4, 6);
        assert!(a.contains(&b));
        assert!(!b.contains(&a));
    }

    #[test]
    fn test_has_non_empty_intersection() {
        let a = CompressedBits::with_range(10, 0, 4);
        let b = CompressedBits::with_range(10, 3, 7);
        let c = CompressedBits::with_range(10, 6, 9);

        assert!(a.has_non_empty_intersection(&b));
        assert!(!a.has_non_empty_intersection(&c));
    }

    #[test]
    fn test_iter_set() {
        let bits = CompressedBits::with_range(10, 3, 6);
        let set: Vec<_> = bits.iter_set().collect();
        assert_eq!(set, vec![3, 4, 5, 6]);
    }

    #[test]
    fn test_iter_unset() {
        let bits = CompressedBits::with_range(10, 3, 6);
        let unset: Vec<_> = bits.iter_unset().collect();
        assert_eq!(unset, vec![0, 1, 2, 7, 8, 9]);
    }

    #[test]
    fn test_iter_platforms() {
        let bits = CompressedBits::with_range(10, 3, 6);
        let platforms: Vec<_> = bits.iter_platforms().collect();
        // (start, size, is_set)
        assert_eq!(platforms, vec![(0, 3, false), (3, 4, true), (7, 3, false)]);
    }

    #[test]
    fn test_string_conversion() {
        let bits = CompressedBits::with_range(12, 3, 5);
        let ltr = bits.as_string_left_to_right();
        assert_eq!(ltr, "000111000000");

        let rle = bits.as_rle_string();
        assert_eq!(rle, "0x3-1x3-0x6");

        let parsed = CompressedBits::from_string(&rle);
        assert_eq!(bits, parsed);
    }

    #[test]
    fn test_from_binary_string() {
        let bits = CompressedBits::from_string("00011100");
        assert_eq!(bits.size(), 8);
        assert!(!bits.is_set(0));
        assert!(bits.is_set(3));
        assert!(!bits.is_set(6));
    }

    #[test]
    fn test_equality() {
        let a = CompressedBits::with_range(10, 2, 5);
        let b = CompressedBits::with_range(10, 2, 5);
        let c = CompressedBits::with_range(10, 2, 6);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_hash() {
        let a = CompressedBits::with_range(100, 20, 50);
        let b = CompressedBits::with_range(100, 20, 50);
        assert_eq!(a.get_hash(), b.get_hash());
    }

    #[test]
    fn test_contiguously_set() {
        let a = CompressedBits::with_range(10, 3, 7);
        assert!(a.are_contiguously_set());

        let mut b = CompressedBits::new(10);
        b.set(2);
        b.set(5);
        assert!(!b.are_contiguously_set());
    }

    #[test]
    fn test_empty_bits() {
        let bits = CompressedBits::new(0);
        assert!(bits.is_empty());
        assert!(!bits.is_any_set());
        assert_eq!(bits.get_num_set(), 0);
        assert_eq!(bits.get_first_set(), 0);
        assert_eq!(bits.get_last_set(), 0);
    }

    #[test]
    fn test_find_prev_set() {
        // bits 5-10 set in a 20-bit array
        let bits = CompressedBits::with_range(20, 5, 10);

        // index in the set range returns itself
        assert_eq!(bits.find_prev_set(7), Some(7));
        assert_eq!(bits.find_prev_set(5), Some(5));
        assert_eq!(bits.find_prev_set(10), Some(10));

        // index in the leading unset range [0..4] has no preceding set bit
        assert_eq!(bits.find_prev_set(0), None);
        assert_eq!(bits.find_prev_set(3), None);

        // index in the trailing unset range [11..19] returns last set bit (10)
        assert_eq!(bits.find_prev_set(11), Some(10));
        assert_eq!(bits.find_prev_set(19), Some(10));

        // out-of-bounds returns None
        assert_eq!(bits.find_prev_set(20), None);
    }

    #[test]
    fn test_to_bits_and_from_bits() {
        // round-trip: CompressedBits -> Bits -> CompressedBits
        let original = CompressedBits::with_range(12, 3, 7);
        let flat = original.to_bits();

        assert_eq!(flat.get_size(), 12);
        for i in 0..12usize {
            assert_eq!(flat.is_set(i), original.is_set(i), "bit {i} mismatch");
        }

        let roundtrip = CompressedBits::from(&flat);
        assert_eq!(roundtrip, original);
    }

    #[test]
    fn test_from_bits_all_zeros() {
        let flat = crate::bits::Bits::new(8);
        let cb = CompressedBits::from(&flat);
        assert_eq!(cb.size(), 8);
        assert_eq!(cb.get_num_set(), 0);
    }

    #[test]
    fn test_count() {
        // bits: 000 111 00 1 000  (indices 3-5, 8)
        let mut bits = CompressedBits::new(12);
        bits.set(3);
        bits.set(4);
        bits.set(5);
        bits.set(8);

        let (num_set, max_gap) = bits.count();
        assert_eq!(num_set, 4);
        // Interior gap between platforms [3..5] and [8]: 2 zeros
        assert_eq!(max_gap, 2);

        // All zeros: no set bits, no gap
        let zeros = CompressedBits::new(10);
        assert_eq!(zeros.count(), (0, 0));

        // All set: num_set = size, max_gap = 0
        let all_set = CompressedBits::with_range(5, 0, 4);
        assert_eq!(all_set.count(), (5, 0));
    }

    #[test]
    fn test_platform_counts() {
        // platforms: [3 zeros, 4 ones, 5 zeros] -> running_bit=0, 3 platforms
        let bits = CompressedBits::with_range(12, 3, 6);
        // running_bit=0: set platforms at odd indices -> 1 set platform
        assert_eq!(bits.get_num_set_platforms(), 1);
        assert_eq!(bits.get_num_unset_platforms(), 2);

        // All ones: 1 platform, running_bit=1
        let all_ones = CompressedBits::with_range(5, 0, 4);
        assert_eq!(all_ones.get_num_set_platforms(), 1);
        assert_eq!(all_ones.get_num_unset_platforms(), 0);

        // Empty
        let empty = CompressedBits::new(0);
        assert_eq!(empty.get_num_set_platforms(), 0);
        assert_eq!(empty.get_num_unset_platforms(), 0);
    }
}
