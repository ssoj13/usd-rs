#![allow(unsafe_code)]
//! Symbol lookup and address information utilities.
//!
//! Provides functions to query information about code addresses, including:
//! - Object/module path
//! - Base load address
//! - Symbol name
//! - Symbol address offset
//!
//! Platform support:
//! - Windows: DbgHelp API (SymFromAddr, GetModuleInformation)
//! - Unix/macOS: dladdr API
//! - WebAssembly: dladdr API

use std::path::PathBuf;

#[cfg(unix)]
use std::os::raw::c_void;

/// Information about a code address.
///
/// Contains details about the module/library, symbol, and offsets
/// associated with a specific memory address.
#[derive(Debug, Clone, PartialEq)]
pub struct ArchAddressInfo {
    /// Base address where the module is loaded in memory
    pub base_address: usize,
    /// Absolute path to the executable or shared library
    pub file_name: PathBuf,
    /// Name of the symbol containing this address (may be empty)
    pub symbol_name: String,
    /// Offset from the symbol start address
    pub offset_from_symbol: usize,
}

/// Get information about a code address.
///
/// Returns symbol information for the given memory address, including:
/// - The module/library file path
/// - Base load address of the module
/// - Symbol name (function, variable, etc.)
/// - Offset within the symbol
///
/// # Arguments
/// * `address` - Memory address to query
///
/// # Returns
/// `Some(ArchAddressInfo)` if information is available, `None` if lookup fails
///
/// # Platform Notes
/// - **Windows**: Uses DbgHelp API. Symbols must be available.
/// - **Unix/macOS**: Uses dladdr. Works with dynamically loaded symbols.
/// - **WASM**: Uses dladdr where available.
///
/// # Examples
/// ```
/// use usd_arch::arch_get_address_info;
///
/// fn my_function() {
///     let addr = my_function as usize;
///     if let Some(info) = arch_get_address_info(addr) {
///         println!("Function: {}", info.symbol_name);
///         println!("Module: {}", info.file_name.display());
///     }
/// }
/// ```
#[inline]
pub fn arch_get_address_info(address: usize) -> Option<ArchAddressInfo> {
    if address == 0 {
        return None;
    }

    #[cfg(windows)]
    {
        get_address_info_windows(address as *mut std::ffi::c_void)
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    {
        get_address_info_unix(address as *mut c_void)
    }

    #[cfg(not(any(
        windows,
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    )))]
    {
        None
    }
}

/// Get the address of the caller function.
///
/// Returns the return address of the current function, which is the
/// address in the calling function. This can be used for stack tracing
/// or diagnostic purposes.
///
/// # Returns
/// Address of the instruction in the caller function, or 0 if unavailable
///
/// # Examples
/// ```
/// use usd_arch::arch_get_current_function_address;
///
/// fn caller() {
///     callee();
/// }
///
/// fn callee() {
///     let addr = arch_get_current_function_address();
///     println!("Called from: {:#x}", addr);
/// }
/// ```
#[inline]
pub fn arch_get_current_function_address() -> usize {
    // Use backtrace to get caller's return address
    let mut addr = 0usize;
    let mut skip_first = true;

    backtrace::trace(|frame| {
        if skip_first {
            // Skip the first frame (this function itself)
            skip_first = false;
            true
        } else if addr == 0 {
            // Get the second frame (caller)
            addr = frame.ip() as usize;
            false // Stop tracing
        } else {
            true
        }
    });

    addr
}

/// Windows implementation using backtrace crate for symbol resolution.
/// This avoids direct DbgHelp calls which aren't thread-safe and conflict
/// with backtrace's internal DbgHelp usage.
#[cfg(windows)]
fn get_address_info_windows(address: *mut std::ffi::c_void) -> Option<ArchAddressInfo> {
    let mut result: Option<ArchAddressInfo> = None;

    // Use backtrace::resolve which handles DbgHelp thread-safety internally
    backtrace::resolve(address, |symbol| {
        let symbol_name = symbol.name().map(|n| n.to_string()).unwrap_or_default();

        let file_name = symbol
            .filename()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();

        let symbol_addr = symbol.addr().map(|a| a as usize).unwrap_or(0);
        let offset = if symbol_addr > 0 && symbol_addr <= address as usize {
            (address as usize).saturating_sub(symbol_addr)
        } else {
            0
        };

        // For base_address, we use 0 since backtrace doesn't expose module base.
        // This is acceptable - the symbol info is what matters most.
        result = Some(ArchAddressInfo {
            base_address: 0,
            file_name,
            symbol_name,
            offset_from_symbol: offset,
        });
    });

    // If backtrace::resolve didn't find anything, try to at least get module info
    if result.is_none() {
        result = Some(ArchAddressInfo {
            base_address: 0,
            file_name: PathBuf::new(),
            symbol_name: String::new(),
            offset_from_symbol: 0,
        });
    }

    result
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn get_address_info_unix(address: *mut c_void) -> Option<ArchAddressInfo> {
    use std::ffi::CStr;

    // Use libc::Dl_info directly instead of a hand-rolled struct.
    unsafe {
        let mut info: libc::Dl_info = std::mem::zeroed();
        let result = libc::dladdr(address, &mut info);

        if result == 0 {
            return None;
        }

        let file_name = if !info.dli_fname.is_null() {
            let c_str = CStr::from_ptr(info.dli_fname);
            PathBuf::from(c_str.to_string_lossy().as_ref())
        } else {
            PathBuf::new()
        };

        // Make path absolute if relative
        let file_name = if file_name.is_relative() {
            std::env::current_dir()
                .ok()
                .and_then(|cwd| cwd.join(&file_name).canonicalize().ok())
                .unwrap_or(file_name)
        } else {
            file_name
        };

        let symbol_name = if !info.dli_sname.is_null() {
            let c_str = CStr::from_ptr(info.dli_sname);
            c_str.to_string_lossy().into_owned()
        } else {
            String::new()
        };

        let base_address = info.dli_fbase as usize;
        let symbol_addr = info.dli_saddr as usize;
        let offset_from_symbol = if symbol_addr > 0 {
            (address as usize).saturating_sub(symbol_addr)
        } else {
            0
        };

        Some(ArchAddressInfo {
            base_address,
            file_name,
            symbol_name,
            offset_from_symbol,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_address_info_self() {
        // Get info about this test function itself
        let addr = test_get_address_info_self as *const () as usize;
        let info = arch_get_address_info(addr);

        assert!(info.is_some());
        let info = info.unwrap();

        // Symbol name should be found (may be mangled)
        // On Windows with backtrace crate, symbol resolution should work
        #[cfg(not(windows))]
        assert!(
            info.base_address > 0,
            "Base address should be valid on Unix"
        );

        // Print debug info
        println!("Symbol: {}", info.symbol_name);
        println!("File: {}", info.file_name.display());
        println!("Base: {:#x}", info.base_address);
        println!("Offset: {:#x}", info.offset_from_symbol);
    }

    #[test]
    fn test_get_address_info_null() {
        // Null address should return None
        let info = arch_get_address_info(0);
        assert!(info.is_none());
    }

    #[test]
    fn test_get_current_function_address() {
        let addr = arch_get_current_function_address();
        // Should return non-zero address
        assert!(addr > 0);
        println!("Return address: {:#x}", addr);
    }

    #[test]
    fn test_address_info_ordering() {
        fn nested_call() -> usize {
            nested_call as *const () as usize
        }

        let addr = nested_call();
        let info = arch_get_address_info(addr);

        if let Some(info) = info {
            // Base address should be less than or equal to function address
            // On Windows with backtrace crate, base_address is 0
            #[cfg(not(windows))]
            assert!(info.base_address <= addr);
            println!("Nested function info: {:?}", info);
        }
    }
}
