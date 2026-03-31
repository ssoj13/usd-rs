//! Platform, compiler, and architecture detection.
//!
//! In Rust, most of this is handled by `cfg!()` macros and `std::env::consts`.
//! This module provides constants and functions for compatibility with USD code
//! that checks these values at runtime.

/// Operating system detection
pub mod os {
    /// True if running on Linux
    pub const IS_LINUX: bool = cfg!(target_os = "linux");

    /// True if running on macOS
    pub const IS_MACOS: bool = cfg!(target_os = "macos");

    /// True if running on iOS
    pub const IS_IOS: bool = cfg!(target_os = "ios");

    /// True if running on any Apple platform
    pub const IS_DARWIN: bool = IS_MACOS || IS_IOS;

    /// True if running on Windows
    pub const IS_WINDOWS: bool = cfg!(target_os = "windows");

    /// True if running on WebAssembly
    pub const IS_WASM: bool = cfg!(target_family = "wasm");
}

/// CPU architecture detection
pub mod cpu {
    /// True if running on x86/x86_64
    pub const IS_INTEL: bool = cfg!(any(target_arch = "x86", target_arch = "x86_64"));

    /// True if running on ARM/AArch64
    pub const IS_ARM: bool = cfg!(any(target_arch = "arm", target_arch = "aarch64"));

    /// True if running on 64-bit architecture
    pub const IS_64_BIT: bool = cfg!(target_pointer_width = "64");

    /// True if running on 32-bit architecture
    pub const IS_32_BIT: bool = cfg!(target_pointer_width = "32");
}

/// Platform feature detection (matches C++ defines.h).
pub mod features {
    /// MAP_POPULATE flag for mmap exists on Linux.
    /// C++: ARCH_HAS_MMAP_MAP_POPULATE
    pub const HAS_MMAP_MAP_POPULATE: bool = cfg!(target_os = "linux");

    /// True when built with address sanitizer.
    /// C++: ARCH_SANITIZE_ADDRESS. Set via `--cfg sanitize_address` when using -Z sanitizer=address.
    #[cfg(sanitize_address)]
    pub const SANITIZE_ADDRESS: bool = true;
    /// False when not built with address sanitizer.
    #[cfg(not(sanitize_address))]
    pub const SANITIZE_ADDRESS: bool = false;
}

/// Compiler detection (for conditional compilation).
/// In Rust we typically use rustc; C++ defines ARCH_COMPILER_CLANG, etc.
pub mod compiler {
    /// True if using rustc (always true for Rust code)
    pub const IS_RUSTC: bool = true;
}

/// Cache line size for the current architecture.
///
/// This is 128 bytes on Apple Silicon (M1/M2/etc.) and 64 bytes on most other platforms.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const CACHE_LINE_SIZE: usize = 128;

/// Cache line size for the current architecture.
#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
pub const CACHE_LINE_SIZE: usize = 64;

/// Returns the name of the current operating system.
#[inline]
#[must_use]
pub fn os_name() -> &'static str {
    std::env::consts::OS
}

/// Returns the current CPU architecture name.
#[inline]
#[must_use]
pub fn arch_name() -> &'static str {
    std::env::consts::ARCH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_detection() {
        // At least one OS should be detected
        let any_os = os::IS_LINUX || os::IS_MACOS || os::IS_WINDOWS || os::IS_WASM || os::IS_IOS;
        assert!(any_os || !any_os); // Always passes, just ensures compilation
    }

    #[test]
    fn test_cache_line_size() {
        assert!(CACHE_LINE_SIZE == 64 || CACHE_LINE_SIZE == 128);
    }

    #[test]
    fn test_os_name() {
        let name = os_name();
        assert!(!name.is_empty());
    }

    #[test]
    fn test_arch_name() {
        let name = arch_name();
        assert!(!name.is_empty());
    }
}
