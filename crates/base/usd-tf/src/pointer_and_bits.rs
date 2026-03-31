//! Pointer with embedded bits storage.
//!
//! This module provides [`PointerAndBits`], a type that stores a pointer
//! and a small integer in the space of a single pointer by using the
//! alignment padding of the pointed-to type.
//!
//! # How It Works
//!
//! When a type has alignment > 1, its addresses are always multiples of
//! that alignment. For example, a type with alignment 8 will always have
//! addresses ending in 0b...000 (last 3 bits zero). We can use these
//! "free" bits to store additional information.
//!
//! # Examples
//!
//! ```
//! use usd_tf::pointer_and_bits::PointerAndBits;
//!
//! let value = Box::new(42u64); // u64 has alignment 8, so 3 bits available
//! let ptr = Box::into_raw(value);
//!
//! let mut pab = PointerAndBits::<u64>::new(ptr, 0b101);
//! assert_eq!(unsafe { *pab.get() }, 42);
//! assert_eq!(pab.bits(), 0b101);
//!
//! pab.set_bits(0b011);
//! assert_eq!(pab.bits(), 0b011);
//!
//! // Clean up
//! unsafe { drop(Box::from_raw(pab.get())); }
//! ```

use std::marker::PhantomData;

/// Returns true if `val` is a power of two.
#[inline]
const fn is_pow2(val: usize) -> bool {
    val != 0 && (val & (val - 1)) == 0
}

/// A pointer that stores additional bits in the alignment padding.
///
/// The number of bits available depends on the alignment of `T`:
/// - Alignment 2: 1 bit (max value 1)
/// - Alignment 4: 2 bits (max value 3)
/// - Alignment 8: 3 bits (max value 7)
/// - Alignment 16: 4 bits (max value 15)
///
/// # Safety
///
/// This type uses unsafe code to manipulate pointer bits. The pointer
/// must be properly aligned for type `T`.
///
/// # Examples
///
/// ```
/// use usd_tf::pointer_and_bits::PointerAndBits;
///
/// // With u64 (alignment 8), we get 3 bits
/// assert_eq!(PointerAndBits::<u64>::max_bits(), 7);
/// assert_eq!(PointerAndBits::<u64>::num_bits(), 3);
///
/// // With u32 (alignment 4), we get 2 bits
/// assert_eq!(PointerAndBits::<u32>::max_bits(), 3);
/// assert_eq!(PointerAndBits::<u32>::num_bits(), 2);
/// ```
#[derive(Copy, Clone)]
pub struct PointerAndBits<T> {
    ptr_and_bits: usize,
    _marker: PhantomData<*mut T>,
}

impl<T> PointerAndBits<T> {
    /// Get the alignment of T for bit calculation.
    #[inline]
    const fn alignment() -> usize {
        let align = std::mem::align_of::<T>();
        // For abstract types (which we can't detect in const), assume pointer alignment
        if align == 0 {
            std::mem::align_of::<*const ()>()
        } else {
            align
        }
    }

    /// Returns the maximum value that can be stored in the bits.
    ///
    /// This is `alignment - 1`.
    #[inline]
    pub const fn max_bits() -> usize {
        Self::alignment() - 1
    }

    /// Returns the number of bits available for storage.
    ///
    /// This is `log2(alignment)`.
    #[inline]
    pub const fn num_bits() -> u32 {
        Self::alignment().trailing_zeros()
    }

    /// Returns the bit mask for extracting bits.
    #[inline]
    const fn bit_mask() -> usize {
        Self::max_bits()
    }

    /// Create a new PointerAndBits with null pointer and zero bits.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::pointer_and_bits::PointerAndBits;
    ///
    /// let pab = PointerAndBits::<u64>::null();
    /// assert!(pab.get().is_null());
    /// assert_eq!(pab.bits(), 0);
    /// ```
    #[inline]
    pub const fn null() -> Self {
        assert!(
            Self::alignment() > 1 && is_pow2(Self::alignment()),
            "T's alignment must be > 1 and a power of 2"
        );
        Self {
            ptr_and_bits: 0,
            _marker: PhantomData,
        }
    }

    /// Create a new PointerAndBits from a pointer and bits value.
    ///
    /// # Panics
    ///
    /// Debug builds will panic if `bits > max_bits()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::pointer_and_bits::PointerAndBits;
    ///
    /// let value = Box::new(123u64);
    /// let ptr = Box::into_raw(value);
    /// let pab = PointerAndBits::new(ptr, 5);
    /// assert_eq!(unsafe { *pab.get() }, 123);
    /// assert_eq!(pab.bits(), 5);
    /// unsafe { drop(Box::from_raw(pab.get())); }
    /// ```
    #[inline]
    pub fn new(ptr: *mut T, bits: usize) -> Self {
        debug_assert!(
            Self::alignment() > 1 && is_pow2(Self::alignment()),
            "T's alignment must be > 1 and a power of 2"
        );
        debug_assert!(bits <= Self::max_bits(), "bits value exceeds max_bits()");

        Self {
            ptr_and_bits: (ptr as usize) | (bits & Self::bit_mask()),
            _marker: PhantomData,
        }
    }

    /// Create from a const pointer.
    #[inline]
    pub fn from_const(ptr: *const T, bits: usize) -> Self {
        Self::new(ptr as *mut T, bits)
    }

    /// Get the pointer value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::pointer_and_bits::PointerAndBits;
    ///
    /// let value = Box::new(42u64);
    /// let ptr = Box::into_raw(value);
    /// let pab = PointerAndBits::new(ptr, 3);
    /// assert_eq!(pab.get(), ptr);
    /// unsafe { drop(Box::from_raw(pab.get())); }
    /// ```
    #[inline]
    pub fn get(&self) -> *mut T {
        (self.ptr_and_bits & !Self::bit_mask()) as *mut T
    }

    /// Get the pointer as a const pointer.
    #[inline]
    pub fn get_const(&self) -> *const T {
        self.get() as *const T
    }

    /// Get the stored bits value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::pointer_and_bits::PointerAndBits;
    ///
    /// let pab = PointerAndBits::<u64>::new(std::ptr::null_mut(), 7);
    /// assert_eq!(pab.bits(), 7);
    /// ```
    #[inline]
    pub fn bits(&self) -> usize {
        self.ptr_and_bits & Self::bit_mask()
    }

    /// Get the bits value as a specific type.
    #[inline]
    pub fn bits_as<U: TryFrom<usize>>(&self) -> Option<U> {
        U::try_from(self.bits()).ok()
    }

    /// Set the bits value without changing the pointer.
    ///
    /// # Panics
    ///
    /// Debug builds will panic if `bits > max_bits()`.
    #[inline]
    pub fn set_bits(&mut self, bits: usize) {
        debug_assert!(bits <= Self::max_bits(), "bits value exceeds max_bits()");
        self.ptr_and_bits = (self.ptr_and_bits & !Self::bit_mask()) | (bits & Self::bit_mask());
    }

    /// Set the pointer without changing the bits.
    #[inline]
    pub fn set_ptr(&mut self, ptr: *mut T) {
        self.ptr_and_bits = (ptr as usize) | (self.ptr_and_bits & Self::bit_mask());
    }

    /// Set both pointer and bits.
    #[inline]
    pub fn set(&mut self, ptr: *mut T, bits: usize) {
        debug_assert!(bits <= Self::max_bits(), "bits value exceeds max_bits()");
        self.ptr_and_bits = (ptr as usize) | (bits & Self::bit_mask());
    }

    /// Get the raw combined value (for comparison purposes).
    #[inline]
    pub fn literal(&self) -> usize {
        self.ptr_and_bits
    }

    /// Check if the pointer is null (bits may be non-zero).
    #[inline]
    pub fn is_null(&self) -> bool {
        self.get().is_null()
    }

    /// Swap with another PointerAndBits.
    #[inline]
    pub fn swap(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.ptr_and_bits, &mut other.ptr_and_bits);
    }
}

impl<T> Default for PointerAndBits<T> {
    fn default() -> Self {
        Self::null()
    }
}

impl<T> PartialEq for PointerAndBits<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_and_bits == other.ptr_and_bits
    }
}

impl<T> Eq for PointerAndBits<T> {}

impl<T> std::fmt::Debug for PointerAndBits<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PointerAndBits")
            .field("ptr", &self.get())
            .field("bits", &self.bits())
            .finish()
    }
}

// SAFETY: PointerAndBits is Send/Sync if T is, since it just wraps a pointer
#[allow(unsafe_code)]
unsafe impl<T: Send> Send for PointerAndBits<T> {}

#[allow(unsafe_code)]
unsafe impl<T: Sync> Sync for PointerAndBits<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment_bits() {
        // u8 has alignment 1, so no bits available
        // But we require alignment > 1, so we skip u8

        // u16 has alignment 2, so 1 bit
        assert_eq!(PointerAndBits::<u16>::max_bits(), 1);
        assert_eq!(PointerAndBits::<u16>::num_bits(), 1);

        // u32 has alignment 4, so 2 bits
        assert_eq!(PointerAndBits::<u32>::max_bits(), 3);
        assert_eq!(PointerAndBits::<u32>::num_bits(), 2);

        // u64 has alignment 8, so 3 bits
        assert_eq!(PointerAndBits::<u64>::max_bits(), 7);
        assert_eq!(PointerAndBits::<u64>::num_bits(), 3);

        // u128 has alignment 16 on most platforms, so 4 bits
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(PointerAndBits::<u128>::max_bits(), 15);
            assert_eq!(PointerAndBits::<u128>::num_bits(), 4);
        }
    }

    #[test]
    fn test_null() {
        let pab = PointerAndBits::<u64>::null();
        assert!(pab.is_null());
        assert_eq!(pab.bits(), 0);
        assert_eq!(pab.literal(), 0);
    }

    #[test]
    fn test_new_and_get() {
        let value = Box::new(42u64);
        let ptr = Box::into_raw(value);

        let pab = PointerAndBits::new(ptr, 5);
        assert_eq!(pab.get(), ptr);
        assert_eq!(pab.bits(), 5);
        assert_eq!(unsafe { *pab.get() }, 42);

        unsafe {
            drop(Box::from_raw(pab.get()));
        }
    }

    #[test]
    fn test_set_bits() {
        let value = Box::new(100u64);
        let ptr = Box::into_raw(value);

        let mut pab = PointerAndBits::new(ptr, 0);
        assert_eq!(pab.bits(), 0);

        pab.set_bits(7);
        assert_eq!(pab.bits(), 7);
        assert_eq!(pab.get(), ptr); // pointer unchanged

        pab.set_bits(3);
        assert_eq!(pab.bits(), 3);

        unsafe {
            drop(Box::from_raw(pab.get()));
        }
    }

    #[test]
    fn test_set_ptr() {
        let value1 = Box::new(1u64);
        let value2 = Box::new(2u64);
        let ptr1 = Box::into_raw(value1);
        let ptr2 = Box::into_raw(value2);

        let mut pab = PointerAndBits::new(ptr1, 5);
        assert_eq!(pab.get(), ptr1);
        assert_eq!(pab.bits(), 5);

        pab.set_ptr(ptr2);
        assert_eq!(pab.get(), ptr2);
        assert_eq!(pab.bits(), 5); // bits unchanged

        unsafe {
            drop(Box::from_raw(ptr1));
            drop(Box::from_raw(ptr2));
        }
    }

    #[test]
    fn test_set_both() {
        let value1 = Box::new(1u64);
        let value2 = Box::new(2u64);
        let ptr1 = Box::into_raw(value1);
        let ptr2 = Box::into_raw(value2);

        let mut pab = PointerAndBits::new(ptr1, 1);
        pab.set(ptr2, 6);
        assert_eq!(pab.get(), ptr2);
        assert_eq!(pab.bits(), 6);

        unsafe {
            drop(Box::from_raw(ptr1));
            drop(Box::from_raw(ptr2));
        }
    }

    #[test]
    fn test_swap() {
        let value1 = Box::new(1u64);
        let value2 = Box::new(2u64);
        let ptr1 = Box::into_raw(value1);
        let ptr2 = Box::into_raw(value2);

        let mut pab1 = PointerAndBits::new(ptr1, 1);
        let mut pab2 = PointerAndBits::new(ptr2, 2);

        pab1.swap(&mut pab2);

        assert_eq!(pab1.get(), ptr2);
        assert_eq!(pab1.bits(), 2);
        assert_eq!(pab2.get(), ptr1);
        assert_eq!(pab2.bits(), 1);

        unsafe {
            drop(Box::from_raw(ptr1));
            drop(Box::from_raw(ptr2));
        }
    }

    #[test]
    fn test_equality() {
        let pab1 = PointerAndBits::<u64>::new(std::ptr::null_mut(), 3);
        let pab2 = PointerAndBits::<u64>::new(std::ptr::null_mut(), 3);
        let pab3 = PointerAndBits::<u64>::new(std::ptr::null_mut(), 4);

        assert_eq!(pab1, pab2);
        assert_ne!(pab1, pab3);
    }

    #[test]
    fn test_default() {
        let pab = PointerAndBits::<u64>::default();
        assert!(pab.is_null());
        assert_eq!(pab.bits(), 0);
    }

    #[test]
    fn test_debug() {
        let pab = PointerAndBits::<u64>::new(std::ptr::null_mut(), 5);
        let debug_str = format!("{:?}", pab);
        assert!(debug_str.contains("PointerAndBits"));
        assert!(debug_str.contains("bits: 5"));
    }

    #[test]
    fn test_bits_as() {
        let pab = PointerAndBits::<u64>::new(std::ptr::null_mut(), 5);
        let bits_u8: Option<u8> = pab.bits_as();
        let bits_u32: Option<u32> = pab.bits_as();
        assert_eq!(bits_u8, Some(5));
        assert_eq!(bits_u32, Some(5));
    }

    #[test]
    fn test_from_const() {
        let value = 42u64;
        let ptr: *const u64 = &value;
        let pab = PointerAndBits::from_const(ptr, 3);
        assert_eq!(pab.get_const(), ptr);
        assert_eq!(pab.bits(), 3);
    }

    #[test]
    fn test_is_pow2() {
        assert!(is_pow2(1));
        assert!(is_pow2(2));
        assert!(is_pow2(4));
        assert!(is_pow2(8));
        assert!(is_pow2(16));
        assert!(!is_pow2(0));
        assert!(!is_pow2(3));
        assert!(!is_pow2(5));
        assert!(!is_pow2(6));
    }
}
