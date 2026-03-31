// SAFETY: This module provides FFI bindings to system APIs requiring unsafe
#![allow(unsafe_code)]

//! Compiler hints for branch prediction and optimization.
//!
//! These functions provide hints to the compiler about expected branch outcomes,
//! which can improve performance in hot code paths.
//!
//! # Examples
//!
//! ```
//! use usd_arch::{likely, unlikely};
//!
//! fn process(value: i32) -> i32 {
//!     if unlikely(value < 0) {
//!         // Error handling path - rarely taken
//!         return -1;
//!     }
//!     // Normal path - usually taken
//!     value * 2
//! }
//! ```

/// Hints to the compiler that the condition is likely to be true.
///
/// Use this for conditions that are expected to be true in the common case.
/// This can help the compiler optimize branch prediction.
///
/// # Examples
///
/// ```
/// use usd_arch::likely;
///
/// let x = 42;
/// if likely(x > 0) {
///     println!("positive");
/// }
/// ```
#[inline(always)]
#[must_use]
pub const fn likely(b: bool) -> bool {
    #[cfg(feature = "nightly")]
    {
        std::intrinsics::likely(b)
    }
    #[cfg(not(feature = "nightly"))]
    {
        // On stable Rust, we can't use intrinsics directly
        b
    }
}

/// Hints to the compiler that the condition is unlikely to be true.
///
/// Use this for conditions that are expected to be false in the common case,
/// such as error checking or rare edge cases.
///
/// # Examples
///
/// ```
/// use usd_arch::unlikely;
///
/// fn check_error(code: i32) -> Result<(), &'static str> {
///     if unlikely(code != 0) {
///         return Err("error occurred");
///     }
///     Ok(())
/// }
/// ```
#[inline(always)]
#[must_use]
pub const fn unlikely(b: bool) -> bool {
    #[cfg(feature = "nightly")]
    {
        std::intrinsics::unlikely(b)
    }
    #[cfg(not(feature = "nightly"))]
    {
        b
    }
}

/// Tells the compiler that a condition is guaranteed to be true.
///
/// # Safety
///
/// This is an optimization hint that allows the compiler to assume the
/// condition is always true. If the condition is ever false, the behavior
/// is undefined. Only use this when you can mathematically prove the
/// condition holds.
///
/// # Examples
///
/// ```
/// use usd_arch::assume;
///
/// fn divide_positive(a: u32, b: u32) -> u32 {
///     // SAFETY: We know b > 0 from the function's contract
///     unsafe { assume(b > 0); }
///     a / b
/// }
/// ```
#[inline(always)]
pub unsafe fn assume(cond: bool) {
    unsafe {
        if !cond {
            #[cfg(feature = "nightly")]
            {
                std::hint::unreachable_unchecked();
            }
            #[cfg(not(feature = "nightly"))]
            {
                std::hint::unreachable_unchecked();
            }
        }
    }
}

/// Hints to the compiler that this code path is unreachable.
///
/// # Safety
///
/// If this code is ever executed, the behavior is undefined.
/// Only use when you can prove the code path is impossible.
#[inline(always)]
pub unsafe fn unreachable() -> ! {
    unsafe { std::hint::unreachable_unchecked() }
}

/// A cold function attribute marker.
///
/// Functions called through this wrapper are hinted as "cold" (rarely called),
/// which can help the compiler optimize the hot path.
#[inline(always)]
pub fn cold<F: FnOnce() -> R, R>(f: F) -> R {
    #[cold]
    #[inline(never)]
    fn cold_inner<F: FnOnce() -> R, R>(f: F) -> R {
        f()
    }
    cold_inner(f)
}

/// Emits a prefetch hint for the given memory address.
///
/// This hints to the CPU that the memory at the given address will be
/// accessed soon, potentially reducing cache miss latency.
///
/// # Arguments
///
/// * `ptr` - Pointer to the memory to prefetch
/// * `rw` - 0 for read, 1 for write
/// * `locality` - Temporal locality hint (0-3, higher = more local)
#[inline(always)]
pub fn prefetch<T>(ptr: *const T, _rw: i32, _locality: i32) {
    // Prefetch hints are architecture-specific and require intrinsics
    // For now, this is a no-op on stable Rust
    // The compiler may optimize memory access patterns anyway
    let _ = ptr;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_likely() {
        assert!(likely(true));
        assert!(!likely(false));
    }

    #[test]
    fn test_unlikely() {
        assert!(unlikely(true));
        assert!(!unlikely(false));
    }

    #[test]
    fn test_cold() {
        let result = cold(|| 42);
        assert_eq!(result, 42);
    }

    #[test]
    fn test_prefetch() {
        let data = [1, 2, 3, 4, 5];
        prefetch(data.as_ptr(), 0, 3);
        // Just ensure it compiles and doesn't crash
    }
}
