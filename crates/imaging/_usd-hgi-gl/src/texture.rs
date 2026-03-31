//! OpenGL texture implementation

use super::conversions::*;
use usd_gf::Vec3i;
use usd_hgi::*;

/// OpenGL texture resource
///
/// Wraps an OpenGL texture object (1D, 2D, 3D, array, cubemap)
#[derive(Debug)]
pub struct HgiGLTexture {
    /// OpenGL texture object name
    gl_id: u32,

    /// Texture descriptor
    desc: HgiTextureDesc,

    /// Texture target (GL_TEXTURE_2D, GL_TEXTURE_3D, etc.)
    gl_target: GLenum,

    /// Internal format (GL_RGBA8, GL_RGBA16F, etc.)
    gl_internal_format: GLenum,
}

impl HgiGLTexture {
    /// Wrap an existing GL texture name (e.g. a glTextureView result).
    ///
    /// The returned object takes ownership of `gl_id` and will call
    /// `glDeleteTextures` on drop. Used by `HgiGL::create_texture_view()`.
    pub fn from_raw(gl_id: u32, desc: &HgiTextureDesc) -> Self {
        let gl_target = hgi_texture_type_to_gl_target(desc.texture_type);
        // P0-2: use usage-aware conversion so depth targets get GL_DEPTH_COMPONENT32F.
        let is_depth = desc.usage.contains(usd_hgi::HgiTextureUsage::DEPTH_TARGET);
        let gl_internal_format = hgi_format_to_gl_internal_format_with_usage(desc.format, is_depth);
        Self {
            gl_id,
            desc: desc.clone(),
            gl_target,
            gl_internal_format,
        }
    }

    /// Create a new OpenGL texture
    pub fn new(desc: &HgiTextureDesc, initial_data: Option<&[u8]>) -> Self {
        let gl_target = hgi_texture_type_to_gl_target(desc.texture_type);
        // P0-2: use usage-aware conversion so depth targets get GL_DEPTH_COMPONENT32F.
        let is_depth = desc.usage.contains(usd_hgi::HgiTextureUsage::DEPTH_TARGET);
        let gl_internal_format = hgi_format_to_gl_internal_format_with_usage(desc.format, is_depth);
        let gl_id = Self::create_gl_texture(desc, initial_data, gl_target, gl_internal_format);

        Self {
            gl_id,
            desc: desc.clone(),
            gl_target,
            gl_internal_format,
        }
    }

    /// Create OpenGL texture object
    #[cfg(feature = "opengl")]
    fn create_gl_texture(
        desc: &HgiTextureDesc,
        initial_data: Option<&[u8]>,
        gl_target: GLenum,
        gl_internal_format: GLenum,
    ) -> u32 {
        use gl::types::*;

        let mut texture_id: GLuint = 0;

        unsafe {
            gl::CreateTextures(gl_target, 1, &mut texture_id);

            if texture_id == 0 {
                log::error!("Failed to create OpenGL texture");
                return 0;
            }

            let width = desc.dimensions.x.max(1);
            let height = desc.dimensions.y.max(1);
            let depth = desc.dimensions.z.max(1);
            let mip_levels = desc.mip_levels.max(1) as GLsizei;

            // Allocate immutable storage based on texture type
            match desc.texture_type {
                HgiTextureType::Texture1D => {
                    gl::TextureStorage1D(texture_id, mip_levels, gl_internal_format, width);
                }
                HgiTextureType::Texture2D | HgiTextureType::Cubemap => {
                    gl::TextureStorage2D(texture_id, mip_levels, gl_internal_format, width, height);
                }
                HgiTextureType::Texture3D => {
                    gl::TextureStorage3D(
                        texture_id,
                        mip_levels,
                        gl_internal_format,
                        width,
                        height,
                        depth,
                    );
                }
                HgiTextureType::Texture1DArray => {
                    gl::TextureStorage2D(
                        texture_id,
                        mip_levels,
                        gl_internal_format,
                        width,
                        desc.layer_count as GLsizei,
                    );
                }
                HgiTextureType::Texture2DArray => {
                    gl::TextureStorage3D(
                        texture_id,
                        mip_levels,
                        gl_internal_format,
                        width,
                        height,
                        desc.layer_count as GLsizei,
                    );
                }
            }

            // Upload initial data if provided
            if let Some(data) = initial_data {
                let is_depth = desc.usage.contains(usd_hgi::HgiTextureUsage::DEPTH_TARGET);
                let pixel_format = hgi_format_to_gl_pixel_format_with_usage(desc.format, is_depth);
                let pixel_type = hgi_format_to_gl_pixel_type(desc.format);

                match desc.texture_type {
                    HgiTextureType::Texture1D => {
                        gl::TextureSubImage1D(
                            texture_id,
                            0,
                            0,
                            width,
                            pixel_format,
                            pixel_type,
                            data.as_ptr() as *const std::ffi::c_void,
                        );
                    }
                    HgiTextureType::Texture2D | HgiTextureType::Cubemap => {
                        gl::TextureSubImage2D(
                            texture_id,
                            0,
                            0,
                            0,
                            width,
                            height,
                            pixel_format,
                            pixel_type,
                            data.as_ptr() as *const std::ffi::c_void,
                        );
                    }
                    HgiTextureType::Texture3D | HgiTextureType::Texture2DArray => {
                        gl::TextureSubImage3D(
                            texture_id,
                            0,
                            0,
                            0,
                            0,
                            width,
                            height,
                            depth,
                            pixel_format,
                            pixel_type,
                            data.as_ptr() as *const std::ffi::c_void,
                        );
                    }
                    HgiTextureType::Texture1DArray => {
                        gl::TextureSubImage2D(
                            texture_id,
                            0,
                            0,
                            0,
                            width,
                            desc.layer_count as GLsizei,
                            pixel_format,
                            pixel_type,
                            data.as_ptr() as *const std::ffi::c_void,
                        );
                    }
                }
            }

            // Set debug label if provided
            if !desc.debug_name.is_empty() {
                gl::ObjectLabel(
                    gl::TEXTURE,
                    texture_id,
                    desc.debug_name.len() as GLsizei,
                    desc.debug_name.as_ptr() as *const GLchar,
                );
            }
        }

        texture_id
    }

    #[cfg(not(feature = "opengl"))]
    fn create_gl_texture(
        _desc: &HgiTextureDesc,
        _initial_data: Option<&[u8]>,
        _gl_target: GLenum,
        _gl_internal_format: GLenum,
    ) -> u32 {
        0
    }

    /// Get the OpenGL texture object name
    pub fn gl_id(&self) -> u32 {
        self.gl_id
    }

    /// Get the OpenGL texture target
    pub fn gl_target(&self) -> GLenum {
        self.gl_target
    }

    /// Get the OpenGL internal format
    pub fn gl_internal_format(&self) -> GLenum {
        self.gl_internal_format
    }

    /// Get the texture descriptor
    pub fn descriptor(&self) -> &HgiTextureDesc {
        &self.desc
    }

    /// Get texture dimensions
    pub fn dimensions(&self) -> Vec3i {
        self.desc.dimensions
    }

    /// Get texture format
    pub fn format(&self) -> HgiFormat {
        self.desc.format
    }

    /// Get texture type
    pub fn texture_type(&self) -> HgiTextureType {
        self.desc.texture_type
    }

    /// Get mip level count
    pub fn mip_levels(&self) -> u16 {
        self.desc.mip_levels
    }
}

impl HgiTexture for HgiGLTexture {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiTextureDesc {
        &self.desc
    }

    fn byte_size_of_resource(&self) -> usize {
        let width = self.desc.dimensions.x as usize;
        let height = self.desc.dimensions.y.max(1) as usize;
        let depth = self.desc.dimensions.z.max(1) as usize;
        let mip_levels = self.desc.mip_levels.max(1) as usize;
        let bpp = hgi_format_byte_size(self.desc.format);

        width * height * depth * bpp * mip_levels
    }

    fn raw_resource(&self) -> u64 {
        self.gl_id as u64
    }

    fn cpu_staging_address(&mut self) -> Option<*mut u8> {
        None
    }
}

impl Drop for HgiGLTexture {
    #[cfg(feature = "opengl")]
    fn drop(&mut self) {
        if self.gl_id != 0 {
            unsafe {
                gl::DeleteTextures(1, &self.gl_id);
            }
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn drop(&mut self) {}
}

/// Upload data to a texture
#[cfg(feature = "opengl")]
pub fn upload_texture_data(
    texture: &HgiGLTexture,
    data: &[u8],
    mip_level: u32,
    offset: Vec3i,
    dimensions: Vec3i,
) {
    use gl::types::*;

    if texture.gl_id() == 0 {
        return;
    }

    let pixel_format = hgi_format_to_gl_pixel_format(texture.format());
    let pixel_type = hgi_format_to_gl_pixel_type(texture.format());

    unsafe {
        match texture.texture_type() {
            HgiTextureType::Texture1D => {
                gl::TextureSubImage1D(
                    texture.gl_id(),
                    mip_level as GLint,
                    offset.x,
                    dimensions.x,
                    pixel_format,
                    pixel_type,
                    data.as_ptr() as *const std::ffi::c_void,
                );
            }
            HgiTextureType::Texture2D
            | HgiTextureType::Texture1DArray
            | HgiTextureType::Cubemap => {
                gl::TextureSubImage2D(
                    texture.gl_id(),
                    mip_level as GLint,
                    offset.x,
                    offset.y,
                    dimensions.x,
                    dimensions.y,
                    pixel_format,
                    pixel_type,
                    data.as_ptr() as *const std::ffi::c_void,
                );
            }
            HgiTextureType::Texture3D | HgiTextureType::Texture2DArray => {
                gl::TextureSubImage3D(
                    texture.gl_id(),
                    mip_level as GLint,
                    offset.x,
                    offset.y,
                    offset.z,
                    dimensions.x,
                    dimensions.y,
                    dimensions.z,
                    pixel_format,
                    pixel_type,
                    data.as_ptr() as *const std::ffi::c_void,
                );
            }
        }
    }
}

/// Upload data to a texture (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn upload_texture_data(
    _texture: &HgiGLTexture,
    _data: &[u8],
    _mip_level: u32,
    _offset: Vec3i,
    _dimensions: Vec3i,
) {
}

/// Generate mipmaps for a texture
#[cfg(feature = "opengl")]
pub fn generate_mipmaps(texture: &HgiGLTexture) {
    if texture.gl_id() != 0 {
        unsafe {
            gl::GenerateTextureMipmap(texture.gl_id());
        }
    }
}

/// Generate mipmaps for a texture (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn generate_mipmaps(_texture: &HgiGLTexture) {}

/// Bind texture to a texture unit
#[cfg(feature = "opengl")]
pub fn bind_texture(texture: &HgiGLTexture, unit: u32) {
    if texture.gl_id() != 0 {
        unsafe {
            gl::BindTextureUnit(unit, texture.gl_id());
        }
    }
}

/// Bind texture to a texture unit (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn bind_texture(_texture: &HgiGLTexture, _unit: u32) {}

/// Clear a texture to a specific color
#[cfg(feature = "opengl")]
pub fn clear_texture(texture: &HgiGLTexture, color: &[f32; 4]) {
    if texture.gl_id() != 0 {
        let pixel_format = hgi_format_to_gl_pixel_format(texture.format());
        unsafe {
            gl::ClearTexImage(
                texture.gl_id(),
                0,
                pixel_format,
                gl::FLOAT,
                color.as_ptr() as *const std::ffi::c_void,
            );
        }
    }
}

/// Clear a texture to a specific color (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn clear_texture(_texture: &HgiGLTexture, _color: &[f32; 4]) {}

/// Copy image data between textures
#[cfg(feature = "opengl")]
pub fn copy_texture(
    src: &HgiGLTexture,
    dst: &HgiGLTexture,
    src_offset: Vec3i,
    dst_offset: Vec3i,
    src_mip: u32,
    dst_mip: u32,
    dimensions: Vec3i,
) {
    use gl::types::*;

    if src.gl_id() != 0 && dst.gl_id() != 0 {
        unsafe {
            gl::CopyImageSubData(
                src.gl_id(),
                src.gl_target(),
                src_mip as GLint,
                src_offset.x,
                src_offset.y,
                src_offset.z,
                dst.gl_id(),
                dst.gl_target(),
                dst_mip as GLint,
                dst_offset.x,
                dst_offset.y,
                dst_offset.z,
                dimensions.x,
                dimensions.y,
                dimensions.z,
            );
        }
    }
}

/// Copy image data between textures (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn copy_texture(
    _src: &HgiGLTexture,
    _dst: &HgiGLTexture,
    _src_offset: Vec3i,
    _dst_offset: Vec3i,
    _src_mip: u32,
    _dst_mip: u32,
    _dimensions: Vec3i,
) {
}

#[cfg(all(test, feature = "opengl"))]
pub(crate) fn run_gl_tests() {
    use super::*;

    let desc = HgiTextureDesc::new()
        .with_format(HgiFormat::UNorm8Vec4)
        .with_dimensions(Vec3i::new(512, 512, 1))
        .with_texture_type(HgiTextureType::Texture2D)
        .with_usage(HgiTextureUsage::COLOR_TARGET | HgiTextureUsage::SHADER_READ);

    let texture = HgiGLTexture::new(&desc, None);
    assert_eq!(texture.dimensions(), Vec3i::new(512, 512, 1));
    assert_eq!(texture.format(), HgiFormat::UNorm8Vec4);
    assert_eq!(texture.gl_target(), GL_TEXTURE_2D);

    let desc = HgiTextureDesc::new()
        .with_format(HgiFormat::Float32Vec4)
        .with_dimensions(Vec3i::new(128, 128, 128))
        .with_texture_type(HgiTextureType::Texture3D)
        .with_usage(HgiTextureUsage::SHADER_READ);

    let texture = HgiGLTexture::new(&desc, None);
    assert_eq!(texture.dimensions(), Vec3i::new(128, 128, 128));
    assert_eq!(texture.format(), HgiFormat::Float32Vec4);
    assert_eq!(texture.gl_target(), GL_TEXTURE_3D);
}
