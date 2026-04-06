// SAFETY: This module provides FFI bindings to system APIs requiring unsafe
#![allow(unsafe_code)]

//! Dynamic library loading and symbol resolution.
//!
//! Provides cross-platform abstractions for loading shared libraries (.so/.dylib/.dll)
//! and resolving symbols from them.
//!
//! # Examples
//!
//! ```ignore
//! use usd_arch::{Library, LIBRARY_SUFFIX, LibraryFlags};
//!
//! // Load a library
//! let lib = Library::open(&format!("libfoo{}", LIBRARY_SUFFIX), LibraryFlags::NOW).unwrap();
//!
//! // Get a symbol
//! unsafe {
//!     let func: libloading::Symbol<unsafe extern "C" fn() -> i32> =
//!         lib.get_symbol(b"my_function\0").unwrap();
//!     let result = func();
//! }
//! ```

use std::ffi::{CStr, OsStr};
use std::path::Path;

pub use libloading::Library as RawLibrary;

/// Platform-specific library file suffix.
///
/// - Windows: `.dll`
/// - macOS: `.dylib`
/// - Linux/Unix: `.so`
#[cfg(target_os = "windows")]
pub const LIBRARY_SUFFIX: &str = ".dll";

#[cfg(target_os = "macos")]
pub const LIBRARY_SUFFIX: &str = ".dylib";

#[cfg(all(unix, not(target_os = "macos")))]
pub const LIBRARY_SUFFIX: &str = ".so";

#[cfg(not(any(unix, windows)))]
pub const LIBRARY_SUFFIX: &str = ".so";

/// Platform-specific static library file suffix.
///
/// - Windows: `.lib`
/// - Unix/Linux/macOS: `.a`
#[cfg(target_os = "windows")]
pub const STATIC_LIBRARY_SUFFIX: &str = ".lib";

#[cfg(not(target_os = "windows"))]
pub const STATIC_LIBRARY_SUFFIX: &str = ".a";

/// Platform-specific plugin file suffix.
///
/// On macOS, plugins typically use `.so` for cross-platform compatibility,
/// even though native loadable bundles use `.bundle`.
#[cfg(target_os = "macos")]
pub const PLUGIN_SUFFIX: &str = ".so";

#[cfg(not(target_os = "macos"))]
/// Suffix for plugin libraries (same as LIBRARY_SUFFIX).
pub const PLUGIN_SUFFIX: &str = LIBRARY_SUFFIX;

/// Library loading flags.
///
/// On Windows, these flags are currently unused (set to 0).
/// On Unix, they map to RTLD_* constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LibraryFlags(u32);

impl Default for LibraryFlags {
    fn default() -> Self {
        // Default: LAZY | LOCAL (matches typical dlopen defaults)
        Self::LAZY.or(Self::LOCAL)
    }
}

impl std::ops::BitOr for LibraryFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl LibraryFlags {
    /// Check if flags contain the specified flag.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl LibraryFlags {
    /// Lazy symbol resolution (Unix: RTLD_LAZY, Windows: 0)
    #[cfg(unix)]
    pub const LAZY: Self = Self(libc::RTLD_LAZY as u32);

    #[cfg(not(unix))]
    /// Lazy symbol resolution (resolve symbols only when used).
    pub const LAZY: Self = Self(0);

    /// Immediate symbol resolution (Unix: RTLD_NOW, Windows: 0)
    #[cfg(unix)]
    pub const NOW: Self = Self(libc::RTLD_NOW as u32);

    #[cfg(not(unix))]
    /// Immediate symbol resolution (resolve all symbols at load time).
    pub const NOW: Self = Self(0);

    /// Local symbol scope (Unix: RTLD_LOCAL, Windows: 0)
    #[cfg(unix)]
    pub const LOCAL: Self = Self(libc::RTLD_LOCAL as u32);

    #[cfg(not(unix))]
    /// Local symbol scope (symbols not visible to other libraries).
    pub const LOCAL: Self = Self(0);

    /// Global symbol scope (Unix: RTLD_GLOBAL, Windows: 0)
    #[cfg(unix)]
    pub const GLOBAL: Self = Self(libc::RTLD_GLOBAL as u32);

    #[cfg(not(unix))]
    /// Global symbol scope (symbols visible to other libraries).
    pub const GLOBAL: Self = Self(0);

    /// Combine flags with bitwise OR
    #[must_use]
    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Get the raw flag value
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

/// A loaded dynamic library.
///
/// This is a thin wrapper around `libloading::Library` providing a USD-compatible API.
pub struct Library {
    inner: RawLibrary,
}

impl Library {
    /// Opens a dynamic library.
    ///
    /// # Arguments
    ///
    /// * `filename` - Path to the library file
    /// * `flags` - Loading flags (note: currently ignored, using libloading defaults)
    ///
    /// # Errors
    ///
    /// Returns an error if the library cannot be loaded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_arch::{Library, LibraryFlags};
    ///
    /// let lib = Library::open("libfoo.so", LibraryFlags::NOW)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn open<P: AsRef<OsStr>>(
        filename: P,
        flags: LibraryFlags,
    ) -> Result<Self, libloading::Error> {
        #[cfg(unix)]
        {
            // Use libloading's unix Library::open to honor caller's RTLD_* flags
            let unix_lib = unsafe {
                libloading::os::unix::Library::open(
                    Some(filename),
                    flags.bits() as std::os::raw::c_int,
                )?
            };
            // Convert os::unix::Library -> libloading::Library via From impl
            Ok(Self {
                inner: unix_lib.into(),
            })
        }
        #[cfg(not(unix))]
        {
            let _ = flags; // Windows: libloading defaults are fine
            let inner = unsafe { RawLibrary::new(filename)? };
            Ok(Self { inner })
        }
    }

    /// Gets a pointer to a symbol in the library.
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - The symbol name is valid and null-terminated
    /// - The symbol is used with the correct type
    /// - The symbol remains valid for the lifetime of the Symbol
    ///
    /// # Errors
    ///
    /// Returns an error if the symbol cannot be found.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_arch::Library;
    ///
    /// # let lib = unsafe { libloading::Library::new("libfoo.so")? };
    /// # let lib = usd_arch::Library::from_raw(lib);
    /// unsafe {
    ///     let func: libloading::Symbol<unsafe extern "C" fn() -> i32> =
    ///         lib.get_symbol(b"my_function\0")?;
    ///     let result = func();
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub unsafe fn get_symbol<'lib, T>(
        &'lib self,
        symbol: &[u8],
    ) -> Result<libloading::Symbol<'lib, T>, libloading::Error> {
        unsafe { self.inner.get(symbol) }
    }

    /// Gets a pointer to a symbol by name string.
    ///
    /// This is a convenience method that automatically adds null termination.
    ///
    /// # Safety
    ///
    /// Same safety requirements as `get_symbol`.
    ///
    /// # Errors
    ///
    /// Returns an error if the symbol cannot be found.
    pub unsafe fn get_symbol_str<'lib, T>(
        &'lib self,
        symbol: &str,
    ) -> Result<libloading::Symbol<'lib, T>, libloading::Error> {
        unsafe {
            let mut bytes = symbol.as_bytes().to_vec();
            bytes.push(0);
            self.get_symbol(&bytes)
        }
    }

    /// Gets a raw pointer to a symbol.
    ///
    /// Unlike `get_symbol`, this returns a raw `*mut ()` pointer rather than
    /// a typed `Symbol`. This matches the C++ API more closely.
    ///
    /// # Safety
    ///
    /// The caller must ensure the symbol name is valid and null-terminated.
    ///
    /// # Errors
    ///
    /// Returns `None` if the symbol cannot be found.
    pub unsafe fn get_symbol_address(&self, name: &CStr) -> Option<*mut ()> {
        unsafe {
            match self.inner.get::<*mut ()>(name.to_bytes_with_nul()) {
                Ok(symbol) => Some(*symbol),
                Err(_) => None,
            }
        }
    }

    /// Creates a Library from a raw `libloading::Library`.
    #[must_use]
    pub const fn from_raw(inner: RawLibrary) -> Self {
        Self { inner }
    }

    /// Consumes this Library and returns the inner `libloading::Library`.
    #[must_use]
    pub fn into_raw(self) -> RawLibrary {
        self.inner
    }

    /// Gets a reference to the inner `libloading::Library`.
    #[must_use]
    pub const fn as_raw(&self) -> &RawLibrary {
        &self.inner
    }
}

/// Opens a dynamic library (C-style API).
///
/// Returns a raw pointer to the library handle on success, null on failure.
/// Use `library_error()` to get error information.
///
/// # Safety
///
/// The caller must ensure:
/// - The filename is a valid null-terminated C string
/// - The returned handle is eventually closed with `library_close`
///
/// # Examples
///
/// ```no_run
/// use std::ffi::CString;
/// use usd_arch::{library_open, library_close, LibraryFlags};
///
/// unsafe {
///     let name = CString::new("libfoo.so").unwrap();
///     let handle = library_open(&name, LibraryFlags::NOW);
///     if !handle.is_null() {
///         // Use handle...
///         library_close(handle);
///     }
/// }
/// ```
#[must_use]
pub unsafe fn library_open(filename: &CStr, flags: LibraryFlags) -> *mut () {
    unsafe {
        #[cfg(unix)]
        {
            // Clear stale error before dlopen (C++ library.cpp:40)
            libc::dlerror();
            let ptr = libc::dlopen(filename.as_ptr(), flags.bits() as i32);
            ptr as *mut ()
        }

        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            let _ = flags; // Unused on Windows

            // Convert to wide string for Windows
            let wide: Vec<u16> = OsStr::new(filename.to_str().unwrap_or(""))
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let handle = windows_sys::Win32::System::LibraryLoader::LoadLibraryW(wide.as_ptr());
            handle as *mut ()
        }

        #[cfg(not(any(unix, windows)))]
        {
            let _ = (filename, flags);
            std::ptr::null_mut()
        }
    }
}

/// Returns a description of the most recent error from library operations.
///
/// # Examples
///
/// ```no_run
/// use std::ffi::CString;
/// use usd_arch::{library_open, library_error, LibraryFlags};
///
/// unsafe {
///     let name = CString::new("nonexistent.so").unwrap();
///     let handle = library_open(&name, LibraryFlags::NOW);
///     if handle.is_null() {
///         let error = library_error();
///         println!("Failed to load: {}", error);
///     }
/// }
/// ```
#[must_use]
pub fn library_error() -> String {
    #[cfg(unix)]
    {
        unsafe {
            let err_ptr = libc::dlerror();
            if err_ptr.is_null() {
                // C++ returns empty string when dlerror is null
                return String::new();
            }
            CStr::from_ptr(err_ptr).to_string_lossy().into_owned()
        }
    }

    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::GetLastError;
        use windows_sys::Win32::System::Diagnostics::Debug::{
            FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS, FormatMessageW,
        };

        unsafe {
            let error_code = GetLastError();
            let mut buffer: Vec<u16> = vec![0; 512];

            let len = FormatMessageW(
                FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
                std::ptr::null(),
                error_code,
                0,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                std::ptr::null(),
            );

            if len > 0 {
                buffer.truncate(len as usize);
                String::from_utf16_lossy(&buffer)
            } else {
                format!("Error code: {}", error_code)
            }
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        String::from("Library operations not supported")
    }
}

/// Closes a dynamic library handle.
///
/// # Safety
///
/// The caller must ensure:
/// - The handle was returned from `library_open`
/// - The handle has not been closed already
/// - No symbols from this library are still in use
///
/// # Returns
///
/// Returns 0 on success, non-zero on failure.
///
/// # Examples
///
/// ```no_run
/// use std::ffi::CString;
/// use usd_arch::{library_open, library_close, LibraryFlags};
///
/// unsafe {
///     let name = CString::new("libfoo.so").unwrap();
///     let handle = library_open(&name, LibraryFlags::NOW);
///     if !handle.is_null() {
///         library_close(handle);
///     }
/// }
/// ```
pub unsafe fn library_close(handle: *mut ()) -> i32 {
    unsafe {
        #[cfg(unix)]
        {
            libc::dlclose(handle as *mut _)
        }

        #[cfg(windows)]
        {
            use windows_sys::Win32::Foundation::FreeLibrary;

            let handle_ptr = handle as *mut std::ffi::c_void;
            let result = FreeLibrary(handle_ptr as _);
            if result != 0 {
                0 // Success
            } else {
                -1 // Failure
            }
        }

        #[cfg(not(any(unix, windows)))]
        {
            let _ = handle;
            -1
        }
    }
}

/// Gets the address of a symbol in a loaded library.
///
/// # Safety
///
/// The caller must ensure:
/// - The handle is valid and was returned from `library_open`
/// - The name is a valid null-terminated C string
/// - The returned pointer is cast to the correct type before use
///
/// # Returns
///
/// Returns a pointer to the symbol on success, null on failure.
///
/// # Examples
///
/// ```no_run
/// use std::ffi::CString;
/// use usd_arch::{library_open, library_get_symbol_address, library_close, LibraryFlags};
///
/// unsafe {
///     let lib_name = CString::new("libfoo.so").unwrap();
///     let handle = library_open(&lib_name, LibraryFlags::NOW);
///     
///     if !handle.is_null() {
///         let sym_name = CString::new("my_function").unwrap();
///         let sym_ptr = library_get_symbol_address(handle, &sym_name);
///         
///         if !sym_ptr.is_null() {
///             let func: extern "C" fn() -> i32 = std::mem::transmute(sym_ptr);
///             let result = func();
///         }
///         
///         library_close(handle);
///     }
/// }
/// ```
#[must_use]
pub unsafe fn library_get_symbol_address(handle: *mut (), name: &CStr) -> *mut () {
    unsafe {
        #[cfg(unix)]
        {
            libc::dlsym(handle as *mut _, name.as_ptr()) as *mut ()
        }

        #[cfg(windows)]
        {
            use windows_sys::Win32::System::LibraryLoader::GetProcAddress;

            // GetProcAddress expects ANSI string, not wide
            let name_bytes = name.to_bytes();
            let handle_ptr = handle as *mut std::ffi::c_void;
            let result = GetProcAddress(handle_ptr as _, name_bytes.as_ptr());
            result.map(|f| f as *mut ()).unwrap_or(std::ptr::null_mut())
        }

        #[cfg(not(any(unix, windows)))]
        {
            let _ = (handle, name);
            std::ptr::null_mut()
        }
    }
}

/// Checks if a library file exists at the given path.
///
/// This is a convenience function for plugin systems that need to probe
/// for library files.
pub fn library_exists<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().exists()
}

// =============================================================================
// C++ API compatibility aliases (matching OpenUSD naming)
// =============================================================================

/// Type alias for `Library` (C++ compatibility).
pub type ArchLibrary = Library;

/// Library file extension without dot (e.g., "dll", "so", "dylib").
#[cfg(target_os = "windows")]
pub const LIBRARY_EXTENSION: &str = "dll";

#[cfg(target_os = "macos")]
pub const LIBRARY_EXTENSION: &str = "dylib";

#[cfg(all(unix, not(target_os = "macos")))]
pub const LIBRARY_EXTENSION: &str = "so";

#[cfg(not(any(unix, windows)))]
pub const LIBRARY_EXTENSION: &str = "so";

/// Library file prefix (platform-specific).
#[cfg(windows)]
pub const LIBRARY_PREFIX: &str = "";

#[cfg(not(windows))]
pub const LIBRARY_PREFIX: &str = "lib";

/// Opens a dynamic library (C++ API compatible).
///
/// # Safety
/// Same as `library_open`.
pub unsafe fn arch_library_open(filename: &CStr, flags: LibraryFlags) -> *mut () {
    unsafe { library_open(filename, flags) }
}

/// Returns error description (C++ API compatible).
#[must_use]
pub fn arch_library_error() -> String {
    library_error()
}

/// Closes library handle (C++ API compatible).
///
/// # Safety
/// Same as `library_close`.
pub unsafe fn arch_library_close(handle: *mut ()) -> i32 {
    unsafe { library_close(handle) }
}

/// Gets symbol address (C++ API compatible).
///
/// # Safety
/// Same as `library_get_symbol_address`.
#[must_use]
pub unsafe fn arch_library_get_symbol(handle: *mut (), name: &CStr) -> *mut () {
    unsafe { library_get_symbol_address(handle, name) }
}

/// Constructs platform-specific library filename.
///
/// # Examples
/// ```
/// use usd_arch::make_library_name;
///
/// let name = make_library_name("foo");
/// #[cfg(windows)]
/// assert_eq!(name, "foo.dll");
/// #[cfg(target_os = "macos")]
/// assert_eq!(name, "libfoo.dylib");
/// #[cfg(all(unix, not(target_os = "macos")))]
/// assert_eq!(name, "libfoo.so");
/// ```
#[must_use]
pub fn make_library_name(base_name: &str) -> String {
    format!("{}{}{}", LIBRARY_PREFIX, base_name, LIBRARY_SUFFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_suffix() {
        // Just verify the constants are set
        assert!(!LIBRARY_SUFFIX.is_empty());
        assert!(!STATIC_LIBRARY_SUFFIX.is_empty());
        assert!(!PLUGIN_SUFFIX.is_empty());

        #[cfg(windows)]
        {
            assert_eq!(LIBRARY_SUFFIX, ".dll");
            assert_eq!(STATIC_LIBRARY_SUFFIX, ".lib");
        }

        #[cfg(target_os = "macos")]
        {
            assert_eq!(LIBRARY_SUFFIX, ".dylib");
            assert_eq!(PLUGIN_SUFFIX, ".so"); // Cross-platform plugins use .so
            assert_eq!(STATIC_LIBRARY_SUFFIX, ".a");
        }

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            assert_eq!(LIBRARY_SUFFIX, ".so");
            assert_eq!(PLUGIN_SUFFIX, ".so");
            assert_eq!(STATIC_LIBRARY_SUFFIX, ".a");
        }
    }

    #[test]
    fn test_library_flags() {
        let flags = LibraryFlags::NOW.or(LibraryFlags::GLOBAL);

        #[cfg(unix)]
        {
            assert_eq!(flags.bits(), (libc::RTLD_NOW | libc::RTLD_GLOBAL) as u32);
        }

        #[cfg(not(unix))]
        {
            assert_eq!(flags.bits(), 0);
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_library_open_close_libc() {
        use std::ffi::CString;

        unsafe {
            // Try to load libc (should always exist on Unix)
            let name = CString::new("libc.so.6")
                .or_else(|_| CString::new("libc.so"))
                .unwrap();
            let handle = library_open(&name, LibraryFlags::NOW);

            if !handle.is_null() {
                // Try to get a symbol
                let sym = CString::new("printf").unwrap();
                let sym_ptr = library_get_symbol_address(handle, &sym);
                assert!(!sym_ptr.is_null());

                // NOTE: do NOT library_close libc — crashes process on
                // some Linux distros (SIGSEGV during cleanup).
                // C++ reference uses a dedicated test .so, not libc.
            }
        }
    }

    #[test]
    fn test_library_error() {
        // Just ensure it doesn't crash
        let error = library_error();
        // Error might be empty or contain a message, just check it's valid
        let _ = error.len();
    }

    #[test]
    fn test_library_open_nonexistent() {
        use std::ffi::CString;

        unsafe {
            let name = CString::new("this_library_definitely_does_not_exist_12345.so").unwrap();
            let handle = library_open(&name, LibraryFlags::NOW);
            assert!(handle.is_null());

            let error = library_error();
            assert!(!error.is_empty());
        }
    }

    #[test]
    fn test_library_exists() {
        // Test with a path that definitely doesn't exist
        assert!(!library_exists("/nonexistent/path/to/library.so"));

        // Test with current executable (should exist)
        if let Some(exe) = std::env::current_exe().ok() {
            assert!(library_exists(&exe));
        }
    }
}
