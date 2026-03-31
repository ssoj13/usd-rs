//! GL info queries.
//!
//! Port of pxr/imaging/glf/info.h

#[cfg(feature = "opengl")]
use std::ffi::CStr;

/// Tests for GL extension support.
///
/// Returns true if each extension name listed in extensions is supported
/// by the current GL context.
///
/// # Arguments
///
/// * `extensions` - Space-separated list of extension names
#[cfg(feature = "opengl")]
pub fn glf_has_extensions(extensions: &str) -> bool {
    let supported = glf_get_extensions();
    extensions
        .split_whitespace()
        .all(|ext| supported.iter().any(|s| s.eq_ignore_ascii_case(ext)))
}

/// Returns false when OpenGL feature is disabled.
#[cfg(not(feature = "opengl"))]
pub fn glf_has_extensions(_extensions: &str) -> bool {
    false
}

/// Queries GL vendor string.
#[cfg(feature = "opengl")]
pub fn glf_get_vendor() -> String {
    unsafe {
        let ptr = gl::GetString(gl::VENDOR);
        if ptr.is_null() {
            return String::new();
        }
        CStr::from_ptr(ptr as *const i8)
            .to_string_lossy()
            .into_owned()
    }
}

/// Returns empty string when OpenGL feature is disabled.
#[cfg(not(feature = "opengl"))]
pub fn glf_get_vendor() -> String {
    String::new()
}

/// Queries GL renderer string.
#[cfg(feature = "opengl")]
pub fn glf_get_renderer() -> String {
    unsafe {
        let ptr = gl::GetString(gl::RENDERER);
        if ptr.is_null() {
            return String::new();
        }
        CStr::from_ptr(ptr as *const i8)
            .to_string_lossy()
            .into_owned()
    }
}

/// Returns empty string when OpenGL feature is disabled.
#[cfg(not(feature = "opengl"))]
pub fn glf_get_renderer() -> String {
    String::new()
}

/// Queries GL version string.
#[cfg(feature = "opengl")]
pub fn glf_get_version() -> String {
    unsafe {
        let ptr = gl::GetString(gl::VERSION);
        if ptr.is_null() {
            return String::new();
        }
        CStr::from_ptr(ptr as *const i8)
            .to_string_lossy()
            .into_owned()
    }
}

/// Returns empty string when OpenGL feature is disabled.
#[cfg(not(feature = "opengl"))]
pub fn glf_get_version() -> String {
    String::new()
}

/// Queries GLSL version string.
#[cfg(feature = "opengl")]
pub fn glf_get_glsl_version() -> String {
    unsafe {
        let ptr = gl::GetString(gl::SHADING_LANGUAGE_VERSION);
        if ptr.is_null() {
            return String::new();
        }
        CStr::from_ptr(ptr as *const i8)
            .to_string_lossy()
            .into_owned()
    }
}

/// Returns empty string when OpenGL feature is disabled.
#[cfg(not(feature = "opengl"))]
pub fn glf_get_glsl_version() -> String {
    String::new()
}

/// Returns a list of all supported GL extensions.
#[cfg(feature = "opengl")]
pub fn glf_get_extensions() -> Vec<String> {
    unsafe {
        let mut num_extensions: gl::types::GLint = 0;
        gl::GetIntegerv(gl::NUM_EXTENSIONS, &mut num_extensions);

        let mut extensions = Vec::with_capacity(num_extensions as usize);
        for i in 0..num_extensions as gl::types::GLuint {
            let ptr = gl::GetStringi(gl::EXTENSIONS, i);
            if !ptr.is_null() {
                let ext = CStr::from_ptr(ptr as *const i8)
                    .to_string_lossy()
                    .into_owned();
                extensions.push(ext);
            }
        }
        extensions
    }
}

/// Returns empty list when OpenGL feature is disabled.
#[cfg(not(feature = "opengl"))]
pub fn glf_get_extensions() -> Vec<String> {
    Vec::new()
}

/// GL context information structure.
#[derive(Debug, Clone, Default)]
pub struct GlfContextInfo {
    /// GL vendor string
    pub vendor: String,
    /// GL renderer string
    pub renderer: String,
    /// GL version string
    pub version: String,
    /// GLSL version string
    pub glsl_version: String,
    /// List of supported extensions
    pub extensions: Vec<String>,
}

impl GlfContextInfo {
    /// Queries current GL context info.
    pub fn query_current() -> Self {
        Self {
            vendor: glf_get_vendor(),
            renderer: glf_get_renderer(),
            version: glf_get_version(),
            glsl_version: glf_get_glsl_version(),
            extensions: glf_get_extensions(),
        }
    }

    /// Checks if a specific extension is supported.
    pub fn has_extension(&self, extension: &str) -> bool {
        self.extensions
            .iter()
            .any(|ext| ext.eq_ignore_ascii_case(extension))
    }

    /// Checks if all specified extensions are supported.
    pub fn has_extensions(&self, extensions: &str) -> bool {
        extensions
            .split_whitespace()
            .all(|ext| self.has_extension(ext))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_info_with_extensions() {
        let mut info = GlfContextInfo::default();
        info.extensions.push("GL_ARB_vertex_shader".to_string());
        info.extensions.push("GL_ARB_fragment_shader".to_string());

        assert!(info.has_extension("GL_ARB_vertex_shader"));
        assert!(info.has_extension("GL_ARB_VERTEX_SHADER")); // Case insensitive
        assert!(!info.has_extension("GL_ARB_geometry_shader"));
        assert!(info.has_extensions("GL_ARB_vertex_shader GL_ARB_fragment_shader"));
        assert!(!info.has_extensions("GL_ARB_vertex_shader GL_ARB_geometry_shader"));
    }
}
