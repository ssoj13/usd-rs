//! Small vector with local storage optimization.
//!
//! This module provides a vector type that stores a small number of elements
//! inline without heap allocation. When the number of elements exceeds the
//! local capacity, it automatically switches to heap storage.
//!
//! # Examples
//!
//! ```
//! use usd_tf::small_vector::SmallVec;
//!
//! // Create a SmallVec with capacity for 4 elements inline
//! let mut vec: SmallVec<i32, 4> = SmallVec::new();
//!
//! // These fit in local storage
//! vec.push(1);
//! vec.push(2);
//! vec.push(3);
//! vec.push(4);
//! assert!(!vec.is_heap());
//!
//! // This triggers heap allocation
//! vec.push(5);
//! assert!(vec.is_heap());
//! ```
//!
//! # Memory Layout
//!
//! The vector uses an enum to store either:
//! - Inline storage for up to N elements
//! - A pointer to heap-allocated storage
//!
//! This gives a minimum size of approximately `max(N * sizeof(T), sizeof(usize)) + 8` bytes.

use std::alloc::{self, Layout};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::iter::FusedIterator;
use std::mem::{self, MaybeUninit};
use std::ops::{Deref, DerefMut, Index, IndexMut};
use std::ptr::{self, NonNull};
use std::slice;

/// A vector with inline storage optimization.
///
/// `SmallVec<T, N>` stores up to N elements inline without heap allocation.
/// When more elements are needed, it automatically allocates on the heap.
///
/// # Type Parameters
///
/// - `T`: The element type
/// - `N`: The number of elements to store inline
///
/// # Examples
///
/// ```
/// use usd_tf::small_vector::SmallVec;
///
/// let mut vec: SmallVec<String, 2> = SmallVec::new();
/// vec.push("hello".to_string());
/// vec.push("world".to_string());
///
/// assert_eq!(vec.len(), 2);
/// assert_eq!(&vec[0], "hello");
/// ```
pub struct SmallVec<T, const N: usize> {
    /// Storage discriminated by capacity value.
    storage: Storage<T, N>,
}

/// Storage enum for inline or heap data.
enum Storage<T, const N: usize> {
    /// Inline storage with length.
    Inline {
        data: [MaybeUninit<T>; N],
        len: usize,
    },
    /// Heap storage with pointer, length, and capacity.
    Heap {
        ptr: NonNull<T>,
        len: usize,
        cap: usize,
    },
}

impl<T, const N: usize> SmallVec<T, N> {
    /// Creates a new empty SmallVec.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::small_vector::SmallVec;
    ///
    /// let vec: SmallVec<i32, 4> = SmallVec::new();
    /// assert!(vec.is_empty());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            storage: Storage::Inline {
                // SAFETY: MaybeUninit array can be safely created uninitialized
                #[allow(unsafe_code)]
                data: unsafe { MaybeUninit::uninit().assume_init() },
                len: 0,
            },
        }
    }

    /// Creates a SmallVec with the specified capacity.
    ///
    /// If `cap > N`, heap storage is allocated.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::small_vector::SmallVec;
    ///
    /// let vec: SmallVec<i32, 4> = SmallVec::with_capacity(10);
    /// assert!(vec.capacity() >= 10);
    /// ```
    pub fn with_capacity(cap: usize) -> Self {
        let mut vec = Self::new();
        vec.reserve(cap);
        vec
    }

    /// Creates a SmallVec from an iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::small_vector::SmallVec;
    ///
    /// let vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3]);
    /// assert_eq!(vec.len(), 3);
    /// ```
    pub fn from_iter_items<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (lower, _) = iter.size_hint();
        let mut vec = Self::with_capacity(lower);
        for item in iter {
            vec.push(item);
        }
        vec
    }

    /// Returns the number of elements.
    #[inline]
    pub fn len(&self) -> usize {
        match &self.storage {
            Storage::Inline { len, .. } => *len,
            Storage::Heap { len, .. } => *len,
        }
    }

    /// Returns true if the vector is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the current capacity.
    #[inline]
    pub fn capacity(&self) -> usize {
        match &self.storage {
            Storage::Inline { .. } => N,
            Storage::Heap { cap, .. } => *cap,
        }
    }

    /// Returns the inline capacity (N).
    #[inline]
    pub const fn inline_capacity() -> usize {
        N
    }

    /// Returns true if using heap storage.
    #[inline]
    pub fn is_heap(&self) -> bool {
        matches!(self.storage, Storage::Heap { .. })
    }

    /// Returns true if using inline storage.
    #[inline]
    pub fn is_inline(&self) -> bool {
        matches!(self.storage, Storage::Inline { .. })
    }

    /// Returns a pointer to the data.
    #[inline]
    fn as_ptr(&self) -> *const T {
        match &self.storage {
            Storage::Heap { ptr, .. } => ptr.as_ptr(),
            Storage::Inline { data, .. } => data.as_ptr() as *const T,
        }
    }

    /// Returns a mutable pointer to the data.
    #[inline]
    fn as_mut_ptr(&mut self) -> *mut T {
        match &mut self.storage {
            Storage::Heap { ptr, .. } => ptr.as_ptr(),
            Storage::Inline { data, .. } => data.as_mut_ptr() as *mut T,
        }
    }

    /// Returns a slice of the elements.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: as_ptr() returns valid pointer, len is accurate
        #[allow(unsafe_code)]
        unsafe {
            slice::from_raw_parts(self.as_ptr(), self.len())
        }
    }

    /// Returns a mutable slice of the elements.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: as_mut_ptr() returns valid mutable pointer, len is accurate
        #[allow(unsafe_code)]
        unsafe {
            slice::from_raw_parts_mut(self.as_mut_ptr(), self.len())
        }
    }

    /// Reserves capacity for at least `additional` more elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::small_vector::SmallVec;
    ///
    /// let mut vec: SmallVec<i32, 4> = SmallVec::new();
    /// vec.reserve(10);
    /// assert!(vec.capacity() >= 10);
    /// ```
    pub fn reserve(&mut self, additional: usize) {
        let needed = self.len().saturating_add(additional);
        if needed > self.capacity() {
            self.grow(needed);
        }
    }

    /// Grows the storage to at least `new_cap`.
    fn grow(&mut self, new_cap: usize) {
        assert!(new_cap > self.capacity());

        // Growth factor 1.5
        let growth_cap = self.capacity() + self.capacity() / 2 + 1;
        let new_cap = new_cap.max(growth_cap);
        let new_cap = new_cap.max(N); // Never shrink below N

        let new_layout = Layout::array::<T>(new_cap).expect("capacity overflow");

        let (new_ptr, old_len) = match &mut self.storage {
            Storage::Heap { ptr, len, cap } => {
                // Reallocate existing heap storage
                let old_layout = Layout::array::<T>(*cap).expect("layout error");
                // SAFETY: ptr is valid heap allocation from alloc, old_layout matches
                #[allow(unsafe_code)]
                let new_ptr = unsafe {
                    let old_ptr = ptr.as_ptr() as *mut u8;
                    let raw_ptr = alloc::realloc(old_ptr, old_layout, new_layout.size());
                    if raw_ptr.is_null() {
                        alloc::handle_alloc_error(new_layout);
                    }
                    raw_ptr as *mut T
                };
                (new_ptr, *len)
            }
            Storage::Inline { data, len } => {
                // Allocate new heap storage and move from inline
                // SAFETY: allocating with valid layout
                #[allow(unsafe_code)]
                let new_ptr = unsafe {
                    let ptr = alloc::alloc(new_layout);
                    if ptr.is_null() {
                        alloc::handle_alloc_error(new_layout);
                    }
                    ptr as *mut T
                };

                // Move elements from inline to heap
                // SAFETY: data contains len initialized elements, new_ptr is valid allocation
                #[allow(unsafe_code)]
                unsafe {
                    let inline_ptr = data.as_ptr() as *const T;
                    ptr::copy_nonoverlapping(inline_ptr, new_ptr, *len);
                }

                (new_ptr, *len)
            }
        };

        self.storage = Storage::Heap {
            ptr: NonNull::new(new_ptr).expect("null pointer after allocation"),
            len: old_len,
            cap: new_cap,
        };
    }

    /// Pushes an element to the back.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::small_vector::SmallVec;
    ///
    /// let mut vec: SmallVec<i32, 4> = SmallVec::new();
    /// vec.push(1);
    /// vec.push(2);
    /// assert_eq!(vec.len(), 2);
    /// ```
    pub fn push(&mut self, value: T) {
        if self.len() == self.capacity() {
            self.grow(self.len() + 1);
        }

        // SAFETY: we ensured capacity above, ptr is valid, len is within bounds
        #[allow(unsafe_code)]
        unsafe {
            let ptr = self.as_mut_ptr().add(self.len());
            ptr::write(ptr, value);
        }

        // Increment length
        match &mut self.storage {
            Storage::Inline { len, .. } => *len += 1,
            Storage::Heap { len, .. } => *len += 1,
        }
    }

    /// Removes and returns the last element.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::small_vector::SmallVec;
    ///
    /// let mut vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3]);
    /// assert_eq!(vec.pop(), Some(3));
    /// assert_eq!(vec.pop(), Some(2));
    /// ```
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        // Decrement length first
        match &mut self.storage {
            Storage::Inline { len, .. } => *len -= 1,
            Storage::Heap { len, .. } => *len -= 1,
        }

        // SAFETY: we just decremented len, so len is now the index of the last element
        #[allow(unsafe_code)]
        unsafe {
            let ptr = self.as_mut_ptr().add(self.len());
            Some(ptr::read(ptr))
        }
    }

    /// Clears the vector, removing all elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::small_vector::SmallVec;
    ///
    /// let mut vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3]);
    /// vec.clear();
    /// assert!(vec.is_empty());
    /// ```
    pub fn clear(&mut self) {
        // Drop all elements
        // SAFETY: ptr is valid, len is accurate
        #[allow(unsafe_code)]
        unsafe {
            let ptr = self.as_mut_ptr();
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(ptr, self.len()));
        }

        // Set length to 0
        match &mut self.storage {
            Storage::Inline { len, .. } => *len = 0,
            Storage::Heap { len, .. } => *len = 0,
        }
    }

    /// Inserts an element at position `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index > len`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::small_vector::SmallVec;
    ///
    /// let mut vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 3]);
    /// vec.insert(1, 2);
    /// assert_eq!(vec.as_slice(), &[1, 2, 3]);
    /// ```
    pub fn insert(&mut self, index: usize, value: T) {
        assert!(index <= self.len(), "index out of bounds");

        if self.len() == self.capacity() {
            self.grow(self.len() + 1);
        }

        // SAFETY: capacity checked above, ptr is valid, index is in bounds
        #[allow(unsafe_code)]
        unsafe {
            let ptr = self.as_mut_ptr();
            if index < self.len() {
                // Shift elements to make room
                ptr::copy(ptr.add(index), ptr.add(index + 1), self.len() - index);
            }
            ptr::write(ptr.add(index), value);
        }

        // Increment length
        match &mut self.storage {
            Storage::Inline { len, .. } => *len += 1,
            Storage::Heap { len, .. } => *len += 1,
        }
    }

    /// Removes and returns the element at position `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index >= len`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::small_vector::SmallVec;
    ///
    /// let mut vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3]);
    /// assert_eq!(vec.remove(1), 2);
    /// assert_eq!(vec.as_slice(), &[1, 3]);
    /// ```
    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len(), "index out of bounds");

        // SAFETY: index is checked above, ptr is valid
        #[allow(unsafe_code)]
        let value = unsafe {
            let ptr = self.as_mut_ptr();
            let value = ptr::read(ptr.add(index));
            // Shift elements to fill the gap
            ptr::copy(ptr.add(index + 1), ptr.add(index), self.len() - index - 1);
            value
        };

        // Decrement length
        match &mut self.storage {
            Storage::Inline { len, .. } => *len -= 1,
            Storage::Heap { len, .. } => *len -= 1,
        }

        value
    }

    /// Swaps two elements in the vector.
    ///
    /// # Panics
    ///
    /// Panics if `a >= len` or `b >= len`.
    pub fn swap(&mut self, a: usize, b: usize) {
        assert!(a < self.len() && b < self.len(), "index out of bounds");
        if a != b {
            // SAFETY: indices are bounds-checked above
            #[allow(unsafe_code)]
            unsafe {
                let ptr = self.as_mut_ptr();
                ptr::swap(ptr.add(a), ptr.add(b));
            }
        }
    }

    /// Removes an element by swapping it with the last element.
    ///
    /// This is O(1) but doesn't preserve order.
    ///
    /// # Panics
    ///
    /// Panics if `index >= len`.
    pub fn swap_remove(&mut self, index: usize) -> T {
        assert!(index < self.len(), "index out of bounds");
        let last = self.len() - 1;
        self.swap(index, last);
        // SAFETY: we just asserted len > 0, so pop will succeed
        self.pop().expect("pop after swap should succeed")
    }

    /// Resizes the vector to `new_len` elements.
    ///
    /// If `new_len > len`, new elements are initialized with `value`.
    /// If `new_len < len`, excess elements are dropped.
    pub fn resize(&mut self, new_len: usize, value: T)
    where
        T: Clone,
    {
        if new_len > self.len() {
            self.reserve(new_len - self.len());
            while self.len() < new_len {
                self.push(value.clone());
            }
        } else {
            self.truncate(new_len);
        }
    }

    /// Truncates the vector to `new_len` elements.
    ///
    /// If `new_len >= len`, this is a no-op.
    pub fn truncate(&mut self, new_len: usize) {
        if new_len < self.len() {
            // SAFETY: ptr is valid, count is within bounds
            #[allow(unsafe_code)]
            unsafe {
                let ptr = self.as_mut_ptr().add(new_len);
                let count = self.len() - new_len;
                ptr::drop_in_place(ptr::slice_from_raw_parts_mut(ptr, count));
            }

            // Update length
            match &mut self.storage {
                Storage::Inline { len, .. } => *len = new_len,
                Storage::Heap { len, .. } => *len = new_len,
            }
        }
    }

    /// Retains only elements that satisfy the predicate.
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        let mut i = 0;
        while i < self.len() {
            if f(&self[i]) {
                i += 1;
            } else {
                self.remove(i);
            }
        }
    }

    /// Extends the vector with elements from an iterator.
    pub fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        let (lower, _) = iter.size_hint();
        self.reserve(lower);
        for item in iter {
            self.push(item);
        }
    }

    /// Returns the first element, or None if empty.
    #[inline]
    pub fn first(&self) -> Option<&T> {
        self.as_slice().first()
    }

    /// Returns the first element mutably, or None if empty.
    #[inline]
    pub fn first_mut(&mut self) -> Option<&mut T> {
        self.as_mut_slice().first_mut()
    }

    /// Returns the last element, or None if empty.
    #[inline]
    pub fn last(&self) -> Option<&T> {
        self.as_slice().last()
    }

    /// Returns the last element mutably, or None if empty.
    #[inline]
    pub fn last_mut(&mut self) -> Option<&mut T> {
        self.as_mut_slice().last_mut()
    }

    /// Returns an iterator over the elements.
    #[inline]
    pub fn iter(&self) -> slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    /// Returns a mutable iterator over the elements.
    #[inline]
    pub fn iter_mut(&mut self) -> slice::IterMut<'_, T> {
        self.as_mut_slice().iter_mut()
    }

    /// Converts the vector into a boxed slice.
    pub fn into_boxed_slice(mut self) -> Box<[T]> {
        // Shrink to fit
        self.truncate(self.len());

        if self.is_empty() {
            return Vec::new().into_boxed_slice();
        }

        let len = self.len();
        let ptr = self.as_mut_ptr();

        // If heap allocated, we can use the existing allocation
        if let Storage::Heap { .. } = self.storage {
            // SAFETY: ptr is valid heap allocation with len elements
            #[allow(unsafe_code)]
            let slice = unsafe {
                let raw_slice = slice::from_raw_parts_mut(ptr, len);
                Box::from_raw(raw_slice)
            };
            // Prevent double-free
            mem::forget(self);
            slice
        } else {
            // Copy from inline to new allocation
            let mut vec = Vec::with_capacity(len);
            // SAFETY: ptr is valid inline storage with len initialized elements
            #[allow(unsafe_code)]
            unsafe {
                ptr::copy_nonoverlapping(ptr, vec.as_mut_ptr(), len);
                vec.set_len(len);
            }
            mem::forget(self);
            vec.into_boxed_slice()
        }
    }

    /// Converts the vector into a Vec.
    pub fn into_vec(self) -> Vec<T> {
        self.into_boxed_slice().into_vec()
    }
}

impl<T, const N: usize> Drop for SmallVec<T, N> {
    fn drop(&mut self) {
        // Drop all elements
        self.clear();

        // Free heap memory if allocated
        if let Storage::Heap { ptr, cap, .. } = &self.storage {
            let layout = Layout::array::<T>(*cap).expect("layout error in drop");
            // SAFETY: ptr is valid heap allocation from alloc with matching layout
            #[allow(unsafe_code)]
            unsafe {
                alloc::dealloc(ptr.as_ptr() as *mut u8, layout);
            }
        }
    }
}

impl<T: Clone, const N: usize> Clone for SmallVec<T, N> {
    fn clone(&self) -> Self {
        let mut new_vec = Self::with_capacity(self.len());
        for item in self.iter() {
            new_vec.push(item.clone());
        }
        new_vec
    }
}

impl<T, const N: usize> Default for SmallVec<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Deref for SmallVec<T, N> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, const N: usize> DerefMut for SmallVec<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<T, const N: usize> Index<usize> for SmallVec<T, N> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.as_slice()[index]
    }
}

impl<T, const N: usize> IndexMut<usize> for SmallVec<T, N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.as_mut_slice()[index]
    }
}

impl<T: PartialEq, const N: usize> PartialEq for SmallVec<T, N> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Eq, const N: usize> Eq for SmallVec<T, N> {}

impl<T: PartialOrd, const N: usize> PartialOrd for SmallVec<T, N> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl<T: Ord, const N: usize> Ord for SmallVec<T, N> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl<T: Hash, const N: usize> Hash for SmallVec<T, N> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for SmallVec<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T, const N: usize> IntoIterator for SmallVec<T, N> {
    type Item = T;
    type IntoIter = IntoIter<T, N>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            vec: self,
            index: 0,
        }
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a SmallVec<T, N> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a mut SmallVec<T, N> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T, const N: usize> FromIterator<T> for SmallVec<T, N> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        SmallVec::from_iter_items(iter)
    }
}

impl<T, const N: usize> Extend<T> for SmallVec<T, N> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        SmallVec::extend(self, iter);
    }
}

impl<T, const N: usize> From<Vec<T>> for SmallVec<T, N> {
    fn from(vec: Vec<T>) -> Self {
        SmallVec::from_iter_items(vec)
    }
}

impl<T: Clone, const N: usize> From<&[T]> for SmallVec<T, N> {
    fn from(slice: &[T]) -> Self {
        SmallVec::from_iter_items(slice.iter().cloned())
    }
}

impl<T, const N: usize, const M: usize> From<[T; M]> for SmallVec<T, N> {
    fn from(array: [T; M]) -> Self {
        SmallVec::from_iter_items(array)
    }
}

/// Owning iterator for SmallVec.
pub struct IntoIter<T, const N: usize> {
    vec: SmallVec<T, N>,
    index: usize,
}

impl<T, const N: usize> Iterator for IntoIter<T, N> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.vec.len() {
            // SAFETY: index is within bounds, ptr is valid
            #[allow(unsafe_code)]
            unsafe {
                let ptr = self.vec.as_ptr().add(self.index);
                self.index += 1;
                Some(ptr::read(ptr))
            }
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.vec.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl<T, const N: usize> ExactSizeIterator for IntoIter<T, N> {}
impl<T, const N: usize> FusedIterator for IntoIter<T, N> {}

impl<T, const N: usize> Drop for IntoIter<T, N> {
    fn drop(&mut self) {
        // Drop remaining elements by consuming the iterator
        for _ in self.by_ref() {}
        // Set len to 0 to prevent double-drop in SmallVec::drop
        match &mut self.vec.storage {
            Storage::Inline { len, .. } => *len = 0,
            Storage::Heap { len, .. } => *len = 0,
        }
    }
}

// SAFETY: SmallVec is Send/Sync if T is Send/Sync - owns its data exclusively
#[allow(unsafe_code)]
unsafe impl<T: Send, const N: usize> Send for SmallVec<T, N> {}

#[allow(unsafe_code)]
unsafe impl<T: Sync, const N: usize> Sync for SmallVec<T, N> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let vec: SmallVec<i32, 4> = SmallVec::new();
        assert!(vec.is_empty());
        assert_eq!(vec.capacity(), 4);
        assert!(vec.is_inline());
    }

    #[test]
    fn test_push_inline() {
        let mut vec: SmallVec<i32, 4> = SmallVec::new();
        vec.push(1);
        vec.push(2);
        vec.push(3);
        vec.push(4);

        assert_eq!(vec.len(), 4);
        assert!(vec.is_inline());
        assert_eq!(vec.as_slice(), &[1, 2, 3, 4]);
    }

    #[test]
    fn test_push_grow() {
        let mut vec: SmallVec<i32, 2> = SmallVec::new();
        vec.push(1);
        vec.push(2);
        assert!(vec.is_inline());

        vec.push(3);
        assert!(vec.is_heap());
        assert_eq!(vec.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_pop() {
        let mut vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3]);
        assert_eq!(vec.pop(), Some(3));
        assert_eq!(vec.pop(), Some(2));
        assert_eq!(vec.pop(), Some(1));
        assert_eq!(vec.pop(), None);
    }

    #[test]
    fn test_clear() {
        let mut vec: SmallVec<String, 4> =
            SmallVec::from_iter_items(["a", "b", "c"].map(String::from));
        vec.clear();
        assert!(vec.is_empty());
    }

    #[test]
    fn test_insert() {
        let mut vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 3]);
        vec.insert(1, 2);
        assert_eq!(vec.as_slice(), &[1, 2, 3]);

        vec.insert(0, 0);
        assert_eq!(vec.as_slice(), &[0, 1, 2, 3]);

        vec.insert(4, 4);
        assert_eq!(vec.as_slice(), &[0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_remove() {
        let mut vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3, 4]);
        assert_eq!(vec.remove(1), 2);
        assert_eq!(vec.as_slice(), &[1, 3, 4]);
    }

    #[test]
    fn test_swap_remove() {
        let mut vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3, 4]);
        assert_eq!(vec.swap_remove(1), 2);
        assert_eq!(vec.as_slice(), &[1, 4, 3]);
    }

    #[test]
    fn test_resize() {
        let mut vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2]);
        vec.resize(4, 0);
        assert_eq!(vec.as_slice(), &[1, 2, 0, 0]);

        vec.resize(2, 0);
        assert_eq!(vec.as_slice(), &[1, 2]);
    }

    #[test]
    fn test_retain() {
        let mut vec: SmallVec<i32, 8> = SmallVec::from_iter_items([1, 2, 3, 4, 5, 6]);
        vec.retain(|&x| x % 2 == 0);
        assert_eq!(vec.as_slice(), &[2, 4, 6]);
    }

    #[test]
    fn test_extend() {
        let mut vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2]);
        vec.extend([3, 4, 5]);
        assert_eq!(vec.as_slice(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_clone() {
        let vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3]);
        let cloned = vec.clone();
        assert_eq!(vec, cloned);
    }

    #[test]
    fn test_into_iter() {
        let vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3]);
        let collected: Vec<_> = vec.into_iter().collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }

    #[test]
    fn test_into_vec() {
        let vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3]);
        let std_vec = vec.into_vec();
        assert_eq!(std_vec, vec![1, 2, 3]);
    }

    #[test]
    fn test_equality() {
        let a: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3]);
        let b: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3]);
        let c: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 4]);

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_debug() {
        let vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3]);
        assert_eq!(format!("{:?}", vec), "[1, 2, 3]");
    }

    #[test]
    fn test_deref() {
        let vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3]);
        assert_eq!(vec.len(), 3);
        assert_eq!(vec[0], 1);
    }

    #[test]
    fn test_deref_mut() {
        let mut vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3]);
        vec[0] = 10;
        assert_eq!(vec[0], 10);
    }

    #[test]
    fn test_from_vec() {
        let std_vec = vec![1, 2, 3];
        let small_vec: SmallVec<i32, 4> = SmallVec::from(std_vec);
        assert_eq!(small_vec.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_from_slice() {
        let slice = &[1, 2, 3][..];
        let vec: SmallVec<i32, 4> = SmallVec::from(slice);
        assert_eq!(vec.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_from_array() {
        let vec: SmallVec<i32, 4> = SmallVec::from([1, 2, 3]);
        assert_eq!(vec.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_first_last() {
        let mut vec: SmallVec<i32, 4> = SmallVec::from_iter_items([1, 2, 3]);
        assert_eq!(vec.first(), Some(&1));
        assert_eq!(vec.last(), Some(&3));

        *vec.first_mut().unwrap() = 10;
        *vec.last_mut().unwrap() = 30;
        assert_eq!(vec.as_slice(), &[10, 2, 30]);
    }

    #[test]
    fn test_empty() {
        let mut vec: SmallVec<i32, 4> = SmallVec::new();
        assert!(vec.first().is_none());
        assert!(vec.last().is_none());
        assert_eq!(vec.pop(), None);
    }

    #[test]
    fn test_drop_elements() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let drop_count = Rc::new(RefCell::new(0));

        struct DropCounter(Rc<RefCell<i32>>);
        impl Drop for DropCounter {
            fn drop(&mut self) {
                *self.0.borrow_mut() += 1;
            }
        }

        {
            let mut vec: SmallVec<DropCounter, 2> = SmallVec::new();
            vec.push(DropCounter(drop_count.clone()));
            vec.push(DropCounter(drop_count.clone()));
            vec.push(DropCounter(drop_count.clone()));
            assert_eq!(*drop_count.borrow(), 0);
        }

        assert_eq!(*drop_count.borrow(), 3);
    }

    #[test]
    fn test_large_capacity() {
        let mut vec: SmallVec<i32, 2> = SmallVec::new();
        for i in 0..1000 {
            vec.push(i);
        }
        assert_eq!(vec.len(), 1000);
        assert!(vec.is_heap());
        assert_eq!(vec[999], 999);
    }
}
