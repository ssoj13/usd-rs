//! OpenGL capabilities querying

use super::diagnostic::*;
use usd_hgi::*;

/// OpenGL-specific capabilities
///
/// Extends HgiCapabilities with OpenGL-specific information
#[derive(Debug, Clone)]
pub struct HgiGLCapabilities {
    /// Base HGI capabilities
    pub base: HgiCapabilities,

    /// OpenGL version (e.g., 450 for OpenGL 4.5)
    pub gl_version: i32,

    /// GLSL version (e.g., 450 for GLSL 4.50)
    pub glsl_version: i32,

    /// OpenGL renderer string
    pub renderer: String,

    /// OpenGL vendor string
    pub vendor: String,

    /// Supported OpenGL extensions
    pub extensions: Vec<String>,

    /// Whether ARB_direct_state_access is available
    pub has_direct_state_access: bool,

    /// Whether ARB_bindless_texture is available
    pub has_bindless_texture: bool,

    /// Whether ARB_multi_draw_indirect is available
    pub has_multi_draw_indirect: bool,

    /// Whether ARB_buffer_storage is available
    pub has_buffer_storage: bool,

    /// Whether ARB_shader_draw_parameters is available
    pub has_shader_draw_parameters: bool,
}

impl Default for HgiGLCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

impl HgiGLCapabilities {
    /// Create and initialize capabilities by querying OpenGL
    ///
    /// # Stub
    ///
    /// Real implementation: glGetIntegerv, glGetString for all caps.
    pub fn new() -> Self {
        let mut caps = Self {
            base: HgiCapabilities::default(),
            gl_version: 450,   // Assume OpenGL 4.5
            glsl_version: 450, // Assume GLSL 4.50
            renderer: get_gl_renderer(),
            vendor: get_gl_vendor(),
            extensions: Vec::new(),
            has_direct_state_access: false,
            has_bindless_texture: false,
            has_multi_draw_indirect: false,
            has_buffer_storage: false,
            has_shader_draw_parameters: false,
        };

        // Load capabilities from OpenGL
        caps.load_capabilities();

        caps
    }

    /// Load capabilities from OpenGL state
    ///
    /// # Stub
    ///
    /// Real implementation: parse GL_VERSION, enumerate extensions,
    /// query GL limits via glGetIntegerv.
    fn load_capabilities(&mut self) {
        // Note: Parse version from string
        let version_str = get_gl_version();
        self.gl_version = Self::parse_gl_version(&version_str);

        let glsl_str = get_glsl_version();
        self.glsl_version = Self::parse_glsl_version(&glsl_str);

        // Note: Would query extensions here
        // let num_extensions = get_integerv(gl::NUM_EXTENSIONS);
        // for i in 0..num_extensions {
        //     let ext = get_stringi(gl::EXTENSIONS, i);
        //     self.extensions.push(ext);
        // }

        // Check for important extensions (stub)
        self.has_direct_state_access = self.has_extension("GL_ARB_direct_state_access");
        self.has_bindless_texture = self.has_extension("GL_ARB_bindless_texture");
        self.has_multi_draw_indirect = self.has_extension("GL_ARB_multi_draw_indirect");
        self.has_buffer_storage = self.has_extension("GL_ARB_buffer_storage");
        self.has_shader_draw_parameters = self.has_extension("GL_ARB_shader_draw_parameters");

        // Query GL limits (stub values)
        self.base.max_uniform_block_size = 65536; // 64KB typical
        self.base.max_storage_block_size = 134217728; // 128MB typical
        self.base.max_texture_dimension_2d = 16384;
        self.base.max_texture_dimension_3d = 2048;
        self.base.max_texture_layers = 2048;
        self.base.max_vertex_attributes = 16;
        self.base.max_color_attachments = 8;
        self.base.max_compute_work_group_size = [1024, 1024, 64];
        self.base.max_compute_work_group_invocations = 1024;
        self.base.page_size_alignment = 256;

        // Set capability flags based on OpenGL version and extensions
        if self.gl_version >= 450 {
            self.base
                .device_capabilities
                .insert(HgiDeviceCapabilities::SHADER_DRAW_PARAMETERS);
            self.base
                .device_capabilities
                .insert(HgiDeviceCapabilities::MULTI_DRAW_INDIRECT);
            self.base
                .device_capabilities
                .insert(HgiDeviceCapabilities::BINDLESS_BUFFERS);
        }

        if self.has_bindless_texture {
            self.base.supports_bindless = true;
        }

        // OpenGL doesn't use unified memory (unlike Metal on macOS)
        self.base.uses_unified_memory = false;
    }

    /// Parse OpenGL version string to integer (e.g., "4.5.0" -> 450)
    fn parse_gl_version(version_str: &str) -> i32 {
        // Parse "4.5.0 STUB" or similar
        if let Some(first_space) = version_str.find(' ') {
            let version_part = &version_str[..first_space];
            Self::parse_version_string(version_part)
        } else {
            Self::parse_version_string(version_str)
        }
    }

    /// Parse GLSL version string to integer (e.g., "450" -> 450)
    fn parse_glsl_version(version_str: &str) -> i32 {
        // GLSL version is typically just "450" or "450 STUB"
        if let Some(first_space) = version_str.find(' ') {
            let version_part = &version_str[..first_space];
            version_part.parse().unwrap_or(450)
        } else {
            version_str.parse().unwrap_or(450)
        }
    }

    /// Parse version string like "4.5" or "4.5.0" to 450
    fn parse_version_string(s: &str) -> i32 {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() >= 2 {
            let major: i32 = parts[0].parse().unwrap_or(4);
            let minor: i32 = parts[1].parse().unwrap_or(5);
            major * 100 + minor * 10
        } else {
            450 // default
        }
    }

    /// Check if a specific extension is supported
    pub fn has_extension(&self, name: &str) -> bool {
        has_extension(name) || self.extensions.iter().any(|ext| ext == name)
    }

    /// Get the base HGI capabilities
    pub fn base_capabilities(&self) -> &HgiCapabilities {
        &self.base
    }

    /// Get the OpenGL version as integer (e.g., 450 for OpenGL 4.5)
    pub fn get_api_version(&self) -> i32 {
        self.gl_version
    }

    /// Get the GLSL version as integer (e.g., 450 for GLSL 4.50)
    pub fn get_shader_version(&self) -> i32 {
        self.glsl_version
    }

    /// Check if OpenGL version meets minimum requirement
    pub fn is_version_at_least(&self, major: i32, minor: i32) -> bool {
        let required = major * 100 + minor * 10;
        self.gl_version >= required
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        assert_eq!(
            HgiGLCapabilities::parse_gl_version("4.5.0 Core Profile"),
            450
        );
        assert_eq!(HgiGLCapabilities::parse_gl_version("4.6.0"), 460);
        assert_eq!(HgiGLCapabilities::parse_glsl_version("450"), 450);
        assert_eq!(HgiGLCapabilities::parse_glsl_version("460 core"), 460);
    }

    #[test]
    fn test_version_comparison() {
        let caps = HgiGLCapabilities::new();
        assert!(caps.is_version_at_least(4, 0));
        assert!(caps.is_version_at_least(4, 5));
    }

    #[test]
    fn test_capabilities_creation() {
        let caps = HgiGLCapabilities::new();
        assert!(caps.gl_version >= 400);
        assert!(caps.glsl_version >= 400);
        assert!(caps.base.max_texture_dimension_2d > 0);
    }
}
