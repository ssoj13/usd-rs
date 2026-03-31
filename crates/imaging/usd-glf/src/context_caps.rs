//! GL context capabilities cache.
//!
//! Port of pxr/imaging/glf/contextCaps.h
//!
//! Caches resource limits and features of the underlying GL context
//! to reduce driver query overhead and allow cross-thread access.

use std::sync::OnceLock;

/// Cached GL context capabilities.
///
/// Singleton that stores queried GL context capabilities. Provides reasonable
/// defaults based on GL minimums when the context hasn't been initialized.
///
/// Storm and other Hydra backends reference this for feature detection.
#[derive(Debug, Clone)]
pub struct GlfContextCaps {
    /// GL version as integer (e.g. 400 for 4.0, 410 for 4.1)
    pub gl_version: i32,
    /// Whether running with core profile
    pub core_profile: bool,
    /// Maximum number of array texture layers
    pub max_array_texture_layers: i32,
}

/// Global singleton instance
static INSTANCE: OnceLock<GlfContextCaps> = OnceLock::new();

impl GlfContextCaps {
    /// Create with uninitialized sentinel defaults.
    ///
    /// Matches C++ constructor: `glVersion(0)` is the sentinel that
    /// `get_instance()` uses to detect an uninitialized context.
    /// `max_array_texture_layers` defaults to GL spec minimum of 256.
    fn new() -> Self {
        Self {
            // C++: glVersion(0) — sentinel for "not initialized yet"
            gl_version: 0,
            core_profile: false,
            max_array_texture_layers: 256,
        }
    }

    /// Initialize the singleton by querying the current GL context.
    ///
    /// Should be called by the application after GL context creation,
    /// before using systems that depend on caps (e.g. Hydra).
    pub fn init_instance() {
        let mut caps = Self::new();
        caps.load_caps();
        let _ = INSTANCE.set(caps);
    }

    /// Get the initialized capabilities instance.
    ///
    /// Matches C++ `GetInstance()`: warns if `glVersion == 0` (not initialized).
    /// Returns defaults if `init_instance()` has not been called yet.
    pub fn get_instance() -> &'static GlfContextCaps {
        let caps = INSTANCE.get_or_init(Self::new);
        if caps.gl_version == 0 {
            // C++: TF_CODING_ERROR("GlfContextCaps has not been initialized")
            log::warn!("GlfContextCaps: not initialized — call init_instance() first");
        }
        caps
    }

    /// Query GL context for actual capabilities.
    ///
    /// Version guards match C++ `_LoadCaps()` to avoid GL errors on old contexts:
    /// - profile mask only available on GL 3.2+
    /// - max array texture layers only available on GL 3.0+
    #[cfg(feature = "opengl")]
    fn load_caps(&mut self) {
        unsafe {
            // Query GL version as integer (major*100 + minor*10)
            let mut major: i32 = 0;
            let mut minor: i32 = 0;
            gl::GetIntegerv(gl::MAJOR_VERSION, &mut major);
            gl::GetIntegerv(gl::MINOR_VERSION, &mut minor);
            self.gl_version = major * 100 + minor * 10;

            // Profile mask only available on GL 3.2+
            if self.gl_version >= 320 {
                let mut profile_mask: i32 = 0;
                gl::GetIntegerv(gl::CONTEXT_PROFILE_MASK, &mut profile_mask);
                self.core_profile = (profile_mask & gl::CONTEXT_CORE_PROFILE_BIT as i32) != 0;
            }

            // Max array texture layers only available on GL 3.0+
            if self.gl_version >= 300 {
                gl::GetIntegerv(
                    gl::MAX_ARRAY_TEXTURE_LAYERS,
                    &mut self.max_array_texture_layers,
                );
            }
        }
    }

    /// Stub when OpenGL feature is not enabled.
    #[cfg(not(feature = "opengl"))]
    fn load_caps(&mut self) {
        log::debug!("GlfContextCaps: OpenGL feature not enabled, using defaults");
    }
}

impl Default for GlfContextCaps {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_caps() {
        let caps = GlfContextCaps::default();
        // C++ default: glVersion=0 (sentinel for uninitialized)
        assert_eq!(caps.gl_version, 0);
        assert!(!caps.core_profile);
        assert_eq!(caps.max_array_texture_layers, 256);
    }

    #[test]
    fn test_singleton_returns_instance() {
        // get_instance() should always return a valid pointer (may be uninitialized)
        let caps = GlfContextCaps::get_instance();
        // gl_version may be 0 if init_instance() was not called (e.g. in unit test environment)
        assert!(caps.gl_version >= 0);
        assert!(caps.max_array_texture_layers >= 256);
    }
}
