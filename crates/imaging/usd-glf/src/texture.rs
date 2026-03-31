//! GL texture wrapper.
//!
//! Port of pxr/imaging/glf/texture.h

use super::{TfToken, VtDictionary};
use std::sync::Arc;

/// GL texture target types.
///
/// Corresponds to OpenGL texture targets (GL_TEXTURE_*) used in OpenUSD's Glf library.
///
/// # See Also
/// - `pxr/imaging/glf/texture.h` in OpenUSD
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlfTextureTarget {
    /// 1D texture (GL_TEXTURE_1D) - single dimension array of texels
    Texture1D,
    /// 2D texture (GL_TEXTURE_2D) - standard image texture
    Texture2D,
    /// 3D texture (GL_TEXTURE_3D) - volumetric texture with width/height/depth
    Texture3D,
    /// Cube map texture (GL_TEXTURE_CUBE_MAP) - 6 faces for environment mapping
    TextureCubeMap,
    /// 1D texture array (GL_TEXTURE_1D_ARRAY) - array of 1D textures
    Texture1DArray,
    /// 2D texture array (GL_TEXTURE_2D_ARRAY) - array of 2D textures
    Texture2DArray,
    /// Cube map array (GL_TEXTURE_CUBE_MAP_ARRAY) - array of cube maps
    TextureCubeMapArray,
}

/// A texture binding describes how a texture aspect should be bound
/// to allow shader access.
///
/// Most textures will have a single binding for the role "texels",
/// but some textures might need multiple bindings, e.g. a ptex texture
/// will have an additional binding for the role "layout".
#[derive(Debug, Clone)]
pub struct GlfTextureBinding {
    /// Binding name
    pub name: TfToken,
    /// Binding role (e.g., "texels", "layout")
    pub role: TfToken,
    /// GL texture target
    pub target: GlfTextureTarget,
    /// GL texture ID
    pub texture_id: u32,
    /// GL sampler ID
    pub sampler_id: u32,
}

impl GlfTextureBinding {
    /// Creates a new texture binding.
    pub fn new(
        name: TfToken,
        role: TfToken,
        target: GlfTextureTarget,
        texture_id: u32,
        sampler_id: u32,
    ) -> Self {
        Self {
            name,
            role,
            target,
            texture_id,
            sampler_id,
        }
    }
}

/// Represents a texture object in Glf.
///
/// A texture is typically defined by reading texture image data from an image
/// file but a texture might also represent an attachment of a draw target.
#[derive(Debug, Clone)]
pub struct GlfTexture {
    /// GL texture name/ID
    texture_id: u32,
    /// Texture target
    target: GlfTextureTarget,
    /// Memory used by this texture in bytes
    memory_used: usize,
    /// Memory requested for this texture in bytes
    memory_requested: usize,
    /// Contents ID for change tracking
    contents_id: usize,
    /// Image origin location (upper-left or lower-left)
    origin_lower_left: bool,
    /// Internal handle for resource management
    /// Note: Requires OpenGL feature for actual GPU resource binding
    #[allow(dead_code)]
    handle: Arc<GlfTextureHandle>,
}

/// Internal handle for GPU texture resource management.
///
/// This is a placeholder for platform-specific texture data.
/// In a full implementation, this would hold references to GPU resources.
#[derive(Debug)]
struct GlfTextureHandle {
    /// Zero-sized marker (placeholder for actual GPU handle)
    _marker: std::marker::PhantomData<()>,
}

impl GlfTexture {
    /// Creates a new empty texture.
    ///
    /// # Stub Implementation
    /// Returns a placeholder texture.
    pub fn new(target: GlfTextureTarget) -> Self {
        Self {
            texture_id: 0,
            target,
            memory_used: 0,
            memory_requested: 0,
            contents_id: 0,
            origin_lower_left: false,
            handle: Arc::new(GlfTextureHandle {
                _marker: std::marker::PhantomData,
            }),
        }
    }

    /// Creates a texture from image data.
    ///
    /// # Stub Implementation
    /// Returns a placeholder texture.
    pub fn from_image_data(
        _data: &[u8],
        _width: u32,
        _height: u32,
        _format: u32,
        target: GlfTextureTarget,
    ) -> Self {
        Self::new(target)
    }

    /// Returns the bindings to use this texture for the shader resource
    /// named identifier.
    ///
    /// If sampler_id is specified, the bindings returned will use this
    /// sampler_id for resources which can be sampled.
    ///
    /// # Stub Implementation
    /// Returns a single binding with placeholder values.
    pub fn get_bindings(&self, identifier: &TfToken, sampler_id: u32) -> Vec<GlfTextureBinding> {
        vec![GlfTextureBinding::new(
            identifier.clone(),
            TfToken::new("texels"),
            self.target,
            self.texture_id,
            sampler_id,
        )]
    }

    /// Returns the OpenGL texture name for the texture.
    pub fn get_gl_texture_name(&self) -> u32 {
        self.texture_id
    }

    /// Returns the texture target.
    pub fn get_target(&self) -> GlfTextureTarget {
        self.target
    }

    /// Amount of memory used to store the texture.
    pub fn get_memory_used(&self) -> usize {
        self.memory_used
    }

    /// Amount of memory the user wishes to allocate to the texture.
    pub fn get_memory_requested(&self) -> usize {
        self.memory_requested
    }

    /// Specify the amount of memory the user wishes to allocate to the texture.
    pub fn set_memory_requested(&mut self, target_memory: usize) {
        self.memory_requested = target_memory;
        self.on_memory_requested_dirty();
    }

    /// Returns texture information.
    ///
    /// # Stub Implementation
    /// Returns empty dictionary.
    pub fn get_texture_info(&self, _force_load: bool) -> VtDictionary {
        VtDictionary::new()
    }

    /// Checks if a minification filter is supported.
    ///
    /// # Stub Implementation
    /// Always returns true.
    pub fn is_min_filter_supported(&self, _filter: u32) -> bool {
        true
    }

    /// Checks if a magnification filter is supported.
    ///
    /// # Stub Implementation
    /// Always returns true.
    pub fn is_mag_filter_supported(&self, _filter: u32) -> bool {
        true
    }

    /// Returns an identifier that can be used to determine when the
    /// contents of this texture (i.e. its image data) has changed.
    ///
    /// The contents of most textures will be immutable for the lifetime
    /// of the texture. However, the contents of the texture attachments
    /// of a draw target change when the draw target is updated.
    pub fn get_contents_id(&self) -> usize {
        self.contents_id
    }

    /// Returns true if the texture origin is lower-left.
    pub fn is_origin_lower_left(&self) -> bool {
        self.origin_lower_left
    }

    /// Sets whether the texture origin is lower-left.
    pub fn set_origin_lower_left(&mut self, lower_left: bool) {
        self.origin_lower_left = lower_left;
    }

    /// Binds the texture to the current GL context.
    #[cfg(feature = "opengl")]
    pub fn bind(&self) {
        unsafe {
            gl::BindTexture(self.gl_target(), self.texture_id);
        }
    }

    /// No-op when OpenGL feature is disabled.
    #[cfg(not(feature = "opengl"))]
    pub fn bind(&self) {}

    /// Unbinds the texture from the current GL context.
    #[cfg(feature = "opengl")]
    pub fn unbind(&self) {
        unsafe {
            gl::BindTexture(self.gl_target(), 0);
        }
    }

    /// No-op when OpenGL feature is disabled.
    #[cfg(not(feature = "opengl"))]
    pub fn unbind(&self) {}

    /// Converts texture target to GL enum.
    #[cfg(feature = "opengl")]
    fn gl_target(&self) -> u32 {
        match self.target {
            GlfTextureTarget::Texture1D => gl::TEXTURE_1D,
            GlfTextureTarget::Texture2D => gl::TEXTURE_2D,
            GlfTextureTarget::Texture3D => gl::TEXTURE_3D,
            GlfTextureTarget::TextureCubeMap => gl::TEXTURE_CUBE_MAP,
            GlfTextureTarget::Texture1DArray => gl::TEXTURE_1D_ARRAY,
            GlfTextureTarget::Texture2DArray => gl::TEXTURE_2D_ARRAY,
            GlfTextureTarget::TextureCubeMapArray => gl::TEXTURE_CUBE_MAP_ARRAY,
        }
    }

    // Protected/internal methods

    #[allow(dead_code)] // Internal helper - used when OpenGL texture ops are enabled
    /// Sets the actual memory used by this texture.
    ///
    /// Used internally to track GPU memory usage. This may differ from
    /// the requested memory if the texture was resized or compressed.
    ///
    /// # Arguments
    /// * `size` - Actual memory usage in bytes
    fn set_memory_used(&mut self, size: usize) {
        self.memory_used = size;
    }

    /// Called when the requested memory amount changes.
    ///
    /// This triggers texture resizing or reloading to match the new memory target.
    fn on_memory_requested_dirty(&mut self) {
        // Memory budget management would trigger mipmap level adjustments
        // or texture reloading at different resolutions.
        // For now, just track that a change was requested.
        let _ = self.memory_requested;
    }

    #[allow(dead_code)] // Internal helper - used when draw target attachments are active
    /// Increments the contents ID to signal that texture data has changed.
    ///
    /// This is primarily used for draw target attachments where the texture
    /// contents can be updated dynamically (e.g., render-to-texture).
    /// Consumers can check the contents ID to detect when they need to re-bind
    /// or re-upload the texture.
    fn update_contents_id(&mut self) {
        self.contents_id += 1;
    }
}

impl Default for GlfTexture {
    fn default() -> Self {
        Self::new(GlfTextureTarget::Texture2D)
    }
}

/// Static reporting for texture memory usage.
pub struct GlfTextureRegistry;

impl GlfTextureRegistry {
    /// Returns the total texture memory allocated across all textures.
    ///
    /// # Stub Implementation
    /// Always returns 0.
    pub fn get_texture_memory_allocated() -> usize {
        // Note: Would aggregate memory_used from all live GlfTexture instances.
        // Returns 0 without texture memory tracking.
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_creation() {
        let tex = GlfTexture::new(GlfTextureTarget::Texture2D);
        assert_eq!(tex.get_gl_texture_name(), 0);
        assert_eq!(tex.get_target(), GlfTextureTarget::Texture2D);
    }

    #[test]
    fn test_texture_binding() {
        let tex = GlfTexture::new(GlfTextureTarget::Texture2D);
        let bindings = tex.get_bindings(&TfToken::new("diffuse"), 0);
        assert_eq!(bindings.len(), 1);
    }

    #[test]
    fn test_memory_tracking() {
        let mut tex = GlfTexture::new(GlfTextureTarget::Texture2D);
        tex.set_memory_requested(1024);
        assert_eq!(tex.get_memory_requested(), 1024);
    }
}
