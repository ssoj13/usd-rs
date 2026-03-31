// SAFETY: This module provides FFI bindings to system APIs requiring unsafe
#![allow(unsafe_code)]

//! Platform assumption validation.
//!
//! This module provides functions to validate platform assumptions about
//! data types, endianness, and other architecture-specific properties.
//! This is the Rust equivalent of `pxr/base/arch/assumptions.cpp`.
//!
//! # Example
//!
//! ```ignore
//! use usd_arch::arch_validate_assumptions;
//!
//! // Validate platform assumptions (called during initialization)
//! arch_validate_assumptions();
//! ```

use crate::defines::CACHE_LINE_SIZE;
use crate::demangle::arch_get_demangled_type_name;
use crate::error::arch_warning;

/// Obtains the cache line size for the current architecture.
///
/// This function queries the system to determine the actual cache line size.
fn arch_obtain_cache_line_size() -> usize {
    #[cfg(target_os = "linux")]
    {
        // Try to read from /sys/devices/system/cpu/cpu0/cache/index0/coherency_line_size
        if let Ok(content) =
            std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cache/index0/coherency_line_size")
        {
            if let Ok(size) = content.trim().parse::<usize>() {
                return size;
            }
        }
        // Fallback: use sysconf on Linux
        unsafe {
            let size = libc::sysconf(libc::_SC_LEVEL1_DCACHE_LINESIZE);
            if size > 0 {
                return size as usize;
            }
        }
        return 64;
    }

    #[cfg(target_os = "macos")]
    {
        use std::mem;
        let mut cache_line_size: usize = 0;
        let mut size = mem::size_of::<usize>();

        // C++ uses sysctlbyname("hw.cachelinesize", ...) - string-based API.
        // sysctl() takes integer MIB arrays, which is different.
        unsafe {
            let result = libc::sysctlbyname(
                b"hw.cachelinesize\0".as_ptr() as *const libc::c_char,
                &mut cache_line_size as *mut _ as *mut libc::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            );
            if result == 0 && cache_line_size > 0 {
                return cache_line_size;
            }
        }
        // Fallback for macOS if sysctlbyname fails
        64
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, we'd need to use GetLogicalProcessorInformation
        // For now, use a reasonable default
        64
    }

    #[cfg(target_family = "wasm")]
    {
        return 64;
    }

    // Default fallback for other platforms
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "windows",
        target_family = "wasm"
    )))]
    64
}

/// Validates platform assumptions about data types and architecture properties.
///
/// This function performs various compile-time and runtime checks to ensure
/// that the platform meets the assumptions made by the USD codebase:
///
/// - Enum size equals int size
/// - int is 4 bytes
/// - float and double are IEEE-754 compliant
/// - Demangling works correctly
/// - Cache line size matches expected value
/// - System is little-endian
///
/// This function is called during library initialization to catch platform
/// compatibility issues early.
///
/// # Example
///
/// ```ignore
/// use usd_arch::arch_validate_assumptions;
///
/// // Validate assumptions (typically called during init)
/// arch_validate_assumptions();
/// ```
pub fn arch_validate_assumptions() {
    // Check repr(i32) enum size equals int size (C++ compatible)
    #[allow(dead_code)]
    #[repr(i32)]
    enum SomeEnum {
        Blah,
    }

    // Runtime checks (compile-time checks would require const_panic which is unstable)
    // Note: In Rust, plain enums don't have C++ int size - use #[repr(i32)] for C++ compatibility
    assert_eq!(
        std::mem::size_of::<SomeEnum>(),
        std::mem::size_of::<i32>(),
        "sizeof(repr(i32) enum) != sizeof(int)"
    );

    // Check int size is 4 bytes
    assert_eq!(std::mem::size_of::<i32>(), 4, "sizeof(int) != 4");

    // Verify float and double are IEEE-754 compliant
    // Rust guarantees this for f32 and f64, but we check sizes anyway
    assert_eq!(std::mem::size_of::<f32>(), 4, "sizeof(float) != 4");
    assert_eq!(std::mem::size_of::<f64>(), 8, "sizeof(double) != 8");

    // Check IEEE-754 compliance using numeric_limits equivalent
    // Rust's f32 and f64 are guaranteed to be IEEE-754 compliant
    // We verify the sizes match expectations
    assert_eq!(std::mem::size_of::<f32>(), 4);
    assert_eq!(std::mem::size_of::<f64>(), 8);

    // Check the demangler on a very simple type
    let demangled = arch_get_demangled_type_name::<i32>();
    if demangled != "i32" && !demangled.contains("int") {
        arch_warning!("C++ demangling appears badly broken.");
    }

    // Check cache line size
    let cache_line_size = arch_obtain_cache_line_size();

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        // On macOS with Rosetta 2, we may be an Intel x86_64 binary running on
        // Apple Silicon arm64 cpu. macOS always returns the underlying HW's cache
        // line size, so we explicitly approve this exception here.
        const ROSETTA_WORKAROUND_CACHE_LINE_SIZE: usize = 128;
        // Note: We can't easily detect Rosetta 2 in Rust without platform-specific code
        // For now, we'll just check if the detected size doesn't match our constant
        if cache_line_size != CACHE_LINE_SIZE
            && cache_line_size != ROSETTA_WORKAROUND_CACHE_LINE_SIZE
        {
            arch_warning!("Cache-line size mismatch may negatively impact performance.");
        }
    }

    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        if cache_line_size != CACHE_LINE_SIZE {
            arch_warning!("ARCH_CACHE_LINE_SIZE != Arch_ObtainCacheLineSize()");
        }
    }

    // Make sure that the machine is little-endian
    {
        let buf: [u8; 4] = [1, 2, 3, 4];
        let check: u32 = unsafe { std::ptr::read(buf.as_ptr() as *const u32) };
        if check != 0x04030201 {
            crate::error::arch_error!("Big-endian byte order not supported.");
        }
    }

    // Windows ARM64 specific check
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        // Check ARM64_CNTVCT == 0x5F02
        // This is manually calculated in pxr/base/arch/timing.h
        // In Rust, we'd need to verify this constant if we have timing code that uses it
        // For now, we skip this check as it's Windows ARM64 specific
    }
}

// Note: Compile-time assertions would require const_panic which is unstable.
// We use runtime assertions instead, which is acceptable since these checks
// are performed during initialization and will catch platform issues early.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arch_validate_assumptions() {
        // This should not panic on a supported platform
        arch_validate_assumptions();
    }

    #[test]
    fn test_cache_line_size() {
        let size = arch_obtain_cache_line_size();
        assert!(size > 0, "Cache line size should be positive");
        assert!(size <= 256, "Cache line size should be reasonable");
    }

    #[test]
    fn test_endianness() {
        let buf: [u8; 4] = [1, 2, 3, 4];
        let check: u32 = unsafe { std::ptr::read(buf.as_ptr() as *const u32) };
        // We require little-endian
        assert_eq!(check, 0x04030201, "System must be little-endian");
    }
}
