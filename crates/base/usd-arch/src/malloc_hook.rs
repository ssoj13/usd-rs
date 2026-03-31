// Copyright 2025 Joss Whittle
//

// SAFETY: This module provides FFI bindings to system APIs requiring unsafe
#![allow(unsafe_code)]

//! Malloc hook abstractions and custom allocator support.
//!
//! This module provides a framework for instrumenting memory allocations in Rust.
//!
//! # Important Limitations
//!
//! Unlike the C++ version which can hook into existing allocators at runtime via
//! `__malloc_hook` and similar mechanisms (available in glibc < 2.34, ptmalloc3,
//! jemalloc with pxmalloc wrapper), Rust's global allocator is determined at
//! compile time via `#[global_allocator]` and cannot be dynamically replaced.
//!
//! ## Rust vs C++ Approach
//!
//! **C++ (Original USD):**
//! - Runtime detection of allocator (ptmalloc3, jemalloc, pxmalloc)
//! - Dynamic hooking via `dlsym()` and `__malloc_hook` variables
//! - Can intercept all allocations including those from STL containers
//! - Works with existing binaries via `LD_PRELOAD`
//!
//! **Rust (This Implementation):**
//! - Compile-time allocator selection via `#[global_allocator]`
//! - Hooks via custom `GlobalAlloc` trait implementation
//! - Requires recompilation to enable instrumentation
//! - Thread-local storage or atomic operations for hook state
//!
//! # Design
//!
//! This module provides:
//! 1. [`MallocHook`] trait for instrumentation callbacks
//! 2. Global hook registry with thread-safe access
//! 3. Optional [`InstrumentedAllocator`] wrapper for `#[global_allocator]`
//!
//! # Usage
//!
//! ## Basic Hook Registration
//!
//! ```rust
//! use usd_arch::malloc_hook::{MallocHook, set_malloc_hook};
//!
//! struct MyHook;
//!
//! impl MallocHook for MyHook {
//!     fn on_alloc(&self, ptr: *mut u8, size: usize) {
//!         println!("Allocated {} bytes at {:p}", size, ptr);
//!     }
//!
//!     fn on_dealloc(&self, ptr: *mut u8) {
//!         println!("Freed {:p}", ptr);
//!     }
//!
//!     fn on_realloc(&self, old_ptr: *mut u8, new_ptr: *mut u8, size: usize) {
//!         println!("Reallocated {:p} -> {:p} ({} bytes)", old_ptr, new_ptr, size);
//!     }
//! }
//!
//! // Register hook (requires InstrumentedAllocator as global allocator)
//! set_malloc_hook(Some(Box::new(MyHook)));
//! ```
//!
//! ## Custom Allocator with Instrumentation
//!
//! ```rust
//! use std::alloc::{GlobalAlloc, System};
//! use usd_arch::malloc_hook::InstrumentedAllocator;
//!
//! #[global_allocator]
//! static GLOBAL: InstrumentedAllocator<System> = InstrumentedAllocator::new(System);
//! ```
//!
//! # Performance Considerations
//!
//! - Hook calls add overhead to every allocation/deallocation
//! - Use atomic operations for hook pointer access (lock-free)
//! - Consider compile-time feature flags to disable in release builds
//! - Avoid allocations within hook implementations (can cause recursion)
//!
//! # Safety
//!
//! Hook implementations must be extremely careful:
//! - Must be async-signal-safe (no locks, no allocations)
//! - Must be thread-safe
//! - Must not panic or unwind
//! - Must not call back into the allocator (infinite recursion)

use std::alloc::{GlobalAlloc, Layout};
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

/// Trait for malloc hook callbacks.
///
/// Implement this trait to instrument memory allocations. Hooks are called
/// on every allocation, deallocation, and reallocation when using
/// [`InstrumentedAllocator`].
///
/// # Safety Requirements
///
/// Hook implementations **must**:
/// - Be async-signal-safe (no locks, no syscalls unless async-signal-safe)
/// - Never allocate memory (would cause infinite recursion)
/// - Never panic or unwind
/// - Be thread-safe (can be called from multiple threads concurrently)
/// - Execute quickly (called on hot path)
///
/// # Example
///
/// ```rust
/// use std::sync::atomic::{AtomicUsize, Ordering};
/// use usd_arch::malloc_hook::MallocHook;
///
/// struct AllocationCounter {
///     count: AtomicUsize,
///     total_bytes: AtomicUsize,
/// }
///
/// impl MallocHook for AllocationCounter {
///     fn on_alloc(&self, _ptr: *mut u8, size: usize) {
///         self.count.fetch_add(1, Ordering::Relaxed);
///         self.total_bytes.fetch_add(size, Ordering::Relaxed);
///     }
///
///     fn on_dealloc(&self, _ptr: *mut u8) {
///         // Note: We don't track size on dealloc in this simple example
///     }
///
///     fn on_realloc(&self, _old_ptr: *mut u8, _new_ptr: *mut u8, size: usize) {
///         self.total_bytes.fetch_add(size, Ordering::Relaxed);
///     }
/// }
/// ```
pub trait MallocHook: Send + Sync {
    /// Called after successful memory allocation.
    ///
    /// # Parameters
    /// - `ptr`: Pointer to the newly allocated memory (never null)
    /// - `size`: Size of the allocation in bytes
    ///
    /// # Safety
    /// The pointer is valid but the memory contents are uninitialized.
    /// Do not dereference or modify the memory.
    fn on_alloc(&self, ptr: *mut u8, size: usize);

    /// Called before memory deallocation.
    ///
    /// # Parameters
    /// - `ptr`: Pointer to the memory being freed (may be null)
    ///
    /// # Safety
    /// The pointer is still valid when this is called, but will be invalidated
    /// immediately after. Do not access the memory contents.
    fn on_dealloc(&self, ptr: *mut u8);

    /// Called after successful memory reallocation.
    ///
    /// # Parameters
    /// - `old_ptr`: Original pointer (may be null if realloc acts as malloc)
    /// - `new_ptr`: New pointer after reallocation (never null on success)
    /// - `size`: New size in bytes
    ///
    /// # Safety
    /// The old pointer is invalid after this call. The new pointer contains
    /// the copied data (up to min of old and new sizes).
    fn on_realloc(&self, old_ptr: *mut u8, new_ptr: *mut u8, size: usize);

    /// Called on memalign/aligned allocation.
    ///
    /// # Parameters
    /// - `ptr`: Pointer to the newly allocated aligned memory (never null)
    /// - `alignment`: Alignment requirement in bytes (power of 2)
    /// - `size`: Size of the allocation in bytes
    ///
    /// Default implementation calls `on_alloc()`.
    fn on_alloc_aligned(&self, ptr: *mut u8, alignment: usize, size: usize) {
        let _ = alignment; // suppress unused warning
        self.on_alloc(ptr, size);
    }
}

// Global hook storage using atomic pointer
static MALLOC_HOOK: AtomicPtr<Box<dyn MallocHook>> = AtomicPtr::new(ptr::null_mut());

/// Set the global malloc hook.
///
/// Replaces any existing hook. Pass `None` to disable hooking.
///
/// # Thread Safety
///
/// This function is thread-safe but not atomic with respect to concurrent
/// allocations. There may be a brief window where some allocations are
/// not instrumented during hook replacement.
///
/// # Example
///
/// ```rust
/// use usd_arch::malloc_hook::{MallocHook, set_malloc_hook};
///
/// struct MyHook;
/// impl MallocHook for MyHook {
///     fn on_alloc(&self, _ptr: *mut u8, _size: usize) {}
///     fn on_dealloc(&self, _ptr: *mut u8) {}
///     fn on_realloc(&self, _old: *mut u8, _new: *mut u8, _size: usize) {}
/// }
///
/// // Enable hook
/// set_malloc_hook(Some(Box::new(MyHook)));
///
/// // Disable hook
/// set_malloc_hook(None);
/// ```
pub fn set_malloc_hook(hook: Option<Box<dyn MallocHook>>) {
    let new_ptr = match hook {
        Some(h) => Box::into_raw(Box::new(h)),
        None => ptr::null_mut(),
    };

    // Swap the hook pointer atomically
    let old_ptr = MALLOC_HOOK.swap(new_ptr, Ordering::AcqRel);

    // Clean up old hook if it existed
    if !old_ptr.is_null() {
        unsafe {
            // SAFETY: old_ptr came from Box::into_raw
            let _ = Box::from_raw(old_ptr);
        }
    }
}

/// Get a reference to the current malloc hook, if any.
///
/// Returns `None` if no hook is currently installed.
///
/// # Safety
///
/// This function returns a raw pointer to avoid lifetime issues and lock
/// overhead. The pointer is only valid until the next call to
/// [`set_malloc_hook`]. Callers must not hold this pointer across any
/// operation that might change the hook.
///
/// # Example
///
/// ```ignore
/// use usd_arch::get_malloc_hook;
///
/// if let Some(hook) = unsafe { get_malloc_hook() } {
///     // Use the hook
/// }
/// ```
pub unsafe fn get_malloc_hook() -> Option<&'static dyn MallocHook> {
    unsafe {
        let ptr = MALLOC_HOOK.load(Ordering::Acquire);
        if ptr.is_null() {
            None
        } else {
            // SAFETY: Pointer came from Box::into_raw and is valid until next set_malloc_hook()
            // Caller must ensure they don't hold reference across hook changes
            Some(&*(*ptr))
        }
    }
}

/// Check if malloc hooking is currently enabled.
///
/// Returns `true` if a hook is installed, `false` otherwise.
///
/// # Example
///
/// ```rust
/// use usd_arch::malloc_hook::{is_malloc_hook_enabled, set_malloc_hook};
///
/// assert!(!is_malloc_hook_enabled());
/// // set_malloc_hook(Some(Box::new(MyHook)));
/// // assert!(is_malloc_hook_enabled());
/// ```
#[inline]
pub fn is_malloc_hook_enabled() -> bool {
    !MALLOC_HOOK.load(Ordering::Acquire).is_null()
}

/// Instrumented allocator wrapper.
///
/// Wraps any `GlobalAlloc` implementation and calls the registered
/// [`MallocHook`] on allocation/deallocation operations.
///
/// # Usage
///
/// ```rust
/// use std::alloc::System;
/// use usd_arch::malloc_hook::InstrumentedAllocator;
///
/// #[global_allocator]
/// static GLOBAL: InstrumentedAllocator<System> = InstrumentedAllocator::new(System);
/// ```
///
/// # Performance
///
/// Adds overhead on every allocation/deallocation:
/// - One atomic load to check if hook is enabled
/// - One function call if hook is present
///
/// Consider using feature flags to conditionally compile this in debug/profile builds only.
///
/// # Type Parameters
///
/// - `A`: The underlying allocator (must implement `GlobalAlloc`)
pub struct InstrumentedAllocator<A: GlobalAlloc> {
    inner: A,
}

impl<A: GlobalAlloc> InstrumentedAllocator<A> {
    /// Create a new instrumented allocator wrapping `inner`.
    ///
    /// This is a const fn so it can be used in static initialization.
    pub const fn new(inner: A) -> Self {
        Self { inner }
    }

    /// Get a reference to the inner allocator.
    pub const fn inner(&self) -> &A {
        &self.inner
    }
}

unsafe impl<A: GlobalAlloc> GlobalAlloc for InstrumentedAllocator<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe {
            let ptr = self.inner.alloc(layout);

            if !ptr.is_null() {
                // SAFETY: get_malloc_hook() contract - don't hold across hook changes
                if let Some(hook) = get_malloc_hook() {
                    hook.on_alloc(ptr, layout.size());
                }
            }

            ptr
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe {
            // SAFETY: get_malloc_hook() contract
            if let Some(hook) = get_malloc_hook() {
                hook.on_dealloc(ptr);
            }

            self.inner.dealloc(ptr, layout);
        }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe {
            let ptr = self.inner.alloc_zeroed(layout);

            if !ptr.is_null() {
                // SAFETY: get_malloc_hook() contract
                if let Some(hook) = get_malloc_hook() {
                    hook.on_alloc(ptr, layout.size());
                }
            }

            ptr
        }
    }

    unsafe fn realloc(&self, old_ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe {
            let new_ptr = self.inner.realloc(old_ptr, layout, new_size);

            if !new_ptr.is_null() {
                // SAFETY: get_malloc_hook() contract
                if let Some(hook) = get_malloc_hook() {
                    hook.on_realloc(old_ptr, new_ptr, new_size);
                }
            }

            new_ptr
        }
    }
}

/// Allocator query functions (for compatibility with C++ API).
///
/// These functions check what allocator is active. In Rust, the allocator
/// is determined at compile-time via `#[global_allocator]`, so these
/// functions are less useful than in C++ but provided for API compatibility.

/// Check if ptmalloc3 is the active allocator.
///
/// In Rust, this will always return `false` unless you've explicitly
/// configured a custom allocator that wraps ptmalloc3 via FFI.
///
/// # C++ Compatibility Note
///
/// The C++ version checks via `dlsym()` and environment variables.
/// In Rust, allocator detection requires build-time configuration.
#[inline]
pub fn is_ptmalloc_active() -> bool {
    // In Rust, we don't have runtime allocator detection like C++
    // This would require build-time configuration or feature flags
    false
}

/// Check if jemalloc is the active allocator.
///
/// Returns `true` if the `jemalloc` feature is enabled and jemalloc
/// is configured as the global allocator.
///
/// # Example
///
/// ```rust
/// use usd_arch::malloc_hook::is_jemalloc_active;
///
/// if is_jemalloc_active() {
///     println!("Using jemalloc allocator");
/// }
/// ```
#[inline]
pub fn is_jemalloc_active() -> bool {
    // Check if jemalloc feature is enabled
    #[cfg(feature = "jemalloc")]
    {
        true
    }
    #[cfg(not(feature = "jemalloc"))]
    {
        false
    }
}

/// Check if the STL allocator is disabled.
///
/// In C++, this checks for `GLIBCXX_FORCE_NEW` environment variable.
/// In Rust, this is not applicable as Rust doesn't use STL.
///
/// Always returns `false` in Rust. Provided for API compatibility only.
#[inline]
pub fn is_stl_allocator_off() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::{Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestHook {
        alloc_count: AtomicUsize,
        dealloc_count: AtomicUsize,
        realloc_count: AtomicUsize,
        total_allocated: AtomicUsize,
    }

    impl TestHook {
        fn new() -> Self {
            Self {
                alloc_count: AtomicUsize::new(0),
                dealloc_count: AtomicUsize::new(0),
                realloc_count: AtomicUsize::new(0),
                total_allocated: AtomicUsize::new(0),
            }
        }

        fn get_stats(&self) -> (usize, usize, usize, usize) {
            (
                self.alloc_count.load(Ordering::Relaxed),
                self.dealloc_count.load(Ordering::Relaxed),
                self.realloc_count.load(Ordering::Relaxed),
                self.total_allocated.load(Ordering::Relaxed),
            )
        }
    }

    impl MallocHook for TestHook {
        fn on_alloc(&self, _ptr: *mut u8, size: usize) {
            self.alloc_count.fetch_add(1, Ordering::Relaxed);
            self.total_allocated.fetch_add(size, Ordering::Relaxed);
        }

        fn on_dealloc(&self, _ptr: *mut u8) {
            self.dealloc_count.fetch_add(1, Ordering::Relaxed);
        }

        fn on_realloc(&self, _old_ptr: *mut u8, _new_ptr: *mut u8, size: usize) {
            self.realloc_count.fetch_add(1, Ordering::Relaxed);
            self.total_allocated.fetch_add(size, Ordering::Relaxed);
        }
    }

    #[test]
    fn test_hook_enable_disable() {
        assert!(!is_malloc_hook_enabled());

        let hook = Box::new(TestHook::new());
        set_malloc_hook(Some(hook));
        assert!(is_malloc_hook_enabled());

        set_malloc_hook(None);
        assert!(!is_malloc_hook_enabled());
    }

    #[test]
    fn test_hook_replacement() {
        let hook1 = Box::new(TestHook::new());
        set_malloc_hook(Some(hook1));
        assert!(is_malloc_hook_enabled());

        let hook2 = Box::new(TestHook::new());
        set_malloc_hook(Some(hook2));
        assert!(is_malloc_hook_enabled());

        set_malloc_hook(None);
    }

    #[test]
    fn test_instrumented_allocator() {
        let allocator = InstrumentedAllocator::new(System);
        let hook = Box::new(TestHook::new());

        // Get a reference to the hook before moving it
        let hook_ptr = &*hook as *const TestHook;
        set_malloc_hook(Some(hook));

        unsafe {
            let layout = Layout::from_size_align(64, 8).unwrap();
            let ptr = allocator.alloc(layout);
            assert!(!ptr.is_null());

            // Check hook was called
            let (alloc_count, _, _, total) = (*hook_ptr).get_stats();
            assert_eq!(alloc_count, 1);
            assert_eq!(total, 64);

            allocator.dealloc(ptr, layout);
            let (_, dealloc_count, _, _) = (*hook_ptr).get_stats();
            assert_eq!(dealloc_count, 1);
        }

        set_malloc_hook(None);
    }

    #[test]
    fn test_allocator_compatibility() {
        // These should not panic
        assert!(!is_ptmalloc_active());
        assert!(!is_stl_allocator_off());

        // is_jemalloc_active() depends on feature flag
        let _ = is_jemalloc_active();
    }

    #[test]
    fn test_hook_trait_object_safety() {
        // Verify we can create trait objects
        let hook: Box<dyn MallocHook> = Box::new(TestHook::new());
        hook.on_alloc(ptr::null_mut(), 100);
        hook.on_dealloc(ptr::null_mut());
        hook.on_realloc(ptr::null_mut(), ptr::null_mut(), 200);
    }
}
