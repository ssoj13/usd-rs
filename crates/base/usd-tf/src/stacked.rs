//! Thread-local stacked objects.
//!
//! This module provides utilities for maintaining stacks of objects
//! that automatically push themselves when created and pop when dropped.
//!
//! This is useful for tracking context, such as the current operation
//! being performed or the current scope.
//!
//! # Examples
//!
//! Use [`StackedBuilder`] for simple cases:
//!
//! ```
//! use usd_tf::stacked::StackedBuilder;
//!
//! // Create stacked values - they're automatically tracked
//! let ctx1 = StackedBuilder::new("outer");
//! assert_eq!(StackedBuilder::<&str>::top(), Some(&"outer"));
//!
//! {
//!     let ctx2 = StackedBuilder::new("inner");
//!     assert_eq!(StackedBuilder::<&str>::top(), Some(&"inner"));
//! }
//!
//! assert_eq!(StackedBuilder::<&str>::top(), Some(&"outer"));
//! drop(ctx1);
//! assert!(StackedBuilder::<&str>::top().is_none());
//! ```

use std::cell::RefCell;
use std::marker::PhantomData;

thread_local! {
    /// Generic thread-local storage for stacks.
    /// We use type_id to distinguish different stack types.
    static STACKS: RefCell<std::collections::HashMap<std::any::TypeId, Vec<*const ()>>> =
        RefCell::new(std::collections::HashMap::new());
}

/// A marker type that automatically manages stack registration.
///
/// When a struct contains a `Stacked<Self>` field and implements
/// `StackedAccess`, it will automatically be pushed onto the thread-local
/// stack when created and popped when dropped.
///
/// # Safety
///
/// Objects containing `Stacked` must not be moved after creation,
/// as the stack stores raw pointers. The recommended pattern is to
/// create the object and immediately use it within the same scope.
pub struct Stacked<T: 'static> {
    /// Storage for the pointer.
    storage: StackedStorage<T>,
    _marker: PhantomData<T>,
}

/// Internal storage for Stacked pointer.
enum StackedStorage<T: 'static> {
    /// Not yet initialized.
    Uninit,
    /// Initialized with pointer.
    Init(*const T),
}

impl<T: 'static> Stacked<T> {
    /// Create a new Stacked marker.
    ///
    /// The containing object must call `push` after construction.
    #[inline]
    pub fn new() -> Self {
        Self {
            storage: StackedStorage::Uninit,
            _marker: PhantomData,
        }
    }

    /// Push the containing object onto the stack.
    ///
    /// This is automatically called when using `StackedAccess`.
    ///
    /// # Safety
    ///
    /// The pointer must remain valid until `pop` is called.
    pub fn push(&mut self, ptr: *const T) {
        self.storage = StackedStorage::Init(ptr);
        STACKS.with(|stacks| {
            let mut stacks = stacks.borrow_mut();
            let type_id = std::any::TypeId::of::<T>();
            stacks.entry(type_id).or_default().push(ptr as *const ());
        });
    }

    /// Get the stored pointer.
    #[inline]
    fn get_ptr(&self) -> *const T {
        match self.storage {
            StackedStorage::Init(ptr) => ptr,
            StackedStorage::Uninit => std::ptr::null(),
        }
    }

    /// Pop the object from the stack.
    ///
    /// Called automatically on drop.
    fn pop(&mut self) {
        let ptr = self.get_ptr();
        if ptr.is_null() {
            return;
        }
        STACKS.with(|stacks| {
            let mut stacks = stacks.borrow_mut();
            let type_id = std::any::TypeId::of::<T>();
            if let Some(stack) = stacks.get_mut(&type_id) {
                if let Some(top) = stack.last() {
                    if *top == ptr as *const () {
                        stack.pop();
                    } else {
                        // Out-of-order pop: C++ TF_FATAL_ERROR equivalent — warns on stack corruption.
                        eprintln!(
                            "TfStacked: out-of-order pop detected - expected item at top of stack"
                        );
                        if let Some(pos) = stack.iter().rposition(|&p| p == ptr as *const ()) {
                            stack.remove(pos);
                        }
                    }
                }
            }
        });
    }
}

impl<T: 'static> Default for Stacked<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static> std::fmt::Debug for Stacked<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stacked")
            .field("ptr", &self.get_ptr())
            .finish()
    }
}

impl<T: 'static> Drop for Stacked<T> {
    fn drop(&mut self) {
        self.pop();
    }
}

// Safety: Stacked is not Send/Sync because it uses thread-local storage
// and stores raw pointers that are only valid in the creating thread.

/// Trait for types that can be stacked.
///
/// Implement this trait to enable automatic stack management.
pub trait StackedAccess: Sized + 'static {
    /// Get a reference to the Stacked marker.
    fn as_stacked(&mut self) -> &mut Stacked<Self>;

    /// Push self onto the thread-local stack.
    ///
    /// Call this immediately after construction.
    fn push_stack(&mut self) {
        let ptr = self as *const Self;
        self.as_stacked().push(ptr);
    }

    /// Get the top of the stack.
    fn stack_top() -> Option<&'static Self> {
        STACKS.with(|stacks| {
            let stacks = stacks.borrow();
            let type_id = std::any::TypeId::of::<Self>();
            stacks
                .get(&type_id)
                .and_then(|stack| stack.last())
                .map(|&ptr| {
                    // SAFETY: Pointers in stack are valid for the lifetime of the stack
                    #[allow(unsafe_code)]
                    unsafe {
                        &*(ptr as *const Self)
                    }
                })
        })
    }

    /// Get the element below the top of the stack.
    fn stack_prev() -> Option<&'static Self> {
        STACKS.with(|stacks| {
            let stacks = stacks.borrow();
            let type_id = std::any::TypeId::of::<Self>();
            stacks.get(&type_id).and_then(|stack| {
                if stack.len() >= 2 {
                    // SAFETY: Pointers in stack are valid for the lifetime of the stack
                    #[allow(unsafe_code)]
                    Some(unsafe { &*(stack[stack.len() - 2] as *const Self) })
                } else {
                    None
                }
            })
        })
    }

    /// Get the current stack depth.
    fn stack_depth() -> usize {
        STACKS.with(|stacks| {
            let stacks = stacks.borrow();
            let type_id = std::any::TypeId::of::<Self>();
            stacks.get(&type_id).map_or(0, |stack| stack.len())
        })
    }

    /// Check if this object is at the top of the stack.
    fn is_stack_top(&self) -> bool {
        STACKS.with(|stacks| {
            let stacks = stacks.borrow();
            let type_id = std::any::TypeId::of::<Self>();
            stacks
                .get(&type_id)
                .and_then(|stack| stack.last())
                .is_some_and(|&top| top == (self as *const Self) as *const ())
        })
    }

    /// Check if the stack is empty.
    fn stack_is_empty() -> bool {
        Self::stack_depth() == 0
    }
}

/// A simpler stacked type using Box for stable addresses.
///
/// This type boxes its value to ensure the pointer remains valid
/// even after the StackedBuilder is moved.
///
/// # Examples
///
/// ```
/// use usd_tf::stacked::StackedBuilder;
///
/// let ctx1 = StackedBuilder::new("context 1");
/// assert_eq!(StackedBuilder::<&str>::top(), Some(&"context 1"));
///
/// {
///     let ctx2 = StackedBuilder::new("context 2");
///     assert_eq!(StackedBuilder::<&str>::top(), Some(&"context 2"));
/// }
///
/// assert_eq!(StackedBuilder::<&str>::top(), Some(&"context 1"));
/// ```
pub struct StackedBuilder<T: 'static> {
    /// Boxed to ensure stable address.
    inner: Box<StackedBuilderInner<T>>,
}

/// Inner struct that holds the actual value and stacked marker.
struct StackedBuilderInner<T: 'static> {
    value: T,
    _stacked: Stacked<Self>,
}

impl<T: 'static> StackedBuilder<T> {
    /// Create a new stacked value.
    pub fn new(value: T) -> Self {
        let mut inner = Box::new(StackedBuilderInner {
            value,
            _stacked: Stacked::new(),
        });
        // Now that the inner is boxed, we can safely take a pointer
        let ptr = &*inner as *const StackedBuilderInner<T>;
        inner._stacked.push(ptr);
        Self { inner }
    }

    /// Get the value at the top of the stack.
    pub fn top() -> Option<&'static T> {
        STACKS.with(|stacks| {
            let stacks = stacks.borrow();
            let type_id = std::any::TypeId::of::<StackedBuilderInner<T>>();
            stacks
                .get(&type_id)
                .and_then(|stack| stack.last())
                .map(|&ptr| {
                    // SAFETY: Pointers in stack are valid for the lifetime of the stack
                    #[allow(unsafe_code)]
                    unsafe {
                        &(*(ptr as *const StackedBuilderInner<T>)).value
                    }
                })
        })
    }

    /// Get a reference to the contained value.
    pub fn value(&self) -> &T {
        &self.inner.value
    }

    /// Get a mutable reference to the contained value.
    pub fn value_mut(&mut self) -> &mut T {
        &mut self.inner.value
    }

    /// Get the stack depth.
    pub fn depth() -> usize {
        STACKS.with(|stacks| {
            let stacks = stacks.borrow();
            let type_id = std::any::TypeId::of::<StackedBuilderInner<T>>();
            stacks.get(&type_id).map_or(0, |stack| stack.len())
        })
    }
}

impl<T: 'static> std::ops::Deref for StackedBuilder<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner.value
    }
}

impl<T: 'static> std::ops::DerefMut for StackedBuilder<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: StackedAccess with raw Stacked requires careful handling of pointer lifetimes.
    // For types that need to be moved, use Box to ensure stable addresses.

    #[test]
    fn test_basic_stack_with_builder() {
        // Using StackedBuilder which handles pointer stability internally
        assert_eq!(StackedBuilder::<String>::depth(), 0);

        {
            let ctx1 = StackedBuilder::new("first".to_string());
            assert_eq!(StackedBuilder::<String>::depth(), 1);
            assert_eq!(
                StackedBuilder::<String>::top().map(|s| s.as_str()),
                Some("first")
            );

            {
                let ctx2 = StackedBuilder::new("second".to_string());
                assert_eq!(StackedBuilder::<String>::depth(), 2);
                assert_eq!(
                    StackedBuilder::<String>::top().map(|s| s.as_str()),
                    Some("second")
                );
                drop(ctx2);
            }

            assert_eq!(StackedBuilder::<String>::depth(), 1);
            assert_eq!(
                StackedBuilder::<String>::top().map(|s| s.as_str()),
                Some("first")
            );
            drop(ctx1);
        }

        assert_eq!(StackedBuilder::<String>::depth(), 0);
    }

    #[test]
    fn test_stacked_builder() {
        assert_eq!(StackedBuilder::<i32>::depth(), 0);

        {
            let _s1 = StackedBuilder::new(42);
            assert_eq!(StackedBuilder::<i32>::depth(), 1);
            assert_eq!(StackedBuilder::<i32>::top(), Some(&42));

            {
                let _s2 = StackedBuilder::new(100);
                assert_eq!(StackedBuilder::<i32>::depth(), 2);
                assert_eq!(StackedBuilder::<i32>::top(), Some(&100));
            }

            assert_eq!(StackedBuilder::<i32>::depth(), 1);
            assert_eq!(StackedBuilder::<i32>::top(), Some(&42));
        }

        assert_eq!(StackedBuilder::<i32>::depth(), 0);
        assert_eq!(StackedBuilder::<i32>::top(), None);
    }

    #[test]
    fn test_stacked_builder_deref() {
        let s = StackedBuilder::new(String::from("hello"));
        assert_eq!(s.len(), 5);
        assert_eq!(*s, "hello");
    }

    #[test]
    fn test_stacked_builder_mut() {
        let mut s = StackedBuilder::new(vec![1, 2, 3]);
        s.push(4);
        assert_eq!(*s, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_different_types_separate_stacks() {
        let _s1 = StackedBuilder::new(42i32);
        let _s2 = StackedBuilder::new("hello");
        let _s3 = StackedBuilder::new(3.14f64);

        assert_eq!(StackedBuilder::<i32>::depth(), 1);
        assert_eq!(StackedBuilder::<&str>::depth(), 1);
        assert_eq!(StackedBuilder::<f64>::depth(), 1);

        assert_eq!(StackedBuilder::<i32>::top(), Some(&42));
        assert_eq!(StackedBuilder::<&str>::top(), Some(&"hello"));
        assert_eq!(StackedBuilder::<f64>::top(), Some(&3.14));
    }

    #[test]
    fn test_empty_stack_with_builder() {
        // Test empty stack behavior
        assert_eq!(StackedBuilder::<u8>::depth(), 0);
        assert!(StackedBuilder::<u8>::top().is_none());
    }

    #[test]
    fn test_single_item_no_prev() {
        let _ctx = StackedBuilder::new("only".to_string());
        // For StackedBuilder, we don't have stack_prev, but we can verify depth
        assert_eq!(StackedBuilder::<String>::depth(), 1);
    }
}
