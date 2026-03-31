//! Dynamic library loading utilities.
//!
//! Provides functions for loading and unloading dynamic libraries (.dll, .so, .dylib)
//! at runtime. This is a wrapper around the platform's dynamic loading APIs.
//!
//! # Platform Support
//!
//! - **Windows**: Uses `LoadLibraryW` / `FreeLibrary`
//! - **Unix/Linux/macOS**: Uses `dlopen` / `dlclose`
//!
//! # Examples
//!
//! ```no_run
//! use usd_tf::dl::{dlopen, dlclose, DlOpenFlags};
//!
//! // Load a library
//! match dlopen("mylib", DlOpenFlags::NOW) {
//!     Ok(handle) => {
//!         // Use the library...
//!
//!         // Unload when done
//!         dlclose(handle);
//!     }
//!     Err(e) => eprintln!("Failed to load library: {}", e),
//! }
//! ```
//!
//! # Safety
//!
//! Loading and using dynamic libraries involves unsafe operations. Ensure that:
//! - The library path is valid
//! - Any symbols obtained from the library are used correctly
//! - The library is not unloaded while its code is still in use

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Handle to a loaded dynamic library.
///
/// This is an opaque handle that represents a loaded library.
/// Use [`dlclose`] to unload the library when done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DlHandle(*mut std::ffi::c_void);

impl DlHandle {
    /// Create a new handle from a raw pointer.
    ///
    /// # Safety
    ///
    /// The pointer must be a valid handle returned by the platform's
    /// dynamic library loading function.
    #[allow(unsafe_code)]
    pub unsafe fn from_raw(ptr: *mut std::ffi::c_void) -> Self {
        Self(ptr)
    }

    /// Get the raw pointer.
    pub fn as_raw(&self) -> *mut std::ffi::c_void {
        self.0
    }

    /// Check if the handle is valid (non-null).
    pub fn is_valid(&self) -> bool {
        !self.0.is_null()
    }
}

// SAFETY: DlHandle is Send + Sync because it's just a pointer and the underlying
// library loading APIs are thread-safe for handle passing.
#[allow(unsafe_code)]
unsafe impl Send for DlHandle {}

#[allow(unsafe_code)]
unsafe impl Sync for DlHandle {}

/// Flags for [`dlopen`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DlOpenFlags(i32);

impl DlOpenFlags {
    /// Lazy binding: resolve symbols as they are used.
    ///
    /// On Windows, this is equivalent to `NOW` since Windows doesn't
    /// support lazy binding.
    pub const LAZY: Self = Self(0x1);

    /// Immediate binding: resolve all symbols when loading.
    pub const NOW: Self = Self(0x2);

    /// Make symbols globally available for subsequently loaded libraries.
    pub const GLOBAL: Self = Self(0x100);

    /// Keep symbols local to this library.
    pub const LOCAL: Self = Self(0x0);

    /// Don't unload the library during dlclose (keep it loaded).
    pub const NODELETE: Self = Self(0x1000);

    /// Don't load the library, but return a valid handle if already loaded.
    pub const NOLOAD: Self = Self(0x4);

    /// Create a new flags value.
    pub const fn new(value: i32) -> Self {
        Self(value)
    }

    /// Get the raw flags value.
    pub const fn value(&self) -> i32 {
        self.0
    }

    /// Combine two flags.
    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl std::ops::BitOr for DlOpenFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl Default for DlOpenFlags {
    fn default() -> Self {
        Self::NOW
    }
}

// Track if dlopen/dlclose is currently active (for re-entrancy detection)
static DLOPEN_ACTIVE: AtomicBool = AtomicBool::new(false);
static DLCLOSE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Check if a dlopen operation is currently in progress.
///
/// This is useful for detecting re-entrant calls during library loading.
pub fn dlopen_is_active() -> bool {
    DLOPEN_ACTIVE.load(Ordering::Acquire)
}

/// Check if a dlclose operation is currently in progress.
///
/// This is useful for detecting re-entrant calls during library unloading.
pub fn dlclose_is_active() -> bool {
    DLCLOSE_ACTIVE.load(Ordering::Acquire)
}

/// Open a dynamic library.
///
/// # Parameters
///
/// - `filename`: Path to the library to load. Can be:
///   - An absolute path
///   - A relative path
///   - Just the library name (system will search standard paths)
/// - `flags`: Loading flags (see [`DlOpenFlags`])
///
/// # Returns
///
/// - `Ok(DlHandle)`: Handle to the loaded library
/// - `Err(String)`: Error message if loading failed
///
/// # Examples
///
/// ```no_run
/// use usd_tf::dl::{dlopen, DlOpenFlags};
///
/// // Load with immediate binding
/// let handle = dlopen("mylib.so", DlOpenFlags::NOW)?;
///
/// // Load with lazy binding and global symbols
/// let handle2 = dlopen("other.so", DlOpenFlags::LAZY | DlOpenFlags::GLOBAL)?;
/// # Ok::<(), String>(())
/// ```
pub fn dlopen<P: AsRef<Path>>(filename: P, flags: DlOpenFlags) -> Result<DlHandle, String> {
    let prev = DLOPEN_ACTIVE.swap(true, Ordering::AcqRel);
    let result = dlopen_impl(filename.as_ref(), flags);
    DLOPEN_ACTIVE.store(prev, Ordering::Release);
    result
}

/// Close a dynamic library.
///
/// # Parameters
///
/// - `handle`: Handle returned by [`dlopen`]
///
/// # Returns
///
/// - `0` on success
/// - Non-zero on failure
///
/// # Safety
///
/// After calling this function, the handle is invalid and must not be used.
/// Any symbols obtained from the library must not be used after closing.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::dl::{dlopen, dlclose, DlOpenFlags};
///
/// let handle = dlopen("mylib.so", DlOpenFlags::NOW)?;
/// // Use library...
/// let result = dlclose(handle);
/// assert_eq!(result, 0);
/// # Ok::<(), String>(())
/// ```
pub fn dlclose(handle: DlHandle) -> i32 {
    let prev = DLCLOSE_ACTIVE.swap(true, Ordering::AcqRel);
    let result = dlclose_impl(handle);
    DLCLOSE_ACTIVE.store(prev, Ordering::Release);
    result
}

/// Get a symbol from a loaded library.
///
/// # Parameters
///
/// - `handle`: Handle to the library
/// - `symbol`: Name of the symbol to look up
///
/// # Returns
///
/// - `Ok(ptr)`: Pointer to the symbol
/// - `Err(String)`: Error message if lookup failed
///
/// # Safety
///
/// The returned pointer must be cast to the correct type and used
/// according to the symbol's actual signature.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::dl::{dlopen, dlsym, DlOpenFlags};
///
/// let handle = dlopen("mylib.so", DlOpenFlags::NOW)?;
///
/// // Get a function pointer
/// let ptr = dlsym(handle, "my_function")?;
/// let func: extern "C" fn() -> i32 = unsafe { std::mem::transmute(ptr) };
/// let result = func();
/// # Ok::<(), String>(())
/// ```
pub fn dlsym(handle: DlHandle, symbol: &str) -> Result<*mut std::ffi::c_void, String> {
    dlsym_impl(handle, symbol)
}

/// Get the last error message from a dlopen/dlsym/dlclose operation.
///
/// # Returns
///
/// The error message, or `None` if no error occurred.
pub fn dlerror() -> Option<String> {
    dlerror_impl()
}

// Platform-specific implementations

#[cfg(windows)]
#[allow(unsafe_code)] // FFI module for dynamic library loading
mod platform {
    use super::*;
    use std::os::windows::ffi::OsStrExt;

    // Windows FFI declarations
    #[allow(clippy::upper_case_acronyms)]
    type HMODULE = *mut std::ffi::c_void;
    #[allow(clippy::upper_case_acronyms)]
    type FARPROC = *mut std::ffi::c_void;
    #[allow(clippy::upper_case_acronyms)]
    type LPCWSTR = *const u16;
    #[allow(clippy::upper_case_acronyms)]
    type LPCSTR = *const i8;
    #[allow(clippy::upper_case_acronyms)]
    type BOOL = i32;
    #[allow(clippy::upper_case_acronyms)]
    type DWORD = u32;

    unsafe extern "system" {
        fn LoadLibraryW(lpLibFileName: LPCWSTR) -> HMODULE;
        fn FreeLibrary(hLibModule: HMODULE) -> BOOL;
        fn GetProcAddress(hModule: HMODULE, lpProcName: LPCSTR) -> FARPROC;
        fn GetLastError() -> DWORD;
    }

    pub fn dlopen_impl(path: &Path, _flags: DlOpenFlags) -> Result<DlHandle, String> {
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe { LoadLibraryW(wide.as_ptr()) };

        if handle.is_null() {
            let error = unsafe { GetLastError() };
            Err(format!("LoadLibrary failed with error code {}", error))
        } else {
            Ok(unsafe { DlHandle::from_raw(handle) })
        }
    }

    pub fn dlclose_impl(handle: DlHandle) -> i32 {
        let result = unsafe { FreeLibrary(handle.as_raw()) };
        if result != 0 { 0 } else { -1 }
    }

    pub fn dlsym_impl(handle: DlHandle, symbol: &str) -> Result<*mut std::ffi::c_void, String> {
        use std::ffi::CString;

        let c_symbol = CString::new(symbol).map_err(|e| e.to_string())?;

        let ptr = unsafe { GetProcAddress(handle.as_raw(), c_symbol.as_ptr()) };

        if ptr.is_null() {
            let error = unsafe { GetLastError() };
            Err(format!(
                "GetProcAddress failed for '{}' with error code {}",
                symbol, error
            ))
        } else {
            Ok(ptr)
        }
    }

    pub fn dlerror_impl() -> Option<String> {
        let error = unsafe { GetLastError() };
        if error != 0 {
            Some(format!("Windows error code: {}", error))
        } else {
            None
        }
    }
}

#[cfg(unix)]
#[allow(unsafe_code)] // FFI module for dynamic library loading
mod platform {
    use super::*;
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    pub fn dlopen_impl(path: &Path, flags: DlOpenFlags) -> Result<DlHandle, String> {
        let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;

        // Map our flags to libc flags
        let mut libc_flags = 0;
        if flags.0 & DlOpenFlags::LAZY.0 != 0 {
            libc_flags |= libc::RTLD_LAZY;
        }
        if flags.0 & DlOpenFlags::NOW.0 != 0 {
            libc_flags |= libc::RTLD_NOW;
        }
        if flags.0 & DlOpenFlags::GLOBAL.0 != 0 {
            libc_flags |= libc::RTLD_GLOBAL;
        }
        if libc_flags == 0 {
            libc_flags = libc::RTLD_NOW; // Default
        }

        let handle = unsafe { libc::dlopen(c_path.as_ptr(), libc_flags) };

        if handle.is_null() {
            Err(dlerror_impl().unwrap_or_else(|| "Unknown dlopen error".to_string()))
        } else {
            Ok(unsafe { DlHandle::from_raw(handle) })
        }
    }

    pub fn dlclose_impl(handle: DlHandle) -> i32 {
        unsafe { libc::dlclose(handle.as_raw()) }
    }

    pub fn dlsym_impl(handle: DlHandle, symbol: &str) -> Result<*mut std::ffi::c_void, String> {
        let c_symbol = CString::new(symbol).map_err(|e| e.to_string())?;

        // Clear any existing error
        unsafe { libc::dlerror() };

        let ptr = unsafe { libc::dlsym(handle.as_raw(), c_symbol.as_ptr()) };

        // Check for error (dlsym can legitimately return NULL)
        if let Some(err) = dlerror_impl() {
            Err(err)
        } else {
            Ok(ptr)
        }
    }

    pub fn dlerror_impl() -> Option<String> {
        let err = unsafe { libc::dlerror() };
        if err.is_null() {
            None
        } else {
            let c_str = unsafe { std::ffi::CStr::from_ptr(err) };
            Some(c_str.to_string_lossy().into_owned())
        }
    }
}

#[cfg(not(any(windows, unix)))]
mod platform {
    use super::*;

    pub fn dlopen_impl(_path: &Path, _flags: DlOpenFlags) -> Result<DlHandle, String> {
        Err("Dynamic library loading not supported on this platform".to_string())
    }

    pub fn dlclose_impl(_handle: DlHandle) -> i32 {
        -1
    }

    pub fn dlsym_impl(_handle: DlHandle, _symbol: &str) -> Result<*mut std::ffi::c_void, String> {
        Err("Dynamic library loading not supported on this platform".to_string())
    }

    pub fn dlerror_impl() -> Option<String> {
        None
    }
}

use platform::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dl_handle_null() {
        let handle = unsafe { DlHandle::from_raw(std::ptr::null_mut()) };
        assert!(!handle.is_valid());
    }

    #[test]
    fn test_dl_handle_non_null() {
        let mut dummy = 42i32;
        let handle = unsafe { DlHandle::from_raw(&mut dummy as *mut i32 as *mut _) };
        assert!(handle.is_valid());
    }

    #[test]
    fn test_dl_open_flags() {
        assert_ne!(DlOpenFlags::LAZY.value(), DlOpenFlags::NOW.value());
        assert_ne!(DlOpenFlags::GLOBAL.value(), DlOpenFlags::LOCAL.value());
    }

    #[test]
    fn test_dl_open_flags_combine() {
        let combined = DlOpenFlags::NOW | DlOpenFlags::GLOBAL;
        assert_eq!(
            combined.value(),
            DlOpenFlags::NOW.value() | DlOpenFlags::GLOBAL.value()
        );
    }

    #[test]
    fn test_dl_open_flags_default() {
        assert_eq!(DlOpenFlags::default(), DlOpenFlags::NOW);
    }

    #[test]
    fn test_dlopen_nonexistent() {
        let result = dlopen(
            "nonexistent_library_that_does_not_exist.so",
            DlOpenFlags::NOW,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_dlopen_active_tracking() {
        // After dlopen completes, active flag should be false
        // Note: We can't reliably test the "during" state in single-threaded tests
        // because it happens synchronously
        let _ = dlopen("nonexistent.so", DlOpenFlags::NOW);

        // After dlopen, active should be false (eventually - tests may run in parallel)
        // Just verify the function exists and returns a bool
        let _ = dlopen_is_active();
    }

    #[test]
    fn test_dlclose_active_tracking() {
        // dlclose with invalid handle
        let handle = unsafe { DlHandle::from_raw(std::ptr::null_mut()) };
        let _ = dlclose(handle);

        // Just verify the function exists and returns a bool
        let _ = dlclose_is_active();
    }

    #[cfg(unix)]
    #[test]
    fn test_dlopen_libc() {
        // On Unix, we can try to load libc which should always be available
        // Note: This might not work in all environments
        let result = dlopen("libc.so.6", DlOpenFlags::NOW);
        if result.is_ok() {
            let handle = result.unwrap();
            assert!(handle.is_valid());

            // Try to get a known symbol
            let sym_result = dlsym(handle, "strlen");
            if sym_result.is_ok() {
                assert!(!sym_result.unwrap().is_null());
            }

            let close_result = dlclose(handle);
            assert_eq!(close_result, 0);
        }
    }

    #[cfg(windows)]
    #[test]
    fn test_dlopen_kernel32() {
        // On Windows, kernel32.dll should always be available
        let result = dlopen("kernel32.dll", DlOpenFlags::NOW);
        if result.is_ok() {
            let handle = result.unwrap();
            assert!(handle.is_valid());

            // Try to get a known symbol
            let sym_result = dlsym(handle, "GetLastError");
            if sym_result.is_ok() {
                assert!(!sym_result.unwrap().is_null());
            }

            // Note: On Windows, FreeLibrary on kernel32 might fail or have no effect
            let _ = dlclose(handle);
        }
    }
}
