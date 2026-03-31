//! Typed array with copy-on-write semantics.
//!
//! `Array<T>` is a typed array that uses copy-on-write (COW) semantics for
//! efficient sharing. This is the Rust equivalent of OpenUSD's `VtArray<T>`.
//!
//! # Examples
//!
//! ```
//! use usd_vt::Array;
//!
//! // Create an array
//! let arr: Array<f32> = Array::from(vec![1.0, 2.0, 3.0]);
//! assert_eq!(arr.len(), 3);
//!
//! // Clone is cheap (shared reference)
//! let arr2 = arr.clone();
//! assert_eq!(arr2.len(), 3);
//!
//! // Modification triggers copy
//! let mut arr3 = arr.clone();
//! arr3.push(4.0);
//! assert_eq!(arr3.len(), 4);
//! assert_eq!(arr.len(), 3); // Original unchanged
//! ```

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, BitAnd, BitOr, BitXor, Deref, Div, Index, IndexMut, Mul, Neg, Rem, Sub};
use std::sync::Arc;

// ============================================================================
// ShapeData - Multidimensional array shape information
// ============================================================================

/// Maximum number of additional dimensions supported (beyond the last dimension).
pub const MAX_OTHER_DIMS: usize = 3;

/// Shape data for multidimensional arrays.
///
/// Stores the total size and dimensions for arrays of rank up to 4.
/// The size of the last dimension is computed as `total_size / (product of other dims)`.
///
/// Matches C++ `Vt_ShapeData`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ShapeData {
    /// Total number of elements in the array.
    pub total_size: usize,
    /// Dimensions other than the last (0 means that dimension is unused).
    pub other_dims: [u32; MAX_OTHER_DIMS],
}

impl ShapeData {
    /// Creates a new 1D shape with the given size.
    pub const fn new_1d(size: usize) -> Self {
        Self {
            total_size: size,
            other_dims: [0, 0, 0],
        }
    }

    /// Creates a 2D shape.
    pub const fn new_2d(rows: u32, cols: usize) -> Self {
        Self {
            total_size: rows as usize * cols,
            other_dims: [rows, 0, 0],
        }
    }

    /// Creates a 3D shape.
    pub const fn new_3d(d0: u32, d1: u32, d2: usize) -> Self {
        Self {
            total_size: d0 as usize * d1 as usize * d2,
            other_dims: [d0, d1, 0],
        }
    }

    /// Creates a 4D shape.
    pub const fn new_4d(d0: u32, d1: u32, d2: u32, d3: usize) -> Self {
        Self {
            total_size: d0 as usize * d1 as usize * d2 as usize * d3,
            other_dims: [d0, d1, d2],
        }
    }

    /// Returns the rank (number of dimensions).
    ///
    /// Matches C++ `GetRank()`.
    #[inline]
    pub fn rank(&self) -> u32 {
        if self.other_dims[0] == 0 {
            1
        } else if self.other_dims[1] == 0 {
            2
        } else if self.other_dims[2] == 0 {
            3
        } else {
            4
        }
    }

    /// Returns the size of the last dimension.
    pub fn last_dim_size(&self) -> usize {
        let product: usize = self
            .other_dims
            .iter()
            .filter(|&&d| d != 0)
            .map(|&d| d as usize)
            .product();
        if product == 0 {
            self.total_size
        } else {
            self.total_size / product
        }
    }

    /// Returns all dimensions as a vector.
    pub fn dims(&self) -> Vec<usize> {
        let mut result: Vec<usize> = self
            .other_dims
            .iter()
            .take_while(|&&d| d != 0)
            .map(|&d| d as usize)
            .collect();
        result.push(self.last_dim_size());
        result
    }

    /// Clears the shape data.
    pub fn clear(&mut self) {
        self.total_size = 0;
        self.other_dims = [0, 0, 0];
    }

    /// Returns true if this is a 1D array.
    #[inline]
    pub fn is_1d(&self) -> bool {
        self.other_dims[0] == 0
    }
}

// ============================================================================
// ForeignDataSource - External memory ownership for arrays
// ============================================================================

/// A callback type for when foreign data is detached from an array.
pub type DetachCallback = Box<dyn Fn() + Send + Sync>;

/// Foreign data source for arrays.
///
/// Allows arrays to reference external memory without owning it.
/// Matches C++ `Vt_ArrayForeignDataSource`. When all arrays referencing
/// this source are detached (mutated or dropped), `detach_fn` is called.
///
/// Create via `ForeignDataSource::new(...)` which returns an `Arc<Self>`
/// to be passed to `Array::from_foreign`.
pub struct ForeignDataSource {
    /// Callback invoked when no more arrays reference this source.
    detach_fn: Option<DetachCallback>,
}

impl ForeignDataSource {
    /// Creates a new foreign data source with an optional detach callback.
    ///
    /// Returns `Arc<Self>` — pass it to `Array::from_foreign`.
    pub fn new(detach_fn: Option<DetachCallback>) -> Arc<Self> {
        Arc::new(Self { detach_fn })
    }

    /// Called internally when the last array referencing this source is dropped.
    fn arrays_detached(&self) {
        if let Some(ref cb) = self.detach_fn {
            cb();
        }
    }
}

impl std::fmt::Debug for ForeignDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ForeignDataSource")
            .field("has_detach_fn", &self.detach_fn.is_some())
            .finish()
    }
}

// ============================================================================
// ForeignSlice - Non-owning view into foreign memory, shared via Arc
// ============================================================================

/// Raw pointer wrapper that is Send+Sync because the foreign data is
/// guaranteed valid for the lifetime of the ForeignDataSource.
struct RawPtr<T>(*const T);
// SAFETY: The pointer points to data managed by the ForeignDataSource which
// is required to outlive all Array references; the data is immutable while
// any Foreign array exists (mutation triggers a copy first).
#[allow(unsafe_code)]
unsafe impl<T: Send> Send for RawPtr<T> {}
#[allow(unsafe_code)]
unsafe impl<T: Sync> Sync for RawPtr<T> {}

/// Shared handle to a foreign (externally-owned) slice.
/// When the last clone of this is dropped, it decrements the source's
/// ref-count and fires the detach callback if it reaches zero.
struct ForeignSlice<T> {
    /// The external data source that owns the memory lifetime.
    source: Arc<ForeignDataSource>,
    /// Pointer to the first element of foreign data.
    ptr: RawPtr<T>,
    /// Number of elements.
    len: usize,
}

impl<T> ForeignSlice<T> {
    /// Returns the foreign data as a slice.
    ///
    /// # Safety
    /// Caller must ensure source is alive and data has not been freed.
    #[inline]
    fn as_slice(&self) -> &[T] {
        // SAFETY: ptr/len were provided by the caller of Array::from_foreign
        // and must remain valid for the lifetime of this ForeignSlice.
        #[allow(unsafe_code)]
        unsafe {
            std::slice::from_raw_parts(self.ptr.0, self.len)
        }
    }
}

impl<T> Drop for ForeignSlice<T> {
    fn drop(&mut self) {
        // Called when the last Arc<ForeignSlice> is dropped, meaning no more
        // arrays reference this foreign data. Fire the detach callback.
        self.source.arrays_detached();
    }
}

// ForeignSlice is shared through Arc so Debug is nice to have.
impl<T> std::fmt::Debug for ForeignSlice<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ForeignSlice")
            .field("ptr", &self.ptr.0)
            .field("len", &self.len)
            .finish()
    }
}

// ============================================================================
// ArrayStorage - owned vs foreign storage enum
// ============================================================================

/// Internal storage for `Array<T>`.
///
/// - `Owned`: normal COW storage via `Arc<Vec<T>>`.
/// - `Foreign`: zero-copy reference to externally-managed data.
#[derive(Debug)]
enum ArrayStorage<T: Clone + Send + Sync + 'static> {
    /// Normal owned copy-on-write storage.
    Owned(Arc<Vec<T>>),
    /// Non-owning reference to foreign data; copy on first mutation.
    Foreign(Arc<ForeignSlice<T>>),
}

impl<T: Clone + Send + Sync + 'static> Clone for ArrayStorage<T> {
    fn clone(&self) -> Self {
        match self {
            ArrayStorage::Owned(arc) => ArrayStorage::Owned(Arc::clone(arc)),
            // Arc::clone increments the Arc strong-count; ForeignSlice::drop
            // fires the callback only when the last Arc is gone.
            ArrayStorage::Foreign(fs) => ArrayStorage::Foreign(Arc::clone(fs)),
        }
    }
}

impl<T: Clone + Send + Sync + 'static> ArrayStorage<T> {
    /// Returns the data as a slice without triggering a detach.
    #[inline]
    fn as_slice(&self) -> &[T] {
        match self {
            ArrayStorage::Owned(arc) => arc.as_slice(),
            ArrayStorage::Foreign(fs) => fs.as_slice(),
        }
    }

    /// Returns the number of elements.
    #[inline]
    fn len(&self) -> usize {
        match self {
            ArrayStorage::Owned(arc) => arc.len(),
            ArrayStorage::Foreign(fs) => fs.len,
        }
    }

    /// Returns the capacity (foreign data has capacity == len).
    #[inline]
    fn capacity(&self) -> usize {
        match self {
            ArrayStorage::Owned(arc) => arc.capacity(),
            ArrayStorage::Foreign(fs) => fs.len,
        }
    }

    /// Returns true if the storage is exclusively owned (no other sharers).
    #[inline]
    fn is_unique(&self) -> bool {
        match self {
            ArrayStorage::Owned(arc) => Arc::strong_count(arc) == 1,
            // Foreign is unique if this Array is the only one holding the Arc.
            ArrayStorage::Foreign(fs) => Arc::strong_count(fs) == 1,
        }
    }

    /// True if two storages point to the same allocation.
    #[inline]
    fn ptr_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ArrayStorage::Owned(a), ArrayStorage::Owned(b)) => Arc::ptr_eq(a, b),
            (ArrayStorage::Foreign(a), ArrayStorage::Foreign(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// Detaches from foreign (or shared owned) storage, ensuring unique
    /// mutable ownership. Returns true if a copy was made.
    fn make_unique(&mut self) -> bool {
        match self {
            ArrayStorage::Owned(arc) => {
                if Arc::strong_count(arc) == 1 {
                    return false;
                }
                *arc = Arc::new((**arc).clone());
                true
            }
            ArrayStorage::Foreign(fs) => {
                // Copy foreign data into a new owned Vec.
                let vec = fs.as_slice().to_vec();
                // Drop our share of the foreign slice (may fire detach callback).
                *self = ArrayStorage::Owned(Arc::new(vec));
                true
            }
        }
    }

    /// Returns a mutable reference to the inner Arc<Vec<T>>, assuming
    /// the storage is Owned. Panics in debug if called on Foreign.
    #[inline]
    fn owned_arc_mut(&mut self) -> &mut Arc<Vec<T>> {
        match self {
            ArrayStorage::Owned(arc) => arc,
            ArrayStorage::Foreign(_) => {
                panic!("owned_arc_mut called on Foreign storage — call make_unique first")
            }
        }
    }
}

/// A typed array with copy-on-write semantics.
///
/// `Array<T>` stores elements of type `T` and uses copy-on-write (COW)
/// semantics for efficient cloning. The array is only copied when a
/// mutation is performed on a shared array.
///
/// # Type Requirements
///
/// Elements must be `Clone + Send + Sync + 'static`.
///
/// # Examples
///
/// ```
/// use usd_vt::Array;
///
/// let arr: Array<i32> = Array::from(vec![1, 2, 3, 4, 5]);
///
/// // Iteration
/// for &x in arr.iter() {
///     println!("{}", x);
/// }
///
/// // Indexing
/// assert_eq!(arr[0], 1);
/// assert_eq!(arr[4], 5);
/// ```
pub struct Array<T: Clone + Send + Sync + 'static> {
    /// Storage: either owned COW Vec or a zero-copy foreign slice.
    storage: ArrayStorage<T>,
    /// Multidimensional shape information (matches C++ Vt_ShapeData).
    shape: ShapeData,
}

impl<T: Clone + Send + Sync + 'static> Clone for Array<T> {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
            shape: self.shape,
        }
    }
}

impl<T: Clone + Send + Sync + 'static> Array<T> {
    /// Creates a new empty array.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr: Array<i32> = Array::new();
    /// assert!(arr.is_empty());
    /// ```
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage: ArrayStorage::Owned(Arc::new(Vec::new())),
            shape: ShapeData::new_1d(0),
        }
    }

    /// Creates an array with the given capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr: Array<i32> = Array::with_capacity(100);
    /// assert!(arr.is_empty());
    /// ```
    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            storage: ArrayStorage::Owned(Arc::new(Vec::with_capacity(capacity))),
            shape: ShapeData::new_1d(0),
        }
    }

    /// Creates an array with `n` copies of `value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr: Array<f32> = Array::from_elem(0.0, 10);
    /// assert_eq!(arr.len(), 10);
    /// assert!(arr.iter().all(|&x| x == 0.0));
    /// ```
    #[inline]
    #[must_use]
    pub fn from_elem(value: T, n: usize) -> Self {
        Self {
            storage: ArrayStorage::Owned(Arc::new(vec![value; n])),
            shape: ShapeData::new_1d(n),
        }
    }

    /// Returns the number of elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// assert_eq!(arr.len(), 3);
    /// ```
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Returns true if the array is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let empty: Array<i32> = Array::new();
    /// let full: Array<i32> = Array::from(vec![1]);
    ///
    /// assert!(empty.is_empty());
    /// assert!(!full.is_empty());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.storage.len() == 0
    }

    /// Returns a slice of the array contents.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// let slice = arr.as_slice();
    /// assert_eq!(slice, &[1, 2, 3]);
    /// ```
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        self.storage.as_slice()
    }

    /// Returns a pointer to the data.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// let ptr = arr.as_ptr();
    /// unsafe {
    ///     assert_eq!(*ptr, 1);
    /// }
    /// ```
    #[inline]
    #[must_use]
    pub fn as_ptr(&self) -> *const T {
        self.storage.as_slice().as_ptr()
    }

    /// Returns an iterator over the array.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// let sum: i32 = arr.iter().sum();
    /// assert_eq!(sum, 6);
    /// ```
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.storage.as_slice().iter()
    }

    /// Returns the first element, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// assert_eq!(arr.first(), Some(&1));
    ///
    /// let empty: Array<i32> = Array::new();
    /// assert_eq!(empty.first(), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn first(&self) -> Option<&T> {
        self.storage.as_slice().first()
    }

    /// Returns the last element, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// assert_eq!(arr.last(), Some(&3));
    /// ```
    #[inline]
    #[must_use]
    pub fn last(&self) -> Option<&T> {
        self.storage.as_slice().last()
    }

    /// Returns true if this array shares storage with another.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr1: Array<i32> = Array::from(vec![1, 2, 3]);
    /// let arr2 = arr1.clone();
    /// let arr3: Array<i32> = Array::from(vec![1, 2, 3]);
    ///
    /// assert!(arr1.is_shared_with(&arr2));
    /// assert!(!arr1.is_shared_with(&arr3));
    /// ```
    #[inline]
    #[must_use]
    pub fn is_shared_with(&self, other: &Self) -> bool {
        self.storage.ptr_eq(&other.storage)
    }

    /// Returns true if this array has unique ownership.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr1: Array<i32> = Array::from(vec![1, 2, 3]);
    /// assert!(arr1.is_unique());
    ///
    /// let arr2 = arr1.clone();
    /// // Now neither is unique
    /// ```
    #[inline]
    #[must_use]
    pub fn is_unique(&self) -> bool {
        self.storage.is_unique()
    }

    /// Ensures the array has unique ownership, cloning data if shared.
    ///
    /// Returns `true` if a copy was made (i.e. was shared), `false` if already unique.
    ///
    /// Matches C++ `VtArray::MakeUnique()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr1: Array<i32> = Array::from(vec![1, 2, 3]);
    /// let mut arr2 = arr1.clone();
    /// assert!(!arr2.is_unique());
    ///
    /// let copied = arr2.make_unique();
    /// assert!(copied);
    /// assert!(arr2.is_unique());
    ///
    /// let copied2 = arr2.make_unique();
    /// assert!(!copied2); // Already unique
    /// ```
    pub fn make_unique(&mut self) -> bool {
        self.storage.make_unique()
    }

    /// Returns a const reference to self.
    ///
    /// In Rust shared references are inherently immutable, so this is a no-op
    /// provided for API compatibility with C++ `VtArray::AsConst()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// let c = arr.as_const();
    /// assert_eq!(c.len(), 3);
    /// ```
    #[inline]
    #[must_use]
    pub fn as_const(&self) -> &Self {
        self
    }

    /// Tests if two arrays share the same underlying copy-on-write data.
    ///
    /// Unlike `PartialEq` which compares contents, this checks pointer identity.
    ///
    /// Matches C++ `VtArray::IsIdentical()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr1: Array<i32> = Array::from(vec![1, 2, 3]);
    /// let arr2 = arr1.clone();
    /// let arr3: Array<i32> = Array::from(vec![1, 2, 3]);
    ///
    /// assert!(arr1.is_identical(&arr2)); // Same Arc
    /// assert!(!arr1.is_identical(&arr3)); // Different alloc, same content
    /// ```
    #[inline]
    #[must_use]
    pub fn is_identical(&self, other: &Self) -> bool {
        self.storage.ptr_eq(&other.storage)
    }

    /// Pushes an element to the end of the array.
    ///
    /// If the array is shared, it will be copied first.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::new();
    /// arr.push(1);
    /// arr.push(2);
    /// arr.push(3);
    /// assert_eq!(arr.len(), 3);
    /// ```
    #[inline]
    pub fn push(&mut self, value: T) {
        self.storage.make_unique();
        Arc::make_mut(self.storage.owned_arc_mut()).push(value);
        self.shape = ShapeData::new_1d(self.storage.len());
    }

    /// Removes and returns the last element.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// assert_eq!(arr.pop(), Some(3));
    /// assert_eq!(arr.len(), 2);
    /// ```
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        self.storage.make_unique();
        let result = Arc::make_mut(self.storage.owned_arc_mut()).pop();
        self.shape = ShapeData::new_1d(self.storage.len());
        result
    }

    /// Clears the array, removing all elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// arr.clear();
    /// assert!(arr.is_empty());
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.storage.make_unique();
        Arc::make_mut(self.storage.owned_arc_mut()).clear();
        self.shape = ShapeData::new_1d(0);
    }

    /// Resizes the array to `new_len` elements.
    ///
    /// If `new_len` is greater than the current length, `value` is used
    /// to fill the new slots.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// arr.resize(5, 0);
    /// assert_eq!(arr.as_slice(), &[1, 2, 3, 0, 0]);
    /// ```
    #[inline]
    pub fn resize(&mut self, new_len: usize, value: T) {
        self.storage.make_unique();
        Arc::make_mut(self.storage.owned_arc_mut()).resize(new_len, value);
        self.shape = ShapeData::new_1d(self.storage.len());
    }

    /// Reserves capacity for at least `num` total elements.
    ///
    /// Matches C++ `VtArray::reserve(size_t num)` which takes an absolute
    /// count, not a relative "additional" count like `Vec::reserve`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::new();
    /// arr.reserve(100);
    /// assert!(arr.capacity() >= 100);
    /// ```
    #[inline]
    pub fn reserve(&mut self, num: usize) {
        let current_cap = self.storage.capacity();
        if num <= current_cap {
            return;
        }
        self.storage.make_unique();
        let current_len = self.storage.len();
        let additional = num.saturating_sub(current_len);
        Arc::make_mut(self.storage.owned_arc_mut()).reserve(additional);
    }

    /// Returns the number of elements the array can hold without reallocating.
    ///
    /// Matches C++ `VtArray::capacity()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::with_capacity(100);
    /// assert!(arr.capacity() >= 100);
    /// ```
    #[inline]
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.storage.capacity()
    }

    /// Shrinks the capacity of the array as much as possible.
    ///
    /// Matches C++ `VtArray::shrink_to_fit()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::with_capacity(100);
    /// arr.push(1);
    /// arr.shrink_to_fit();
    /// assert!(arr.capacity() < 100);
    /// ```
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.storage.make_unique();
        Arc::make_mut(self.storage.owned_arc_mut()).shrink_to_fit();
    }

    /// Swaps the contents of two arrays.
    ///
    /// Matches C++ `VtArray::swap(VtArray&)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr1: Array<i32> = Array::from(vec![1, 2, 3]);
    /// let mut arr2: Array<i32> = Array::from(vec![4, 5]);
    /// arr1.swap(&mut arr2);
    /// assert_eq!(arr1.as_slice(), &[4, 5]);
    /// assert_eq!(arr2.as_slice(), &[1, 2, 3]);
    /// ```
    #[inline]
    pub fn swap(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.storage, &mut other.storage);
        std::mem::swap(&mut self.shape, &mut other.shape);
    }

    /// Returns a reference to the element at the given index, or None if out of bounds.
    ///
    /// Matches C++ `VtArray::at(size_t)` (bounds-checked version).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// assert_eq!(arr.at(1), Some(&2));
    /// assert_eq!(arr.at(10), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn at(&self, index: usize) -> Option<&T> {
        self.storage.as_slice().get(index)
    }

    /// Returns a mutable reference to the first element, or None if empty.
    ///
    /// Matches C++ `VtArray::front()` (mutable version).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// if let Some(front) = arr.front_mut() {
    ///     *front = 10;
    /// }
    /// assert_eq!(arr[0], 10);
    /// ```
    #[inline]
    pub fn front_mut(&mut self) -> Option<&mut T> {
        self.storage.make_unique();
        Arc::make_mut(self.storage.owned_arc_mut()).first_mut()
    }

    /// Returns a mutable reference to the last element, or None if empty.
    ///
    /// Matches C++ `VtArray::back()` (mutable version).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// if let Some(back) = arr.back_mut() {
    ///     *back = 30;
    /// }
    /// assert_eq!(arr[2], 30);
    /// ```
    #[inline]
    pub fn back_mut(&mut self) -> Option<&mut T> {
        self.storage.make_unique();
        Arc::make_mut(self.storage.owned_arc_mut()).last_mut()
    }

    /// Returns the theoretical maximum number of elements the array can hold.
    ///
    /// Matches C++ `VtArray::max_size()`. Uses half-address-space limit
    /// (ptrdiff_t::max - 1) divided by element size.
    #[inline]
    #[must_use]
    pub fn max_size(&self) -> usize {
        // C++: (numeric_limits<ptrdiff_t>::max() - 1 - sizeof(_ControlBlock)) / sizeof(T)
        // We don't have _ControlBlock (use Arc<Vec<T>>), so just subtract 1.
        (isize::MAX as usize - 1) / std::mem::size_of::<T>()
    }

    /// Resizes the array to `new_len` elements, value-initializing new elements.
    ///
    /// Matches C++ `VtArray::resize(size_t)` (without value parameter).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// arr.resize_no_value(5); // New elements are 0 (value-initialized)
    /// assert_eq!(arr.len(), 5);
    /// ```
    #[inline]
    pub fn resize_no_value(&mut self, new_len: usize)
    where
        T: Default,
    {
        self.resize(new_len, T::default());
    }

    /// Inserts an element at the given index.
    ///
    /// Matches C++ `VtArray::insert(const_iterator, const T&)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::from(vec![1, 2, 4]);
    /// arr.insert(2, 3);
    /// assert_eq!(arr.as_slice(), &[1, 2, 3, 4]);
    /// ```
    #[inline]
    pub fn insert(&mut self, index: usize, value: T) {
        self.storage.make_unique();
        Arc::make_mut(self.storage.owned_arc_mut()).insert(index, value);
        self.shape = ShapeData::new_1d(self.storage.len());
    }

    /// Removes and returns the element at the given index.
    ///
    /// Matches C++ `VtArray::erase(const_iterator)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::from(vec![1, 2, 3, 4]);
    /// let removed = arr.remove(1);
    /// assert_eq!(removed, Some(2));
    /// assert_eq!(arr.as_slice(), &[1, 3, 4]);
    /// ```
    #[inline]
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len() {
            return None;
        }
        self.storage.make_unique();
        let result = Arc::make_mut(self.storage.owned_arc_mut()).remove(index);
        self.shape = ShapeData::new_1d(self.storage.len());
        Some(result)
    }

    /// Assigns new contents to the array, replacing its current contents.
    ///
    /// Matches C++ `VtArray::assign(ForwardIter, ForwardIter)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// arr.assign(vec![4, 5, 6].into_iter());
    /// assert_eq!(arr.as_slice(), &[4, 5, 6]);
    /// ```
    pub fn assign<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.storage.make_unique();
        *Arc::make_mut(self.storage.owned_arc_mut()) = iter.into_iter().collect();
        self.shape = ShapeData::new_1d(self.storage.len());
    }

    /// Returns a mutable slice if this array has unique ownership.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// if let Some(slice) = arr.as_mut_slice() {
    ///     slice[0] = 10;
    /// }
    /// assert_eq!(arr[0], 10);
    /// ```
    #[inline]
    pub fn as_mut_slice(&mut self) -> Option<&mut [T]> {
        // Foreign storage is never uniquely mutable; owned only if Arc is unique.
        match &mut self.storage {
            ArrayStorage::Owned(arc) => Arc::get_mut(arc).map(|v| v.as_mut_slice()),
            ArrayStorage::Foreign(_) => None,
        }
    }

    /// Converts to a Vec, cloning if necessary.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// let v = arr.into_vec();
    /// assert_eq!(v, vec![1, 2, 3]);
    /// ```
    #[inline]
    pub fn into_vec(self) -> Vec<T> {
        match self.storage {
            ArrayStorage::Owned(arc) => match Arc::try_unwrap(arc) {
                Ok(vec) => vec,
                Err(shared) => (*shared).clone(),
            },
            ArrayStorage::Foreign(fs) => fs.as_slice().to_vec(),
        }
    }

    /// Constructs an array from a raw pointer and length.
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - `ptr` points to `len` consecutive properly initialized values of type `T`
    /// - The memory is valid for the lifetime of the array
    /// - The memory will not be freed or modified externally
    ///
    /// This is primarily used for wrapping foreign data sources.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let data = vec![1, 2, 3, 4, 5];
    /// let ptr = data.as_ptr();
    /// let len = data.len();
    ///
    /// // SAFETY: data is valid and won't be freed
    /// let arr = unsafe { Array::from_raw_parts(ptr, len) };
    /// std::mem::forget(data); // Prevent double-free
    /// ```
    #[inline]
    #[allow(unsafe_code)]
    pub unsafe fn from_raw_parts(ptr: *const T, len: usize) -> Self {
        // SAFETY: Caller guarantees ptr points to len valid elements of type T.
        // Data is copied into owned storage; no foreign source tracking.
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
        let vec = slice.to_vec();
        Self {
            storage: ArrayStorage::Owned(Arc::new(vec)),
            shape: ShapeData::new_1d(len),
        }
    }

    /// Constructs an array that wraps foreign-owned data without copying.
    ///
    /// The array will read directly from `ptr` for `len` elements. On the first
    /// mutation (push, resize, index_mut, etc.), the data is copied into owned
    /// storage and the `ForeignDataSource` detach callback is fired when the
    /// last reference to the foreign slice is dropped.
    ///
    /// Matches C++ `VtArray(Vt_ArrayForeignDataSource*, ElementType*, size_t, bool)`.
    ///
    /// # Safety
    ///
    /// - `ptr` must point to at least `len` consecutive initialized `T` values.
    /// - The memory must remain valid and unmodified for as long as the array
    ///   (or any clone of it) exists in Foreign state, i.e., until a mutation
    ///   causes a detach.
    /// - The `source` must outlive all clones of the returned array.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
    /// use usd_vt::{Array, ForeignDataSource};
    ///
    /// let data = vec![10i32, 20, 30];
    /// let detached = Arc::new(AtomicBool::new(false));
    /// let d = detached.clone();
    /// let source = ForeignDataSource::new(Some(Box::new(move || {
    ///     d.store(true, Ordering::SeqCst);
    /// })));
    ///
    /// // SAFETY: data lives longer than the array in this example.
    /// let arr = unsafe { Array::from_foreign(source, data.as_ptr(), data.len()) };
    /// assert_eq!(arr.as_slice(), &[10, 20, 30]);
    /// assert!(!detached.load(Ordering::SeqCst)); // not detached yet
    ///
    /// // Mutation detaches and fires callback.
    /// let mut arr2 = arr.clone();
    /// arr2.push(40);
    /// assert!(!detached.load(Ordering::SeqCst)); // arr still holds foreign ref
    /// drop(arr);
    /// assert!(detached.load(Ordering::SeqCst)); // last ref dropped
    /// ```
    #[inline]
    #[allow(unsafe_code)]
    pub unsafe fn from_foreign(source: Arc<ForeignDataSource>, ptr: *const T, len: usize) -> Self {
        let fs = Arc::new(ForeignSlice {
            source,
            ptr: RawPtr(ptr),
            len,
        });
        Self {
            storage: ArrayStorage::Foreign(fs),
            shape: ShapeData::new_1d(len),
        }
    }

    /// Returns true if this array is backed by foreign (externally-owned) data.
    ///
    /// Foreign arrays avoid a copy on construction but will copy on first mutation.
    #[inline]
    #[must_use]
    pub fn is_foreign(&self) -> bool {
        matches!(self.storage, ArrayStorage::Foreign(_))
    }

    /// Assigns `n` copies of `value` to the array.
    ///
    /// Matches C++ `VtArray::assign(size_t, const T&)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::new();
    /// arr.assign_fill(5, 42);
    /// assert_eq!(arr.as_slice(), &[42, 42, 42, 42, 42]);
    /// ```
    #[inline]
    pub fn assign_fill(&mut self, n: usize, value: T) {
        self.storage.make_unique();
        *Arc::make_mut(self.storage.owned_arc_mut()) = vec![value; n];
        self.shape = ShapeData::new_1d(n);
    }

    /// Inserts multiple copies of a value at the given index.
    ///
    /// Matches C++ `VtArray::insert(const_iterator, size_t, const T&)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::from(vec![1, 2, 5, 6]);
    /// arr.insert_n(2, 2, 3); // Insert two 3's at index 2
    /// assert_eq!(arr.as_slice(), &[1, 2, 3, 3, 5, 6]);
    /// ```
    #[inline]
    pub fn insert_n(&mut self, index: usize, count: usize, value: T) {
        if count == 0 {
            return;
        }
        self.storage.make_unique();
        let data = Arc::make_mut(self.storage.owned_arc_mut());
        let values: Vec<T> = vec![value; count];
        data.splice(index..index, values);
        self.shape = ShapeData::new_1d(data.len());
    }

    /// Removes a range of elements.
    ///
    /// Matches C++ `VtArray::erase(const_iterator, const_iterator)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::from(vec![1, 2, 3, 4, 5]);
    /// arr.remove_range(1, 3); // Remove indices 1 and 2
    /// assert_eq!(arr.as_slice(), &[1, 4, 5]);
    /// ```
    #[inline]
    pub fn remove_range(&mut self, start: usize, end: usize) {
        if start >= end || start >= self.len() {
            return;
        }
        self.storage.make_unique();
        Arc::make_mut(self.storage.owned_arc_mut()).drain(start..end);
        self.shape = ShapeData::new_1d(self.storage.len());
    }

    /// Appends an element to the array (alias for push).
    ///
    /// Matches C++ `VtArray::emplace_back(Args&&...)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let mut arr: Array<i32> = Array::new();
    /// arr.emplace_back(1);
    /// arr.emplace_back(2);
    /// assert_eq!(arr.as_slice(), &[1, 2]);
    /// ```
    #[inline]
    pub fn emplace_back(&mut self, value: T) {
        self.push(value);
    }

    /// Returns a mutable reference to the data pointer.
    ///
    /// # Safety
    ///
    /// The returned pointer is only valid while the array is not reallocated.
    /// Prefer using safe methods when possible.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.storage.make_unique();
        Arc::make_mut(self.storage.owned_arc_mut()).as_mut_ptr()
    }

    /// Returns true if this array is uniquely owned (not shared).
    ///
    /// Matches C++ behavior where unique arrays can be mutated without copying.
    #[inline]
    #[must_use]
    pub fn is_unique_ownership(&self) -> bool {
        self.storage.is_unique()
    }

    /// Returns the shape data for this array.
    ///
    /// Matches C++ `VtArray::GetShapeData()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// assert_eq!(arr.shape_data().total_size, 3);
    /// assert!(arr.shape_data().is_1d());
    /// ```
    #[inline]
    #[must_use]
    pub fn shape_data(&self) -> &ShapeData {
        &self.shape
    }

    /// Sets the shape data, returning true if successful.
    ///
    /// Fails if `shape.total_size` doesn't match the current length.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::array::{Array, ShapeData};
    ///
    /// let mut arr: Array<i32> = Array::from(vec![1, 2, 3, 4, 5, 6]);
    /// assert!(arr.set_shape(ShapeData::new_2d(2, 3)));
    /// assert_eq!(arr.rank(), 2);
    ///
    /// // Wrong size fails
    /// assert!(!arr.set_shape(ShapeData::new_2d(3, 3)));
    /// ```
    pub fn set_shape(&mut self, shape: ShapeData) -> bool {
        if shape.total_size != self.storage.len() {
            return false;
        }
        self.shape = shape;
        true
    }

    /// Returns the rank (number of dimensions) of this array.
    ///
    /// Matches C++ `VtArray::GetShapeData()->GetRank()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Array;
    ///
    /// let arr: Array<i32> = Array::from(vec![1, 2, 3]);
    /// assert_eq!(arr.rank(), 1);
    /// ```
    #[inline]
    #[must_use]
    pub fn rank(&self) -> u32 {
        self.shape.rank()
    }
}

impl<T: Clone + Send + Sync + 'static> Default for Array<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Send + Sync + 'static> From<Vec<T>> for Array<T> {
    #[inline]
    fn from(vec: Vec<T>) -> Self {
        let len = vec.len();
        Self {
            storage: ArrayStorage::Owned(Arc::new(vec)),
            shape: ShapeData::new_1d(len),
        }
    }
}

impl<T: Clone + Send + Sync + 'static> FromIterator<T> for Array<T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let vec: Vec<T> = iter.into_iter().collect();
        let len = vec.len();
        Self {
            storage: ArrayStorage::Owned(Arc::new(vec)),
            shape: ShapeData::new_1d(len),
        }
    }
}

impl<T: Clone + Send + Sync + 'static> Deref for Array<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.storage.as_slice()
    }
}

impl<T: Clone + Send + Sync + 'static> Index<usize> for Array<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        &self.storage.as_slice()[index]
    }
}

impl<T: Clone + Send + Sync + 'static> IndexMut<usize> for Array<T> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.storage.make_unique();
        &mut Arc::make_mut(self.storage.owned_arc_mut())[index]
    }
}

impl<T: Clone + Send + Sync + fmt::Debug + 'static> fmt::Debug for Array<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.storage.as_slice().iter())
            .finish()
    }
}

impl<T: Clone + Send + Sync + PartialEq + 'static> PartialEq for Array<T> {
    fn eq(&self, other: &Self) -> bool {
        self.storage.ptr_eq(&other.storage) || self.storage.as_slice() == other.storage.as_slice()
    }
}

impl<T: Clone + Send + Sync + Eq + 'static> Eq for Array<T> {}

impl<T: Clone + Send + Sync + Hash + 'static> Hash for Array<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.storage.as_slice().hash(state);
    }
}

// =============================================================================
// Arithmetic operators for Array + Array
// =============================================================================

/// Element-wise addition of two arrays.
///
/// # Panics
///
/// Panics if arrays have different sizes (unless one is empty).
impl<T> Add for Array<T>
where
    T: Clone + Send + Sync + Add<Output = T> + Default + 'static,
{
    type Output = Array<T>;

    fn add(self, rhs: Array<T>) -> Self::Output {
        let lhs_len = self.len();
        let rhs_len = rhs.len();

        // Handle empty arrays - promote to zero
        if lhs_len == 0 {
            return rhs;
        }
        if rhs_len == 0 {
            return self;
        }

        // Check conformity
        assert_eq!(lhs_len, rhs_len, "Non-conforming inputs for array addition");

        // Element-wise operation
        let result: Vec<T> = self
            .iter()
            .zip(rhs.iter())
            .map(|(a, b)| a.clone() + b.clone())
            .collect();

        Array::from(result)
    }
}

/// Element-wise subtraction of two arrays.
impl<T> Sub for Array<T>
where
    T: Clone + Send + Sync + Sub<Output = T> + Default + 'static,
{
    type Output = Array<T>;

    fn sub(self, rhs: Array<T>) -> Self::Output {
        let lhs_len = self.len();
        let rhs_len = rhs.len();

        if lhs_len == 0 {
            // 0 - rhs = -rhs
            let result: Vec<T> = rhs.iter().map(|x| T::default() - x.clone()).collect();
            return Array::from(result);
        }
        if rhs_len == 0 {
            return self;
        }

        assert_eq!(
            lhs_len, rhs_len,
            "Non-conforming inputs for array subtraction"
        );

        let result: Vec<T> = self
            .iter()
            .zip(rhs.iter())
            .map(|(a, b)| a.clone() - b.clone())
            .collect();

        Array::from(result)
    }
}

/// Element-wise multiplication of two arrays.
///
/// Matches C++ VTOPERATOR_CPPARRAY: promotes empty arrays to zero-filled
/// with the size of the other operand (T::default() * rhs[i] or lhs[i] * T::default()).
impl<T> Mul for Array<T>
where
    T: Clone + Send + Sync + Mul<Output = T> + Default + 'static,
{
    type Output = Array<T>;

    fn mul(self, rhs: Array<T>) -> Self::Output {
        let lhs_len = self.len();
        let rhs_len = rhs.len();

        if lhs_len != 0 && rhs_len != 0 && lhs_len != rhs_len {
            panic!("Non-conforming inputs for array multiplication");
        }

        let zero = T::default();
        let result: Vec<T> = if lhs_len == 0 && rhs_len == 0 {
            return Array::new();
        } else if lhs_len == 0 {
            rhs.iter().map(|r| zero.clone() * r.clone()).collect()
        } else if rhs_len == 0 {
            self.iter().map(|l| l.clone() * zero.clone()).collect()
        } else {
            self.iter()
                .zip(rhs.iter())
                .map(|(a, b)| a.clone() * b.clone())
                .collect()
        };

        Array::from(result)
    }
}

/// Element-wise division of two arrays.
impl<T> Div for Array<T>
where
    T: Clone + Send + Sync + Div<Output = T> + Default + 'static,
{
    type Output = Array<T>;

    fn div(self, rhs: Array<T>) -> Self::Output {
        let lhs_len = self.len();
        let rhs_len = rhs.len();

        if lhs_len == 0 {
            let result: Vec<T> = rhs.iter().map(|x| T::default() / x.clone()).collect();
            return Array::from(result);
        }
        if rhs_len == 0 {
            return self;
        }

        assert_eq!(lhs_len, rhs_len, "Non-conforming inputs for array division");

        let result: Vec<T> = self
            .iter()
            .zip(rhs.iter())
            .map(|(a, b)| a.clone() / b.clone())
            .collect();

        Array::from(result)
    }
}

/// Element-wise remainder of two arrays.
///
/// Matches C++ VTOPERATOR_CPPARRAY: promotes empty arrays to zero-filled.
impl<T> Rem for Array<T>
where
    T: Clone + Send + Sync + Rem<Output = T> + Default + 'static,
{
    type Output = Array<T>;

    fn rem(self, rhs: Array<T>) -> Self::Output {
        let lhs_len = self.len();
        let rhs_len = rhs.len();

        if lhs_len != 0 && rhs_len != 0 && lhs_len != rhs_len {
            panic!("Non-conforming inputs for array remainder");
        }

        let zero = T::default();
        let result: Vec<T> = if lhs_len == 0 && rhs_len == 0 {
            return Array::new();
        } else if lhs_len == 0 {
            rhs.iter().map(|r| zero.clone() % r.clone()).collect()
        } else if rhs_len == 0 {
            self.iter().map(|l| l.clone() % zero.clone()).collect()
        } else {
            self.iter()
                .zip(rhs.iter())
                .map(|(a, b)| a.clone() % b.clone())
                .collect()
        };

        Array::from(result)
    }
}

// =============================================================================
// Arithmetic operators for Array + scalar and scalar + Array
// =============================================================================

/// Multiply array by scalar (array * scalar).
impl<T> Mul<T> for Array<T>
where
    T: Clone + Send + Sync + Mul<Output = T> + 'static,
{
    type Output = Array<T>;

    fn mul(self, scalar: T) -> Self::Output {
        let result: Vec<T> = self.iter().map(|x| x.clone() * scalar.clone()).collect();
        Array::from(result)
    }
}

/// Multiply scalar by array (scalar * array).
impl Mul<Array<f64>> for f64 {
    type Output = Array<f64>;

    fn mul(self, array: Array<f64>) -> Self::Output {
        let result: Vec<f64> = array.iter().map(|x| self * x).collect();
        Array::from(result)
    }
}

impl Mul<Array<f32>> for f32 {
    type Output = Array<f32>;

    fn mul(self, array: Array<f32>) -> Self::Output {
        let result: Vec<f32> = array.iter().map(|x| self * x).collect();
        Array::from(result)
    }
}

impl Mul<Array<i32>> for i32 {
    type Output = Array<i32>;

    fn mul(self, array: Array<i32>) -> Self::Output {
        let result: Vec<i32> = array.iter().map(|x| self * x).collect();
        Array::from(result)
    }
}

impl Mul<Array<i64>> for i64 {
    type Output = Array<i64>;

    fn mul(self, array: Array<i64>) -> Self::Output {
        let result: Vec<i64> = array.iter().map(|x| self * x).collect();
        Array::from(result)
    }
}

/// Divide array by scalar (array / scalar).
impl<T> Div<T> for Array<T>
where
    T: Clone + Send + Sync + Div<Output = T> + 'static,
{
    type Output = Array<T>;

    fn div(self, scalar: T) -> Self::Output {
        let result: Vec<T> = self.iter().map(|x| x.clone() / scalar.clone()).collect();
        Array::from(result)
    }
}

/// Divide scalar by array (scalar / array).
impl Div<Array<f64>> for f64 {
    type Output = Array<f64>;

    fn div(self, array: Array<f64>) -> Self::Output {
        let result: Vec<f64> = array.iter().map(|x| self / x).collect();
        Array::from(result)
    }
}

impl Div<Array<f32>> for f32 {
    type Output = Array<f32>;

    fn div(self, array: Array<f32>) -> Self::Output {
        let result: Vec<f32> = array.iter().map(|x| self / x).collect();
        Array::from(result)
    }
}

/// Add scalar to array (array + scalar).
impl<T> Add<T> for Array<T>
where
    T: Clone + Send + Sync + Add<Output = T> + 'static,
{
    type Output = Array<T>;

    fn add(self, scalar: T) -> Self::Output {
        let result: Vec<T> = self.iter().map(|x| x.clone() + scalar.clone()).collect();
        Array::from(result)
    }
}

/// Subtract scalar from array (array - scalar).
impl<T> Sub<T> for Array<T>
where
    T: Clone + Send + Sync + Sub<Output = T> + 'static,
{
    type Output = Array<T>;

    fn sub(self, scalar: T) -> Self::Output {
        let result: Vec<T> = self.iter().map(|x| x.clone() - scalar.clone()).collect();
        Array::from(result)
    }
}

// =============================================================================
// Unary negation
// =============================================================================

/// Unary negation (element-wise).
impl<T> Neg for Array<T>
where
    T: Clone + Send + Sync + Neg<Output = T> + 'static,
{
    type Output = Array<T>;

    fn neg(self) -> Self::Output {
        let result: Vec<T> = self.iter().map(|x| -x.clone()).collect();
        Array::from(result)
    }
}

/// Unary negation for borrowed arrays.
impl<T> Neg for &Array<T>
where
    T: Clone + Send + Sync + Neg<Output = T> + 'static,
{
    type Output = Array<T>;

    fn neg(self) -> Self::Output {
        let result: Vec<T> = self.iter().map(|x| -x.clone()).collect();
        Array::from(result)
    }
}

// ============================================================================
// Bool array arithmetic specialization (C++ Vt_ArrayOpHelp<bool>)
// In C++: Add=OR, Sub=XOR, Mul=AND, Div=identity(lhs), Mod=false
// ============================================================================

/// Bool array addition = bitwise OR (C++ Vt_ArrayOpHelp<bool>::Add).
impl BitOr for Array<bool> {
    type Output = Array<bool>;

    fn bitor(self, rhs: Self) -> Self::Output {
        let (ll, rl) = (self.len(), rhs.len());
        if ll != 0 && rl != 0 && ll != rl {
            panic!("Non-conforming inputs for bool array OR");
        }
        let r: Vec<bool> = if ll == 0 {
            rhs.iter().copied().collect()
        } else if rl == 0 {
            self.iter().copied().collect()
        } else {
            self.iter().zip(rhs.iter()).map(|(&a, &b)| a | b).collect()
        };
        Array::from(r)
    }
}

/// Bool array subtraction = bitwise XOR (C++ Vt_ArrayOpHelp<bool>::Sub).
impl BitXor for Array<bool> {
    type Output = Array<bool>;

    fn bitxor(self, rhs: Self) -> Self::Output {
        let (ll, rl) = (self.len(), rhs.len());
        if ll != 0 && rl != 0 && ll != rl {
            panic!("Non-conforming inputs for bool array XOR");
        }
        let r: Vec<bool> = if ll == 0 {
            rhs.iter().copied().collect()
        } else if rl == 0 {
            self.iter().copied().collect()
        } else {
            self.iter().zip(rhs.iter()).map(|(&a, &b)| a ^ b).collect()
        };
        Array::from(r)
    }
}

/// Bool array multiplication = bitwise AND (C++ Vt_ArrayOpHelp<bool>::Mul).
impl BitAnd for Array<bool> {
    type Output = Array<bool>;

    fn bitand(self, rhs: Self) -> Self::Output {
        let (ll, rl) = (self.len(), rhs.len());
        if ll != 0 && rl != 0 && ll != rl {
            panic!("Non-conforming inputs for bool array AND");
        }
        let r: Vec<bool> = if ll == 0 {
            vec![false; rl]
        } else if rl == 0 {
            vec![false; ll]
        } else {
            self.iter().zip(rhs.iter()).map(|(&a, &b)| a & b).collect()
        };
        Array::from(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let arr: Array<i32> = Array::new();
        assert!(arr.is_empty());
        assert_eq!(arr.len(), 0);
    }

    #[test]
    fn test_from_vec() {
        let arr: Array<i32> = Array::from(vec![1, 2, 3, 4, 5]);
        assert_eq!(arr.len(), 5);
        assert_eq!(arr[0], 1);
        assert_eq!(arr[4], 5);
    }

    #[test]
    fn test_from_elem() {
        let arr: Array<f32> = Array::from_elem(3.14, 3);
        assert_eq!(arr.len(), 3);
        assert!(arr.iter().all(|&x| (x - 3.14).abs() < 0.001));
    }

    #[test]
    fn test_clone_shares_data() {
        let arr1: Array<i32> = Array::from(vec![1, 2, 3]);
        let arr2 = arr1.clone();
        assert!(arr1.is_shared_with(&arr2));
    }

    #[test]
    fn test_cow_on_mutation() {
        let arr1: Array<i32> = Array::from(vec![1, 2, 3]);
        let mut arr2 = arr1.clone();

        // Should share before mutation
        assert!(arr1.is_shared_with(&arr2));

        // Mutation causes copy
        arr2.push(4);

        // No longer shared
        assert!(!arr1.is_shared_with(&arr2));
        assert_eq!(arr1.len(), 3);
        assert_eq!(arr2.len(), 4);
    }

    #[test]
    fn test_push_pop() {
        let mut arr: Array<i32> = Array::new();
        arr.push(1);
        arr.push(2);
        arr.push(3);

        assert_eq!(arr.pop(), Some(3));
        assert_eq!(arr.pop(), Some(2));
        assert_eq!(arr.pop(), Some(1));
        assert_eq!(arr.pop(), None);
    }

    #[test]
    fn test_resize() {
        let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
        arr.resize(5, 0);
        assert_eq!(arr.as_slice(), &[1, 2, 3, 0, 0]);

        arr.resize(2, 0);
        assert_eq!(arr.as_slice(), &[1, 2]);
    }

    #[test]
    fn test_clear() {
        let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
        arr.clear();
        assert!(arr.is_empty());
    }

    #[test]
    fn test_equality() {
        let arr1: Array<i32> = Array::from(vec![1, 2, 3]);
        let arr2: Array<i32> = Array::from(vec![1, 2, 3]);
        let arr3: Array<i32> = Array::from(vec![1, 2, 4]);

        assert_eq!(arr1, arr2);
        assert_ne!(arr1, arr3);
    }

    #[test]
    fn test_index_mut() {
        let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
        arr[1] = 20;
        assert_eq!(arr[1], 20);
    }

    #[test]
    fn test_into_vec() {
        let arr: Array<i32> = Array::from(vec![1, 2, 3]);
        let v = arr.into_vec();
        assert_eq!(v, vec![1, 2, 3]);
    }

    #[test]
    fn test_iter() {
        let arr: Array<i32> = Array::from(vec![1, 2, 3, 4, 5]);
        let sum: i32 = arr.iter().sum();
        assert_eq!(sum, 15);
    }

    #[test]
    fn test_from_iterator() {
        let arr: Array<i32> = (0..5).collect();
        assert_eq!(arr.as_slice(), &[0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_make_unique_shared() {
        let arr1: Array<i32> = Array::from(vec![1, 2, 3]);
        let mut arr2 = arr1.clone();

        // Shared - make_unique should copy and return true
        assert!(arr2.make_unique());
        assert!(arr2.is_unique());
        assert!(!arr1.is_shared_with(&arr2));

        // Already unique - should return false
        assert!(!arr2.make_unique());
    }

    #[test]
    fn test_make_unique_already_unique() {
        let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
        assert!(!arr.make_unique()); // Already unique, no copy needed
    }

    #[test]
    fn test_as_const() {
        let arr: Array<i32> = Array::from(vec![1, 2, 3]);
        let c = arr.as_const();
        assert_eq!(c.len(), 3);
        assert_eq!(c[0], 1);
        // Verify it's the same reference
        assert!(std::ptr::eq(&arr, c));
    }

    #[test]
    fn test_is_identical() {
        let arr1: Array<i32> = Array::from(vec![1, 2, 3]);
        let arr2 = arr1.clone(); // Shared storage
        let arr3: Array<i32> = Array::from(vec![1, 2, 3]); // Same content, different alloc

        assert!(arr1.is_identical(&arr2));
        assert!(!arr1.is_identical(&arr3));

        // Equal but not identical
        assert_eq!(arr1, arr3);
    }

    #[test]
    fn test_is_identical_after_mutation() {
        let arr1: Array<i32> = Array::from(vec![1, 2, 3]);
        let mut arr2 = arr1.clone();
        assert!(arr1.is_identical(&arr2));

        arr2.push(4); // Triggers COW detach
        assert!(!arr1.is_identical(&arr2));
    }

    // =====================================================================
    // H-vt-3: ShapeData integration tests
    // =====================================================================

    #[test]
    fn test_shape_data_new() {
        let arr: Array<i32> = Array::new();
        assert_eq!(arr.shape_data().total_size, 0);
        assert!(arr.shape_data().is_1d());
        assert_eq!(arr.rank(), 1);
    }

    #[test]
    fn test_shape_data_from_vec() {
        let arr: Array<i32> = Array::from(vec![1, 2, 3]);
        assert_eq!(arr.shape_data().total_size, 3);
        assert!(arr.shape_data().is_1d());
    }

    #[test]
    fn test_shape_data_push_pop() {
        let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
        arr.push(4);
        assert_eq!(arr.shape_data().total_size, 4);

        arr.pop();
        assert_eq!(arr.shape_data().total_size, 3);
    }

    #[test]
    fn test_shape_data_clear() {
        let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
        arr.clear();
        assert_eq!(arr.shape_data().total_size, 0);
    }

    #[test]
    fn test_shape_data_resize() {
        let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
        arr.resize(6, 0);
        assert_eq!(arr.shape_data().total_size, 6);
    }

    #[test]
    fn test_set_shape_valid() {
        let mut arr: Array<i32> = Array::from(vec![1, 2, 3, 4, 5, 6]);
        assert!(arr.set_shape(ShapeData::new_2d(2, 3)));
        assert_eq!(arr.rank(), 2);
        assert_eq!(arr.shape_data().total_size, 6);
    }

    #[test]
    fn test_set_shape_invalid_size() {
        let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
        assert!(!arr.set_shape(ShapeData::new_2d(2, 3))); // needs 6, have 3
        assert_eq!(arr.rank(), 1); // unchanged
    }

    #[test]
    fn test_shape_data_assign() {
        let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
        arr.assign(vec![10, 20].into_iter());
        assert_eq!(arr.shape_data().total_size, 2);
    }

    #[test]
    fn test_shape_data_from_iterator() {
        let arr: Array<i32> = (0..5).collect();
        assert_eq!(arr.shape_data().total_size, 5);
    }

    #[test]
    fn test_shape_data_insert_remove() {
        let mut arr: Array<i32> = Array::from(vec![1, 2, 3]);
        arr.insert(1, 10);
        assert_eq!(arr.shape_data().total_size, 4);

        arr.remove(0);
        assert_eq!(arr.shape_data().total_size, 3);
    }

    #[test]
    fn test_shape_data_swap() {
        let mut a: Array<i32> = Array::from(vec![1, 2, 3]);
        let mut b: Array<i32> = Array::from(vec![4, 5]);
        a.swap(&mut b);
        assert_eq!(a.shape_data().total_size, 2);
        assert_eq!(b.shape_data().total_size, 3);
    }

    // =====================================================================
    // ForeignDataSource integration tests
    // =====================================================================

    #[allow(unsafe_code)]
    #[test]
    fn test_foreign_reads_without_copy() {
        let data = vec![1i32, 2, 3, 4, 5];
        let source = ForeignDataSource::new(None);
        // SAFETY: data outlives arr in this test.
        let arr = unsafe { Array::from_foreign(source, data.as_ptr(), data.len()) };
        assert!(arr.is_foreign());
        assert_eq!(arr.as_slice(), &[1, 2, 3, 4, 5]);
        assert_eq!(arr.len(), 5);
        // Pointer is the same (zero-copy).
        assert_eq!(arr.as_ptr(), data.as_ptr());
    }

    #[allow(unsafe_code)]
    #[test]
    fn test_foreign_clone_shares_foreign() {
        let data = vec![10i32, 20, 30];
        let source = ForeignDataSource::new(None);
        let arr = unsafe { Array::from_foreign(source, data.as_ptr(), data.len()) };
        let arr2 = arr.clone();
        // Both point at the same foreign memory.
        assert!(arr.is_foreign());
        assert!(arr2.is_foreign());
        assert_eq!(arr.as_ptr(), arr2.as_ptr());
    }

    #[allow(unsafe_code)]
    #[test]
    fn test_foreign_mutation_detaches() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let data = vec![1i32, 2, 3];
        let detached = std::sync::Arc::new(AtomicBool::new(false));
        let d = detached.clone();
        let source = ForeignDataSource::new(Some(Box::new(move || {
            d.store(true, Ordering::SeqCst);
        })));
        let arr = unsafe { Array::from_foreign(source, data.as_ptr(), data.len()) };
        assert!(arr.is_foreign());

        // Mutation triggers detach (copy to owned).
        let mut arr2 = arr.clone();
        assert!(!detached.load(Ordering::SeqCst)); // not yet: arr still alive
        arr2.push(4);
        // arr2 is now owned, arr still foreign.
        assert!(!arr2.is_foreign());
        assert!(arr.is_foreign());
        // Callback fires only when last foreign ref is dropped.
        assert!(!detached.load(Ordering::SeqCst));
        drop(arr);
        assert!(detached.load(Ordering::SeqCst)); // now fired
        assert_eq!(arr2.as_slice(), &[1, 2, 3, 4]);
    }

    #[allow(unsafe_code)]
    #[test]
    fn test_foreign_detach_fires_on_both_dropped() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let data = vec![42i32; 5];
        let count = std::sync::Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        let source = ForeignDataSource::new(Some(Box::new(move || {
            c.fetch_add(1, Ordering::SeqCst);
        })));
        let arr1 = unsafe { Array::from_foreign(source, data.as_ptr(), data.len()) };
        let arr2 = arr1.clone();
        drop(arr1); // refcount -> 1, no callback yet
        assert_eq!(count.load(Ordering::SeqCst), 0);
        drop(arr2); // refcount -> 0, callback fires once
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[allow(unsafe_code)]
    #[test]
    fn test_foreign_is_not_unique() {
        let data = vec![1i32];
        let source = ForeignDataSource::new(None);
        let arr = unsafe { Array::from_foreign(source, data.as_ptr(), data.len()) };
        // A single foreign array IS unique (only one Arc<ForeignSlice>).
        assert!(arr.is_unique());
        // After cloning it is shared.
        let arr2 = arr.clone();
        assert!(!arr.is_unique());
        assert!(!arr2.is_unique());
    }
}
