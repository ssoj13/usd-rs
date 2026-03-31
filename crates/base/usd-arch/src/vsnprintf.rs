// Copyright 2025 Joss Whittle
//

// SAFETY: This module provides FFI bindings to system APIs requiring unsafe
#![allow(unsafe_code, dead_code, private_interfaces)]

//! Architecture-dependent memory-safe sprintf capability.
//!
//! # Safety Note
//!
//! In Rust, the `format!` macro and related formatting macros are **already safe by design**.
//! They perform compile-time format string checking and prevent buffer overflows.
//!
//! This module exists primarily for **C interoperability** when interfacing with code that
//! uses `vsnprintf` and related C formatting functions.
//!
//! # Rust-native Formatting
//!
//! For pure Rust code, use standard formatting:
//! ```
//! let i = 0;
//! let val = 42;
//! let msg = format!("val[{}] = {}", i, val);
//! ```
//!
//! # C Interop
//!
//! When interfacing with C code:
//! ```ignore
//! use std::ffi::CString;
//! use usd_arch::arch_vsnprintf;
//!
//! // Safe wrapper around C vsnprintf
//! let mut buf = [0u8; 256];
//! let fmt = CString::new("value = %d").unwrap();
//! // Note: actual va_list usage requires unsafe FFI
//! ```

use std::ffi::CStr;
use std::fmt::{self, Display, Write as FmtWrite};
use std::os::raw::{c_char, c_int};

#[cfg(unix)]
use std::os::raw::c_void;

// Platform-specific va_list type
#[cfg(target_os = "windows")]
type VaList = *mut c_char;

#[cfg(target_os = "linux")]
type VaList = *mut __va_list_tag;

#[cfg(target_os = "macos")]
type VaList = *mut c_void;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[repr(C)]
struct __va_list_tag {
    gp_offset: u32,
    fp_offset: u32,
    overflow_arg_area: *mut c_void,
    reg_save_area: *mut c_void,
}

// The __va_list_tag layout above is x86_64-specific. On aarch64-linux
// va_list is a completely different struct. Gate at compile time.
#[cfg(all(target_os = "linux", not(target_arch = "x86_64")))]
compile_error!(
    "vsnprintf FFI: __va_list_tag layout is only defined for x86_64-linux. \
    aarch64 and other arches need a different struct definition."
);

// Platform-specific vsnprintf linkage
#[cfg(not(target_os = "windows"))]
unsafe extern "C" {
    fn vsnprintf(s: *mut c_char, n: usize, format: *const c_char, ap: VaList) -> c_int;
}

#[cfg(target_os = "windows")]
#[link(name = "legacy_stdio_definitions")]
unsafe extern "C" {
    // On Windows, link against _vsnprintf from legacy_stdio_definitions
    #[link_name = "_vsnprintf"]
    fn vsnprintf(s: *mut c_char, n: usize, format: *const c_char, ap: VaList) -> c_int;
}

/// Return the number of characters (not including the null character)
/// necessary for a particular sprintf into a string.
///
/// `arch_vsnprintf` guarantees the C99 behavior of vsnprintf on all systems:
/// it returns the number of bytes (not including the terminating null
/// character) needed to actually print the requested string. If `size`
/// indicates that `buf` has enough capacity to hold the result, then the
/// function actually prints into `buf`.
///
/// # Arguments
///
/// * `buf` - Output buffer (may be empty slice to query size)
/// * `format` - C-style format string
/// * `ap` - Variable argument list pointer
///
/// # Returns
///
/// Number of characters that would be written (excluding null terminator).
///
/// # Safety
///
/// This function is unsafe because:
/// - `format` must be a valid null-terminated C string
/// - `ap` must be a valid va_list pointer matching the format specifiers
/// - Buffer overflow is prevented, but mismatched format/args cause UB
///
/// # Example
///
/// ```ignore
/// use std::ffi::CString;
/// use usd_arch::arch_vsnprintf;
///
/// // Typically called from C FFI boundary
/// unsafe extern "C" fn my_printf_wrapper(fmt: *const c_char, mut ap: ...) {
///     let mut buf = [0u8; 256];
///     let len = arch_vsnprintf(&mut buf, fmt, ap);
///     println!("Would need {} bytes", len);
/// }
/// ```
#[inline]
pub(crate) unsafe fn arch_vsnprintf(buf: &mut [u8], format: *const c_char, ap: VaList) -> i32 {
    unsafe {
        // vsnprintf either prints into buf, or aborts the print
        // but tells you how much room was needed.
        vsnprintf(buf.as_mut_ptr() as *mut c_char, buf.len(), format, ap)
    }
}

/// Returns a string formed by a printf()-like specification.
///
/// `arch_vstring_printf` is equivalent to `arch_string_printf` except that
/// it is called with a `va_list` instead of a variable number of arguments.
///
/// **Note**: This function does not call the `va_end` macro. Consequently,
/// the value of `ap` is undefined after the call. A function that calls
/// `arch_vstring_printf` should call `va_end(ap)` itself afterwards.
///
/// # Arguments
///
/// * `format` - C-style format string
/// * `ap` - Variable argument list pointer
///
/// # Returns
///
/// Formatted string
///
/// # Safety
///
/// This function is unsafe because:
/// - `format` must be a valid null-terminated C string
/// - `ap` must be a valid va_list pointer matching the format specifiers
///
/// # Example
///
/// ```ignore
/// use std::ffi::CString;
/// use usd_arch::arch_vstring_printf;
///
/// unsafe extern "C" fn format_message(fmt: *const c_char, mut ap: ...) -> String {
///     let result = arch_vstring_printf(fmt, ap);
///     // Caller must call va_end(ap)
///     result
/// }
/// ```
pub(crate) unsafe fn arch_vstring_printf(format: *const c_char, ap: VaList) -> String {
    unsafe {
        // va_copy is a C macro, not callable via FFI — so we cannot safely invoke vsnprintf
        // twice with the same va_list on register-based ABIs (Linux x86-64, macOS arm64).
        // The safe single-pass approach: use a large enough stack buffer (64 KiB covers all
        // practical cases) and make exactly ONE vsnprintf call. The ap pointer is consumed
        // by this single call; caller is responsible for va_end on the original ap.
        //
        // 64 KiB matches what Boost.Format and many other libs use as their upper bound.
        let mut buf = [0u8; 65536];

        let written = vsnprintf(buf.as_mut_ptr() as *mut c_char, buf.len(), format, ap);

        if written < 0 {
            // Encoding error from vsnprintf
            return String::new();
        }

        // written is the number of chars that would have been written (C99 semantics).
        // Clamp to actual buffer content in case output was truncated.
        let len = (written as usize).min(buf.len().saturating_sub(1));
        // Safety: vsnprintf guarantees null-termination within buf.len() bytes.
        let cstr =
            CStr::from_bytes_until_nul(&buf[..=len]).expect("vsnprintf should null-terminate");
        cstr.to_string_lossy().into_owned()
    }
}

/// Returns a string formed by a printf()-like specification.
///
/// `arch_string_printf` is a memory-safe architecture-independent way of
/// forming a string using printf()-like formatting.
///
/// **Important**: This is for C FFI compatibility only. For pure Rust code,
/// use the standard `format!` macro which is safe by design.
///
/// # Arguments
///
/// * `format` - C-style format string
/// * `...` - Variable arguments matching format specifiers
///
/// # Returns
///
/// Formatted string
///
/// # Safety
///
/// This function is unsafe because:
/// - `format` must be a valid null-terminated C string
/// - Variable arguments must match the format specifiers
///
/// # Example
///
/// ```ignore
/// use std::ffi::CString;
/// use usd_arch::arch_string_printf;
///
/// unsafe {
///     let fmt = CString::new("val[%d] = %g\n").unwrap();
///     let msg = arch_string_printf(fmt.as_ptr(), 5, 3.14);
///     println!("{}", msg);
/// }
/// ```
///
/// **Prefer Rust formatting**:
/// ```
/// let msg = format!("val[{}] = {}\n", 5, 3.14);
/// ```
// Note: arch_string_printf with variadic args is not available in stable Rust.
// It requires the unstable c_variadic feature.
// For C interop, create a C wrapper that calls arch_vstring_printf instead.

/// Rust-native safe formatting helper.
///
/// This is a safe alternative to printf-style formatting for pure Rust code.
/// It uses Rust's `Display` trait for type-safe formatting.
///
/// # Arguments
///
/// * `fmt` - Format string (Rust-style, not printf-style)
/// * `args` - Slice of displayable arguments
///
/// # Returns
///
/// Formatted string
///
/// # Note
///
/// This is a simplified helper. For complex formatting, use the standard
/// `format!` macro which is more powerful and zero-cost.
///
/// # Example
///
/// ```ignore
/// use usd_arch::format_safe;
///
/// let result = format_safe("Value: {}, Name: {}", &[&42, &"test"]);
/// // result is Ok("Value: 42, Name: test")
/// ```
///
/// **Prefer standard formatting**:
/// ```
/// let result = format!("Value: {}, Name: {}", 42, "test");
/// ```
pub fn format_safe(template: &str, args: &[&dyn Display]) -> Result<String, fmt::Error> {
    let mut result = String::new();
    let mut arg_idx = 0;
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            if chars.peek() == Some(&'{') {
                // Escaped brace
                chars.next();
                result.push('{');
            } else if chars.peek() == Some(&'}') {
                // Empty placeholder
                chars.next();
                if arg_idx < args.len() {
                    write!(&mut result, "{}", args[arg_idx])?;
                    arg_idx += 1;
                }
            } else {
                // Skip format specifiers (simplified)
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c == '}' {
                        break;
                    }
                }
                if arg_idx < args.len() {
                    write!(&mut result, "{}", args[arg_idx])?;
                    arg_idx += 1;
                }
            }
        } else if ch == '}' {
            if chars.peek() == Some(&'}') {
                // Escaped brace
                chars.next();
                result.push('}');
            }
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}

/// Macro for safe sprintf into fixed-size buffer.
///
/// This macro provides a safe way to format into a fixed-size byte buffer,
/// preventing buffer overflows. It returns the formatted string slice.
///
/// # Example
///
/// ```rust
/// use usd_arch::arch_safe_sprintf;
///
/// let mut buf = [0u8; 64];
/// let result = arch_safe_sprintf!(&mut buf, "Value: {}, Count: {}", 42, 10);
/// assert_eq!(result, "Value: 42, Count: 10");
/// ```
///
/// **Note**: For dynamic strings, prefer `format!` which is more flexible:
/// ```
/// let result = format!("Value: {}, Count: {}", 42, 10);
/// ```
#[macro_export]
macro_rules! arch_safe_sprintf {
    ($buf:expr, $($arg:tt)*) => {{
        let formatted = format!($($arg)*);
        let bytes = formatted.as_bytes();
        let buf: &mut [u8] = $buf;
        let len = bytes.len().min(buf.len().saturating_sub(1));
        buf[..len].copy_from_slice(&bytes[..len]);
        buf[len] = 0; // Null terminate
        std::str::from_utf8(&buf[..len]).unwrap_or("")
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_safe_basic() {
        let result = format_safe("Hello, {}!", &[&"world"]).unwrap();
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_format_safe_multiple() {
        let result = format_safe("x={}, y={}, z={}", &[&1, &2, &3]).unwrap();
        assert_eq!(result, "x=1, y=2, z=3");
    }

    #[test]
    fn test_format_safe_escaped_braces() {
        let result = format_safe("{{escaped}} {}", &[&42]).unwrap();
        assert_eq!(result, "{escaped} 42");
    }

    #[test]
    fn test_format_safe_no_args() {
        let result = format_safe("No arguments", &[]).unwrap();
        assert_eq!(result, "No arguments");
    }

    #[test]
    fn test_arch_safe_sprintf_macro() {
        let mut buf = [0u8; 64];
        let result = arch_safe_sprintf!(&mut buf, "Test: {}", 123);
        assert_eq!(result, "Test: 123");
        assert_eq!(buf[9], 0); // Null terminated
    }

    #[test]
    fn test_arch_safe_sprintf_overflow() {
        let mut buf = [0u8; 8];
        let result = arch_safe_sprintf!(&mut buf, "This is a very long string");
        assert_eq!(result, "This is");
        assert_eq!(buf[7], 0); // Null terminated
    }

    #[test]
    fn test_arch_safe_sprintf_exact_fit() {
        let mut buf = [0u8; 6];
        let result = arch_safe_sprintf!(&mut buf, "12345");
        assert_eq!(result, "12345");
        assert_eq!(buf[5], 0); // Null terminated
    }

    #[test]
    fn test_rust_formatting_comparison() {
        // Demonstrate that Rust's format! is the preferred approach
        let value = 42;
        let name = "test";

        // Rust way (preferred)
        let rust_result = format!("Value: {}, Name: {}", value, name);

        // Our helper (for compatibility)
        let compat_result = format_safe("Value: {}, Name: {}", &[&value, &name]).unwrap();

        assert_eq!(rust_result, "Value: 42, Name: test");
        assert_eq!(compat_result, "Value: 42, Name: test");
    }

    #[cfg(unix)]
    #[test]
    fn test_arch_vsnprintf_basic() {
        // Can't easily test without actual C varargs;
        // verify the function signature compiles
        let _ = arch_vsnprintf;
    }
}
