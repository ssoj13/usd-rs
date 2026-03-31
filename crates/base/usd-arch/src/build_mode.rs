// Rust port

//! Build mode detection and compile-time configuration.
//!
//! This module provides types and functions for detecting the build mode
//! (debug vs release) at compile-time and runtime, enabling conditional
//! compilation and behavior based on the build configuration.

/// Build mode enumeration representing the CMake build types.
///
/// Corresponds to CMAKE_BUILD_TYPE values:
/// - Debug: No optimization, debug symbols, debug assertions enabled
/// - Release: Full optimization, no debug symbols
/// - RelWithDebInfo: Optimizations with debug symbols
/// - MinSizeRel: Optimize for size
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ArchBuildMode {
    /// Debug build: no optimizations, debug assertions enabled
    Debug = 0,
    /// Release build: full optimizations, no debug info
    Release = 1,
    /// Release with debug info: optimizations + debug symbols
    RelWithDebInfo = 2,
    /// Minimum size release: size-optimized build
    MinSizeRel = 3,
}

impl ArchBuildMode {
    /// Get the build mode as a static string.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "Debug",
            Self::Release => "Release",
            Self::RelWithDebInfo => "RelWithDebInfo",
            Self::MinSizeRel => "MinSizeRel",
        }
    }

    /// Check if this is a debug build mode.
    #[inline]
    pub const fn is_debug(self) -> bool {
        matches!(self, Self::Debug)
    }

    /// Check if this is any release variant.
    #[inline]
    pub const fn is_release(self) -> bool {
        !self.is_debug()
    }
}

impl Default for ArchBuildMode {
    #[inline]
    fn default() -> Self {
        arch_build_mode()
    }
}

impl std::fmt::Display for ArchBuildMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Get the current build mode at compile-time.
///
/// Uses Rust's `cfg!()` macro to detect the build configuration:
/// - `debug_assertions` indicates Debug mode
/// - Otherwise assumes Release mode
///
/// Note: Rust doesn't distinguish between RelWithDebInfo and MinSizeRel
/// at the standard library level, so we map them both to Release.
///
/// # Returns
/// The detected build mode
///
/// # Examples
/// ```
/// use usd_arch::arch_build_mode;
///
/// let mode = arch_build_mode();
/// println!("Build mode: {}", mode);
/// ```
#[inline]
pub const fn arch_build_mode() -> ArchBuildMode {
    if cfg!(debug_assertions) {
        ArchBuildMode::Debug
    } else {
        // In standard Rust, we can't distinguish between release variants
        // without custom build scripts or env vars
        ArchBuildMode::Release
    }
}

/// Check if this is a debug build.
///
/// Equivalent to checking `cfg!(debug_assertions)`.
///
/// # Returns
/// `true` if compiled in debug mode, `false` otherwise
///
/// # Examples
/// ```
/// use usd_arch::arch_is_debug_build;
///
/// if arch_is_debug_build() {
///     println!("Debug checks enabled");
/// }
/// ```
#[inline]
pub const fn arch_is_debug_build() -> bool {
    cfg!(debug_assertions)
}

/// Check if this is a development build.
///
/// Development builds include:
/// - Debug builds (debug_assertions enabled)
/// - Builds with custom DEV_BUILD feature flag
///
/// This corresponds to ARCH_DEV_BUILD from the C++ implementation.
///
/// # Returns
/// `true` if this is a development build, `false` otherwise
///
/// # Examples
/// ```
/// use usd_arch::arch_is_dev_build;
///
/// if arch_is_dev_build() {
///     println!("Development mode");
/// }
/// ```
#[inline]
pub const fn arch_is_dev_build() -> bool {
    cfg!(debug_assertions) || cfg!(feature = "dev_build")
}

/// Compile-time constant for development build detection.
///
/// Corresponds to ARCH_DEV_BUILD macro from C++.
pub const ARCH_DEV_BUILD: bool = arch_is_dev_build();

/// Conditional compilation macro for debug builds.
///
/// Executes the provided code block only in debug builds (when `debug_assertions` is enabled).
///
/// # Examples
/// ```
/// use usd_arch::arch_if_debug;
///
/// arch_if_debug! {
///     println!("This only prints in debug builds");
/// }
/// ```
#[macro_export]
macro_rules! arch_if_debug {
    ($($tt:tt)*) => {
        #[cfg(debug_assertions)]
        {
            $($tt)*
        }
    };
}

/// Conditional compilation macro for release builds.
///
/// Executes the provided code block only in release builds (when `debug_assertions` is disabled).
///
/// # Examples
/// ```
/// use usd_arch::arch_if_release;
///
/// arch_if_release! {
///     println!("This only prints in release builds");
/// }
/// ```
#[macro_export]
macro_rules! arch_if_release {
    ($($tt:tt)*) => {
        #[cfg(not(debug_assertions))]
        {
            $($tt)*
        }
    };
}

// Re-export macros at crate level
pub use arch_if_debug;
pub use arch_if_release;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_mode_detection() {
        let mode = arch_build_mode();
        
        #[cfg(debug_assertions)]
        assert_eq!(mode, ArchBuildMode::Debug);
        
        #[cfg(not(debug_assertions))]
        assert!(mode.is_release());
    }

    #[test]
    fn test_debug_build_check() {
        let is_debug = arch_is_debug_build();
        
        #[cfg(debug_assertions)]
        assert!(is_debug);
        
        #[cfg(not(debug_assertions))]
        assert!(!is_debug);
    }

    #[test]
    fn test_dev_build_check() {
        let is_dev = arch_is_dev_build();
        
        // DEV_BUILD is true if debug_assertions OR dev_build feature is enabled
        #[cfg(debug_assertions)]
        assert!(is_dev);
        
        #[cfg(all(not(debug_assertions), not(feature = "dev_build")))]
        assert!(!is_dev);
    }

    #[test]
    fn test_build_mode_const() {
        assert!(ARCH_DEV_BUILD == arch_is_dev_build());
    }

    #[test]
    fn test_build_mode_display() {
        assert_eq!(ArchBuildMode::Debug.to_string(), "Debug");
        assert_eq!(ArchBuildMode::Release.to_string(), "Release");
        assert_eq!(ArchBuildMode::RelWithDebInfo.to_string(), "RelWithDebInfo");
        assert_eq!(ArchBuildMode::MinSizeRel.to_string(), "MinSizeRel");
    }

    #[test]
    fn test_build_mode_as_str() {
        assert_eq!(ArchBuildMode::Debug.as_str(), "Debug");
        assert_eq!(ArchBuildMode::Release.as_str(), "Release");
    }

    #[test]
    fn test_build_mode_checks() {
        assert!(ArchBuildMode::Debug.is_debug());
        assert!(!ArchBuildMode::Debug.is_release());
        
        assert!(!ArchBuildMode::Release.is_debug());
        assert!(ArchBuildMode::Release.is_release());
        
        assert!(ArchBuildMode::RelWithDebInfo.is_release());
        assert!(ArchBuildMode::MinSizeRel.is_release());
    }

    #[test]
    fn test_build_mode_default() {
        let default_mode = ArchBuildMode::default();
        assert_eq!(default_mode, arch_build_mode());
    }

    #[test]
    fn test_conditional_macros() {
        #[cfg(debug_assertions)]
        {
            let mut ran = false;
            arch_if_debug! {
                ran = true;
            }
            assert!(ran, "Debug code should run in debug build");
        }

        #[cfg(not(debug_assertions))]
        {
            let mut ran = false;
            arch_if_release! {
                ran = true;
            }
            assert!(ran, "Release code should run in release build");
        }
    }

    #[test]
    fn test_build_mode_equality() {
        assert_eq!(ArchBuildMode::Debug, ArchBuildMode::Debug);
        assert_ne!(ArchBuildMode::Debug, ArchBuildMode::Release);
    }

    #[test]
    fn test_build_mode_clone_copy() {
        let mode1 = ArchBuildMode::Debug;
        let mode2 = mode1; // Copy
        let mode3 = mode1.clone(); // Clone
        
        assert_eq!(mode1, mode2);
        assert_eq!(mode1, mode3);
    }
}
