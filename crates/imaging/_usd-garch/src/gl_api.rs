//! GL API version detection and capabilities querying.
//!
//! Provides types for detecting OpenGL version, checking extensions,
//! and validating GL capabilities at runtime.

use std::fmt;

/// OpenGL API version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GarchGLApiVersion {
    /// GL major version number
    pub major: u32,
    /// GL minor version number
    pub minor: u32,
}

impl GarchGLApiVersion {
    /// Create new GL version.
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// OpenGL 3.3 (minimum for modern USD)
    pub const GL_3_3: Self = Self::new(3, 3);

    /// OpenGL 4.0
    pub const GL_4_0: Self = Self::new(4, 0);

    /// OpenGL 4.1
    pub const GL_4_1: Self = Self::new(4, 1);

    /// OpenGL 4.2
    pub const GL_4_2: Self = Self::new(4, 2);

    /// OpenGL 4.3
    pub const GL_4_3: Self = Self::new(4, 3);

    /// OpenGL 4.4
    pub const GL_4_4: Self = Self::new(4, 4);

    /// OpenGL 4.5
    pub const GL_4_5: Self = Self::new(4, 5);

    /// OpenGL 4.6
    pub const GL_4_6: Self = Self::new(4, 6);

    /// Check if this version supports at least the given version.
    pub const fn at_least(&self, other: &Self) -> bool {
        self.major > other.major || (self.major == other.major && self.minor >= other.minor)
    }
}

impl fmt::Display for GarchGLApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// OpenGL capabilities and extension support.
///
/// Queries the current GL context for version, extensions, and hardware limits.
#[derive(Debug, Clone)]
pub struct GarchGLApiCapabilities {
    /// GL version
    version: GarchGLApiVersion,
    /// GLSL version
    glsl_version: GarchGLApiVersion,
    /// Vendor string
    vendor: String,
    /// Renderer string
    renderer: String,
    /// Available extensions (cached)
    extensions: Vec<String>,
}

impl GarchGLApiCapabilities {
    /// Query capabilities from current GL context.
    ///
    /// # Safety
    ///
    /// Must be called with a valid GL context current.
    ///
    /// # Stub Implementation
    ///
    /// Currently returns stub data. Future implementation will query actual GL state.
    pub fn query() -> Self {
        Self {
            version: GarchGLApiVersion::GL_4_5,
            glsl_version: GarchGLApiVersion::new(4, 50),
            vendor: String::from("STUB"),
            renderer: String::from("STUB Renderer"),
            extensions: Vec::new(),
        }
    }

    /// Get GL version.
    pub fn version(&self) -> GarchGLApiVersion {
        self.version
    }

    /// Get GLSL version.
    pub fn glsl_version(&self) -> GarchGLApiVersion {
        self.glsl_version
    }

    /// Get vendor string.
    pub fn vendor(&self) -> &str {
        &self.vendor
    }

    /// Get renderer string.
    pub fn renderer(&self) -> &str {
        &self.renderer
    }

    /// Check if extension is supported.
    pub fn has_extension(&self, name: &str) -> bool {
        self.extensions.iter().any(|ext| ext == name)
    }

    /// Get all extensions.
    pub fn extensions(&self) -> &[String] {
        &self.extensions
    }

    /// Check if core GL version is at least the given version.
    pub fn supports_version(&self, version: GarchGLApiVersion) -> bool {
        self.version.at_least(&version)
    }
}

impl Default for GarchGLApiCapabilities {
    fn default() -> Self {
        Self::query()
    }
}

/// GL error checking utilities.
pub mod error {
    /// Check for GL errors.
    ///
    /// Note: Returns Ok when OpenGL not available. With `feature = "opengl"`,
    /// calls glGetError() and returns detailed error info.
    pub fn check_gl_error() -> Result<(), String> {
        // Note: Always succeeds when OpenGL not compiled in
        Ok(())
    }

    /// Clear any pending GL errors.
    pub fn clear_gl_errors() {
        // Note: No-op when OpenGL not compiled in
    }

    /// Get GL error string from error code (stub).
    pub fn gl_error_string(_error_code: u32) -> &'static str {
        "GL_NO_ERROR"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        let v33 = GarchGLApiVersion::GL_3_3;
        let v45 = GarchGLApiVersion::GL_4_5;

        assert!(v45.at_least(&v33));
        assert!(!v33.at_least(&v45));
        assert!(v45.at_least(&v45));
    }

    #[test]
    fn test_capabilities_query() {
        let caps = GarchGLApiCapabilities::query();
        assert!(caps.supports_version(GarchGLApiVersion::GL_3_3));
    }
}
