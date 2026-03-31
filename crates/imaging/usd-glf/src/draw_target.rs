//! Offscreen render target (FBO) management.
//!
//! Port of pxr/imaging/glf/drawTarget.h
//!
//! Provides framebuffer object (FBO) management for offscreen rendering
//! with support for multiple color attachments and MSAA.

use super::{GlfTexture, GlfTextureBinding, GlfTextureTarget, TfToken, VtDictionary};
use std::collections::HashMap;
use std::sync::Arc;
use usd_gf::Vec2i;

/// Type alias for 2D integer vector matching USD naming convention.
pub type GfVec2i = Vec2i;

/// GL type alias for cleaner code
#[cfg(feature = "opengl")]
#[allow(non_camel_case_types)]
type GLenum = u32;

// GL constants for FBO operations
#[cfg(feature = "opengl")]
const GL_FRAMEBUFFER: GLenum = 0x8D40;
#[cfg(feature = "opengl")]
const GL_READ_FRAMEBUFFER: GLenum = 0x8CA8;
#[cfg(feature = "opengl")]
#[allow(dead_code)]
const GL_DRAW_FRAMEBUFFER: GLenum = 0x8CA9;
#[cfg(feature = "opengl")]
const GL_COLOR_ATTACHMENT0: GLenum = 0x8CE0;
#[cfg(feature = "opengl")]
const GL_DEPTH_ATTACHMENT: GLenum = 0x8D00;
#[cfg(feature = "opengl")]
const GL_DEPTH_STENCIL_ATTACHMENT: GLenum = 0x821A;
#[cfg(feature = "opengl")]
const GL_FRAMEBUFFER_COMPLETE: GLenum = 0x8CD5;
#[cfg(feature = "opengl")]
const GL_TEXTURE_2D: GLenum = 0x0DE1;
#[cfg(feature = "opengl")]
const GL_TEXTURE_2D_MULTISAMPLE: GLenum = 0x9100;
#[cfg(feature = "opengl")]
const GL_FRAMEBUFFER_BINDING: GLenum = 0x8CA6;
#[cfg(feature = "opengl")]
const GL_NEAREST: GLenum = 0x2600;
#[cfg(feature = "opengl")]
const GL_COLOR_BUFFER_BIT: u32 = 0x00004000;
#[cfg(feature = "opengl")]
const GL_DEPTH_BUFFER_BIT: u32 = 0x00000100;

/// GL framebuffer attachment format specifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlfAttachmentFormat {
    /// GL format (e.g., GL_RGB, GL_DEPTH_COMPONENT)
    pub format: u32,
    /// GL type (e.g., GL_UNSIGNED_BYTE, GL_FLOAT)
    pub type_: u32,
    /// GL internal format (e.g., GL_RGBA8, GL_DEPTH24_STENCIL8)
    pub internal_format: u32,
}

impl GlfAttachmentFormat {
    /// Creates new attachment format specification.
    ///
    /// # Arguments
    /// * `format` - GL pixel format (e.g., GL_RGBA)
    /// * `type_` - GL pixel data type (e.g., GL_UNSIGNED_BYTE)
    /// * `internal_format` - GL internal format (e.g., GL_RGBA8)
    pub fn new(format: u32, type_: u32, internal_format: u32) -> Self {
        Self {
            format,
            type_,
            internal_format,
        }
    }

    /// Common RGBA8 format for color attachments
    pub fn rgba8() -> Self {
        Self::new(0x1908, 0x1401, 0x8058) // GL_RGBA, GL_UNSIGNED_BYTE, GL_RGBA8
    }

    /// Common RGBA16F format for HDR color attachments
    pub fn rgba16f() -> Self {
        Self::new(0x1908, 0x140B, 0x881A) // GL_RGBA, GL_HALF_FLOAT, GL_RGBA16F
    }

    /// Common RGBA32F format for full precision color
    pub fn rgba32f() -> Self {
        Self::new(0x1908, 0x1406, 0x8814) // GL_RGBA, GL_FLOAT, GL_RGBA32F
    }

    /// Common depth24/stencil8 format
    pub fn depth24_stencil8() -> Self {
        Self::new(0x84F9, 0x84FA, 0x88F0) // GL_DEPTH_STENCIL, GL_UNSIGNED_INT_24_8, GL_DEPTH24_STENCIL8
    }

    /// Common depth32f format
    pub fn depth32f() -> Self {
        Self::new(0x1902, 0x1406, 0x8CAC) // GL_DEPTH_COMPONENT, GL_FLOAT, GL_DEPTH_COMPONENT32F
    }

    /// Check if this is a depth format
    pub fn is_depth(&self) -> bool {
        self.format == 0x1902 || self.format == 0x84F9 // GL_DEPTH_COMPONENT or GL_DEPTH_STENCIL
    }
}

/// An attachment to a draw target.
///
/// Represents a texture attachment to a framebuffer object.
#[derive(Debug, Clone)]
pub struct GlfDrawTargetAttachment {
    /// Underlying texture for non-MSAA or resolved
    texture: GlfTexture,
    /// GL texture ID (created by this attachment)
    texture_id: u32,
    /// GL attachment index
    gl_index: i32,
    /// Attachment format
    format: GlfAttachmentFormat,
    /// Attachment size
    size: GfVec2i,
    /// Number of MSAA samples (0 = no MSAA)
    num_samples: u32,
    /// Multisampled texture ID (if MSAA enabled)
    texture_ms_id: u32,
}

impl GlfDrawTargetAttachment {
    /// Creates a new attachment with real OpenGL texture.
    #[cfg(feature = "opengl")]
    pub fn new(
        gl_index: i32,
        format: GlfAttachmentFormat,
        size: GfVec2i,
        num_samples: u32,
    ) -> Self {
        use gl::types::*;

        let mut texture_id: GLuint = 0;
        let mut texture_ms_id: GLuint = 0;

        unsafe {
            // Create resolve/non-MSAA texture
            gl::CreateTextures(GL_TEXTURE_2D, 1, &mut texture_id);
            if texture_id != 0 {
                gl::TextureStorage2D(
                    texture_id,
                    1,
                    format.internal_format,
                    size.x.max(1),
                    size.y.max(1),
                );
            }

            // Create MSAA texture if needed
            if num_samples > 1 {
                gl::CreateTextures(GL_TEXTURE_2D_MULTISAMPLE, 1, &mut texture_ms_id);
                if texture_ms_id != 0 {
                    gl::TextureStorage2DMultisample(
                        texture_ms_id,
                        num_samples as GLsizei,
                        format.internal_format,
                        size.x.max(1),
                        size.y.max(1),
                        gl::TRUE,
                    );
                }
            }
        }

        Self {
            texture: GlfTexture::new(GlfTextureTarget::Texture2D),
            texture_id,
            gl_index,
            format,
            size,
            num_samples,
            texture_ms_id,
        }
    }

    /// Creates a new attachment (stub when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn new(
        gl_index: i32,
        format: GlfAttachmentFormat,
        size: GfVec2i,
        num_samples: u32,
    ) -> Self {
        Self {
            texture: GlfTexture::new(GlfTextureTarget::Texture2D),
            texture_id: 0,
            gl_index,
            format,
            size,
            num_samples,
            texture_ms_id: 0,
        }
    }

    /// Returns the GL texture ID (resolved texture for MSAA).
    pub fn get_gl_texture_name(&self) -> u32 {
        self.texture_id
    }

    /// Returns the GL texture ID for multisampled texture (if MSAA enabled).
    pub fn get_gl_texture_ms_name(&self) -> u32 {
        self.texture_ms_id
    }

    /// Returns the GL format of the texture.
    pub fn get_format(&self) -> u32 {
        self.format.format
    }

    /// Returns the GL type of the texture.
    pub fn get_type(&self) -> u32 {
        self.format.type_
    }

    /// Returns the GL internal format of the texture.
    pub fn get_internal_format(&self) -> u32 {
        self.format.internal_format
    }

    /// Returns the GL attachment point index in the framebuffer.
    pub fn get_attach(&self) -> i32 {
        self.gl_index
    }

    /// Returns the attachment size.
    pub fn get_size(&self) -> GfVec2i {
        self.size
    }

    /// Returns whether this attachment uses MSAA.
    pub fn is_msaa(&self) -> bool {
        self.num_samples > 1
    }

    /// Resizes the attachment, recreating textures.
    #[cfg(feature = "opengl")]
    pub fn resize_texture(&mut self, size: GfVec2i) {
        use gl::types::*;

        if self.size == size {
            return;
        }
        self.size = size;

        unsafe {
            // Delete old textures
            if self.texture_id != 0 {
                gl::DeleteTextures(1, &self.texture_id);
            }
            if self.texture_ms_id != 0 {
                gl::DeleteTextures(1, &self.texture_ms_id);
            }

            // Create new resolve texture
            gl::CreateTextures(GL_TEXTURE_2D, 1, &mut self.texture_id);
            if self.texture_id != 0 {
                gl::TextureStorage2D(
                    self.texture_id,
                    1,
                    self.format.internal_format,
                    size.x.max(1),
                    size.y.max(1),
                );
            }

            // Create new MSAA texture if needed
            if self.num_samples > 1 {
                gl::CreateTextures(GL_TEXTURE_2D_MULTISAMPLE, 1, &mut self.texture_ms_id);
                if self.texture_ms_id != 0 {
                    gl::TextureStorage2DMultisample(
                        self.texture_ms_id,
                        self.num_samples as GLsizei,
                        self.format.internal_format,
                        size.x.max(1),
                        size.y.max(1),
                        gl::TRUE,
                    );
                }
            }
        }
    }

    /// Resizes the attachment (stub when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn resize_texture(&mut self, size: GfVec2i) {
        self.size = size;
    }

    /// Returns the bindings for this attachment.
    pub fn get_bindings(&self, identifier: &TfToken, sampler_id: u32) -> Vec<GlfTextureBinding> {
        self.texture.get_bindings(identifier, sampler_id)
    }

    /// Returns texture information.
    pub fn get_texture_info(&self, force_load: bool) -> VtDictionary {
        self.texture.get_texture_info(force_load)
    }

    /// Updates the contents signature for the underlying texture.
    pub fn touch_contents(&mut self) {
        // Content tracking for cache invalidation
    }

    /// Returns a reference to the underlying texture.
    pub fn get_texture(&self) -> &GlfTexture {
        &self.texture
    }
}

impl Drop for GlfDrawTargetAttachment {
    #[cfg(feature = "opengl")]
    fn drop(&mut self) {
        unsafe {
            if self.texture_id != 0 {
                gl::DeleteTextures(1, &self.texture_id);
            }
            if self.texture_ms_id != 0 {
                gl::DeleteTextures(1, &self.texture_ms_id);
            }
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn drop(&mut self) {}
}

/// A draw target with multiple image attachments.
///
/// Represents a GL framebuffer object with color and depth attachments
/// for offscreen rendering.
#[derive(Debug, Clone)]
pub struct GlfDrawTarget {
    /// Framebuffer object ID
    fbo: u32,
    /// Resolve FBO for MSAA (blits from MSAA to resolve textures)
    #[allow(dead_code)]
    fbo_resolve: u32,
    /// Size of the draw target
    size: GfVec2i,
    /// Whether MSAA is requested
    msaa: bool,
    /// Number of MSAA samples
    num_samples: u32,
    /// Named attachments map
    attachments: Arc<HashMap<String, Arc<GlfDrawTargetAttachment>>>,
    /// Owning flag (false if sharing attachments)
    #[allow(dead_code)]
    owns_attachments: bool,
    /// Saved read framebuffer binding (written by bind, read by unbind via GL)
    #[cfg_attr(not(feature = "opengl"), allow(dead_code))]
    unbind_restore_read_fb: u32,
    /// Saved draw framebuffer binding (written by bind, read by unbind via GL)
    #[cfg_attr(not(feature = "opengl"), allow(dead_code))]
    unbind_restore_draw_fb: u32,
    /// Bind depth counter for nested bind/unbind calls
    bind_depth: i32,
}

impl GlfDrawTarget {
    /// Creates a new draw target with the given size.
    #[cfg(feature = "opengl")]
    pub fn new(size: GfVec2i, request_msaa: bool) -> Self {
        use gl::types::*;

        let mut fbo: GLuint = 0;
        let mut fbo_resolve: GLuint = 0;
        let num_samples = if request_msaa { 4 } else { 0 };

        unsafe {
            // Create main FBO
            gl::CreateFramebuffers(1, &mut fbo);

            // Create resolve FBO if MSAA enabled
            if request_msaa {
                gl::CreateFramebuffers(1, &mut fbo_resolve);
            }
        }

        Self {
            fbo,
            fbo_resolve,
            size,
            msaa: request_msaa,
            num_samples,
            attachments: Arc::new(HashMap::new()),
            owns_attachments: true,
            unbind_restore_read_fb: 0,
            unbind_restore_draw_fb: 0,
            bind_depth: 0,
        }
    }

    /// Creates a new draw target (stub when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn new(size: GfVec2i, request_msaa: bool) -> Self {
        Self {
            fbo: 0,
            fbo_resolve: 0,
            size,
            msaa: request_msaa,
            num_samples: if request_msaa { 4 } else { 0 },
            attachments: Arc::new(HashMap::new()),
            owns_attachments: true,
            unbind_restore_read_fb: 0,
            unbind_restore_draw_fb: 0,
            bind_depth: 0,
        }
    }

    /// Creates a new draw target sharing attachments with another.
    ///
    /// GL framebuffers cannot be shared across contexts, but texture
    /// attachments can. This creates a new framebuffer with shared attachments.
    #[cfg(feature = "opengl")]
    pub fn new_with_shared_attachments(other: &GlfDrawTarget) -> Self {
        use gl::types::*;

        let mut fbo: GLuint = 0;
        let mut fbo_resolve: GLuint = 0;

        unsafe {
            gl::CreateFramebuffers(1, &mut fbo);
            if other.msaa {
                gl::CreateFramebuffers(1, &mut fbo_resolve);
            }
        }

        let target = Self {
            fbo,
            fbo_resolve,
            size: other.size,
            msaa: other.msaa,
            num_samples: other.num_samples,
            attachments: Arc::clone(&other.attachments),
            owns_attachments: false,
            unbind_restore_read_fb: 0,
            unbind_restore_draw_fb: 0,
            bind_depth: 0,
        };

        // Re-attach shared textures to new FBO
        target.rebind_attachments();
        target
    }

    /// Creates a new draw target sharing attachments (stub when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn new_with_shared_attachments(other: &GlfDrawTarget) -> Self {
        Self {
            fbo: 0,
            fbo_resolve: 0,
            size: other.size,
            msaa: other.msaa,
            num_samples: other.num_samples,
            attachments: Arc::clone(&other.attachments),
            owns_attachments: false,
            unbind_restore_read_fb: 0,
            unbind_restore_draw_fb: 0,
            bind_depth: 0,
        }
    }

    /// Rebinds all attachments to the FBO.
    #[cfg(feature = "opengl")]
    fn rebind_attachments(&self) {
        if self.fbo == 0 {
            return;
        }

        unsafe {
            for (name, attachment) in self.attachments.iter() {
                let attachment_point = if attachment.format.is_depth() {
                    if attachment.format.format == 0x84F9 {
                        // GL_DEPTH_STENCIL
                        GL_DEPTH_STENCIL_ATTACHMENT
                    } else {
                        GL_DEPTH_ATTACHMENT
                    }
                } else {
                    GL_COLOR_ATTACHMENT0 + attachment.gl_index as u32
                };

                // Attach MSAA texture to main FBO, resolve texture to resolve FBO
                if self.msaa && attachment.texture_ms_id != 0 {
                    gl::NamedFramebufferTexture(
                        self.fbo,
                        attachment_point,
                        attachment.texture_ms_id,
                        0,
                    );
                    if self.fbo_resolve != 0 {
                        gl::NamedFramebufferTexture(
                            self.fbo_resolve,
                            attachment_point,
                            attachment.texture_id,
                            0,
                        );
                    }
                } else {
                    gl::NamedFramebufferTexture(
                        self.fbo,
                        attachment_point,
                        attachment.texture_id,
                        0,
                    );
                }

                let _ = name; // Used for debug purposes
            }
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn rebind_attachments(&self) {}

    /// Adds an attachment to the draw target.
    #[cfg(feature = "opengl")]
    pub fn add_attachment(&mut self, name: String, format: GlfAttachmentFormat) {
        use gl::types::*;

        let gl_index = self.attachments.len() as i32;
        let attachment = Arc::new(GlfDrawTargetAttachment::new(
            gl_index,
            format,
            self.size,
            self.num_samples,
        ));

        // Determine attachment point
        let attachment_point = if format.is_depth() {
            if format.format == 0x84F9 {
                GL_DEPTH_STENCIL_ATTACHMENT
            } else {
                GL_DEPTH_ATTACHMENT
            }
        } else {
            GL_COLOR_ATTACHMENT0 + gl_index as u32
        };

        unsafe {
            // Attach to main FBO
            if self.msaa && attachment.texture_ms_id != 0 {
                gl::NamedFramebufferTexture(
                    self.fbo,
                    attachment_point,
                    attachment.texture_ms_id,
                    0,
                );
                // Attach resolve texture to resolve FBO
                if self.fbo_resolve != 0 {
                    gl::NamedFramebufferTexture(
                        self.fbo_resolve,
                        attachment_point,
                        attachment.texture_id,
                        0,
                    );
                }
            } else {
                gl::NamedFramebufferTexture(self.fbo, attachment_point, attachment.texture_id, 0);
            }

            // Set draw buffers for color attachments
            if !format.is_depth() {
                let color_count = self
                    .attachments
                    .values()
                    .filter(|a| !a.format.is_depth())
                    .count()
                    + 1;
                let draw_buffers: Vec<GLenum> = (0..color_count as u32)
                    .map(|i| GL_COLOR_ATTACHMENT0 + i)
                    .collect();
                gl::NamedFramebufferDrawBuffers(
                    self.fbo,
                    draw_buffers.len() as i32,
                    draw_buffers.as_ptr(),
                );
            }
        }

        Arc::make_mut(&mut self.attachments).insert(name, attachment);
    }

    /// Adds an attachment (stub when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn add_attachment(&mut self, name: String, format: GlfAttachmentFormat) {
        let attachment = Arc::new(GlfDrawTargetAttachment::new(
            self.attachments.len() as i32,
            format,
            self.size,
            self.num_samples,
        ));
        Arc::make_mut(&mut self.attachments).insert(name, attachment);
    }

    /// Removes an attachment from the draw target.
    pub fn remove_attachment(&mut self, name: &str) {
        Arc::make_mut(&mut self.attachments).remove(name);
    }

    /// Gets an attachment by name.
    pub fn get_attachment(&self, name: &str) -> Option<Arc<GlfDrawTargetAttachment>> {
        self.attachments.get(name).cloned()
    }

    /// Returns all attachment names.
    pub fn get_attachment_names(&self) -> Vec<String> {
        self.attachments.keys().cloned().collect()
    }

    /// Returns the draw target size.
    pub fn get_size(&self) -> GfVec2i {
        self.size
    }

    /// Sets the draw target size, resizing all attachments.
    pub fn set_size(&mut self, size: GfVec2i) {
        if self.size == size {
            return;
        }
        self.size = size;

        // Resize all attachments
        let attachments = Arc::make_mut(&mut self.attachments);
        for attachment in attachments.values_mut() {
            Arc::make_mut(attachment).resize_texture(size);
        }

        // Rebind resized attachments
        self.rebind_attachments();
    }

    /// Returns the GL framebuffer object ID (resolve FBO for MSAA targets).
    pub fn get_framebuffer_id(&self) -> u32 {
        self.fbo
    }

    /// Returns the MSAA (multisample) framebuffer object ID.
    ///
    /// For non-MSAA targets this is 0. Matches C++ `GetFramebufferMSId()`.
    pub fn get_framebuffer_ms_id(&self) -> u32 {
        if self.msaa { self.fbo } else { 0 }
    }

    /// Check if FBO is complete and valid.
    #[cfg(feature = "opengl")]
    pub fn is_valid(&self) -> bool {
        if self.fbo == 0 {
            return false;
        }

        unsafe {
            let status = gl::CheckNamedFramebufferStatus(self.fbo, GL_FRAMEBUFFER);
            status == GL_FRAMEBUFFER_COMPLETE
        }
    }

    /// Check if FBO is complete (stub when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn is_valid(&self) -> bool {
        false
    }

    /// Returns whether the draw target is currently bound.
    ///
    /// Matches C++ `IsBound()`: returns `bind_depth > 0`.
    pub fn is_bound(&self) -> bool {
        self.bind_depth > 0
    }

    /// Binds the draw target for rendering.
    ///
    /// Matches C++ `Bind()`: reference-counted. Only the first (outermost) bind
    /// saves the previous FBO state. Nested binds are no-ops.
    #[cfg(feature = "opengl")]
    pub fn bind(&mut self) {
        self.bind_depth += 1;
        if self.bind_depth != 1 {
            return; // nested bind — already bound
        }
        if self.fbo != 0 {
            unsafe {
                // Save current FBO bindings for restore on unbind (C++ _SaveBindingState)
                gl::GetIntegerv(
                    0x8CAA, // GL_READ_FRAMEBUFFER_BINDING
                    &mut self.unbind_restore_read_fb as *mut u32 as *mut i32,
                );
                gl::GetIntegerv(
                    0x8CA6, // GL_DRAW_FRAMEBUFFER_BINDING
                    &mut self.unbind_restore_draw_fb as *mut u32 as *mut i32,
                );
                let bind_target = if self.msaa && self.fbo_resolve != 0 {
                    self.fbo_resolve
                } else {
                    self.fbo
                };
                gl::BindFramebuffer(GL_FRAMEBUFFER, bind_target);
            }
        }
    }

    /// Binds the draw target (stub when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn bind(&mut self) {
        self.bind_depth += 1;
    }

    /// Unbinds the draw target, restoring the previous framebuffer.
    ///
    /// Matches C++ `Unbind()`: only the outermost unbind restores saved FBO state.
    #[cfg(feature = "opengl")]
    pub fn unbind(&mut self) {
        self.bind_depth -= 1;
        if self.bind_depth != 0 {
            return; // still nested
        }
        unsafe {
            // Restore saved FBO bindings (C++ _RestoreBindingState)
            gl::BindFramebuffer(0x8CA8, self.unbind_restore_read_fb); // GL_READ_FRAMEBUFFER
            gl::BindFramebuffer(0x8CA9, self.unbind_restore_draw_fb); // GL_DRAW_FRAMEBUFFER
        }
    }

    /// Unbinds the draw target (stub when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn unbind(&mut self) {
        if self.bind_depth > 0 {
            self.bind_depth -= 1;
        }
    }

    /// Returns whether MSAA is enabled.
    pub fn is_msaa_enabled(&self) -> bool {
        self.msaa
    }

    /// Returns the number of MSAA samples.
    pub fn get_num_samples(&self) -> u32 {
        self.num_samples
    }

    /// Resolves MSAA by blitting to resolve textures.
    #[cfg(feature = "opengl")]
    pub fn resolve(&self) {
        if !self.msaa || self.fbo_resolve == 0 {
            return;
        }

        let width = self.size.x.max(1);
        let height = self.size.y.max(1);

        unsafe {
            // Blit from MSAA FBO to resolve FBO
            gl::BlitNamedFramebuffer(
                self.fbo,
                self.fbo_resolve,
                0,
                0,
                width,
                height,
                0,
                0,
                width,
                height,
                GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT,
                GL_NEAREST,
            );
        }
    }

    /// Resolves MSAA (stub when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn resolve(&self) {}

    /// Writes an attachment to an image file (for debugging).
    #[cfg(feature = "opengl")]
    pub fn write_to_file(&self, name: &str, filename: &str) {
        use gl::types::*;

        let Some(attachment) = self.get_attachment(name) else {
            log::warn!("Attachment '{}' not found", name);
            return;
        };

        // Resolve MSAA first
        self.resolve();

        let width = attachment.get_size().x as usize;
        let height = attachment.get_size().y as usize;
        let format = attachment.get_format();
        let type_ = attachment.get_type();

        // Calculate buffer size (assuming 4 components for color)
        let components = if format == 0x1908 { 4 } else { 1 }; // GL_RGBA = 4, depth = 1
        let bytes_per_component = match type_ {
            0x1401 => 1, // GL_UNSIGNED_BYTE
            0x1406 => 4, // GL_FLOAT
            _ => 4,
        };
        let buffer_size = width * height * components * bytes_per_component;
        let mut buffer = vec![0u8; buffer_size];

        unsafe {
            // Read from resolve FBO or main FBO
            let read_fbo = if self.msaa && self.fbo_resolve != 0 {
                self.fbo_resolve
            } else {
                self.fbo
            };

            gl::BindFramebuffer(GL_READ_FRAMEBUFFER, read_fbo);

            if attachment.format.is_depth() {
                gl::ReadBuffer(GL_DEPTH_ATTACHMENT);
            } else {
                gl::ReadBuffer(GL_COLOR_ATTACHMENT0 + attachment.gl_index as GLenum);
            }

            gl::ReadPixels(
                0,
                0,
                width as GLsizei,
                height as GLsizei,
                format,
                type_,
                buffer.as_mut_ptr() as *mut std::ffi::c_void,
            );

            gl::BindFramebuffer(GL_READ_FRAMEBUFFER, 0);
        }

        // Write to file (basic PPM format for debugging)
        if filename.ends_with(".ppm") && format == 0x1908 && type_ == 0x1401 {
            if let Ok(mut file) = std::fs::File::create(filename) {
                use std::io::Write;
                let _ = writeln!(file, "P6\n{} {}\n255", width, height);
                // Convert RGBA to RGB and flip vertically
                for y in (0..height).rev() {
                    for x in 0..width {
                        let idx = (y * width + x) * 4;
                        let _ = file.write_all(&buffer[idx..idx + 3]);
                    }
                }
            }
        }

        log::debug!("Wrote attachment '{}' to '{}'", name, filename);
    }

    /// Writes an attachment to file (stub when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn write_to_file(&self, _name: &str, _filename: &str) {}

    /// Reads color attachment pixels as RGBA (4 bytes per pixel).
    ///
    /// Returns pixels in top-left origin order (egui/conventional), i.e. Y is flipped
    /// from OpenGL's bottom-left origin. Call after render, before unbind.
    ///
    /// # Arguments
    /// * `name` - Attachment name (e.g. "color")
    ///
    /// # Returns
    /// * `Some(Vec<u8>)` - RGBA bytes, row-major, top-left origin; `width*height*4` bytes
    /// * `None` - If attachment not found or wrong format
    #[cfg(feature = "opengl")]
    pub fn read_color_pixels(&self, name: &str) -> Option<Vec<u8>> {
        use gl::types::*;

        let Some(attachment) = self.get_attachment(name) else {
            return None;
        };
        if attachment.format.is_depth() {
            return None;
        }

        self.resolve();

        let width = attachment.get_size().x as usize;
        let height = attachment.get_size().y as usize;
        let format = attachment.get_format();
        let type_ = attachment.get_type();

        if format != 0x1908 || type_ != 0x1401 {
            // Need GL_RGBA + GL_UNSIGNED_BYTE for RGBA u8
            return None;
        }

        let buffer_size = width * height * 4;
        let mut buffer = vec![0u8; buffer_size];

        unsafe {
            let read_fbo = if self.msaa && self.fbo_resolve != 0 {
                self.fbo_resolve
            } else {
                self.fbo
            };

            gl::BindFramebuffer(GL_READ_FRAMEBUFFER, read_fbo);
            gl::ReadBuffer(GL_COLOR_ATTACHMENT0 + attachment.gl_index as GLenum);

            gl::ReadPixels(
                0,
                0,
                width as GLsizei,
                height as GLsizei,
                format,
                type_,
                buffer.as_mut_ptr() as *mut std::ffi::c_void,
            );

            gl::BindFramebuffer(GL_READ_FRAMEBUFFER, 0);
        }

        // Flip Y: OpenGL bottom-left → top-left for egui
        if height > 1 {
            let row_bytes = width * 4;
            let mut row_temp = vec![0u8; row_bytes];
            for y in 0..(height / 2) {
                let top = y * row_bytes;
                let bot = (height - 1 - y) * row_bytes;
                row_temp.copy_from_slice(&buffer[top..top + row_bytes]);
                buffer.copy_within(bot..bot + row_bytes, top);
                buffer[bot..bot + row_bytes].copy_from_slice(&row_temp);
            }
        }

        Some(buffer)
    }

    /// Reads color pixels (stub when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn read_color_pixels(&self, _name: &str) -> Option<Vec<u8>> {
        None
    }
}

impl Drop for GlfDrawTarget {
    #[cfg(feature = "opengl")]
    fn drop(&mut self) {
        unsafe {
            if self.fbo != 0 {
                gl::DeleteFramebuffers(1, &self.fbo);
            }
            if self.fbo_resolve != 0 {
                gl::DeleteFramebuffers(1, &self.fbo_resolve);
            }
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn drop(&mut self) {}
}

/// RAII guard for binding a draw target.
///
/// Binds on construction, unbinds (restoring previous FBO) on drop.
/// Relies on `GlfDrawTarget::bind_depth` for correct nesting semantics.
pub struct GlfDrawTargetGuard {
    target: GlfDrawTarget,
}

impl GlfDrawTargetGuard {
    /// Creates a new guard, binding the draw target.
    #[cfg(feature = "opengl")]
    pub fn new(mut target: GlfDrawTarget) -> Self {
        target.bind();
        Self { target }
    }

    /// Creates a new guard (stub when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn new(mut target: GlfDrawTarget) -> Self {
        target.bind();
        Self { target }
    }

    /// Returns the bound draw target.
    pub fn target(&self) -> &GlfDrawTarget {
        &self.target
    }
}

impl Drop for GlfDrawTargetGuard {
    #[cfg(feature = "opengl")]
    fn drop(&mut self) {
        self.target.unbind();
    }

    #[cfg(not(feature = "opengl"))]
    fn drop(&mut self) {
        self.target.unbind();
    }
}

/// Query the currently bound framebuffer.
#[cfg(feature = "opengl")]
pub fn get_current_framebuffer() -> u32 {
    use gl::types::*;
    let mut fbo: GLint = 0;
    unsafe {
        gl::GetIntegerv(GL_FRAMEBUFFER_BINDING as GLenum, &mut fbo);
    }
    fbo as u32
}

/// Query the currently bound framebuffer (stub when opengl feature disabled).
#[cfg(not(feature = "opengl"))]
pub fn get_current_framebuffer() -> u32 {
    0
}

#[cfg(all(test, feature = "opengl"))]
pub(crate) fn run_gl_tests() {
    use super::*;

    let target = GlfDrawTarget::new(GfVec2i::new(512, 512), false);
    assert_eq!(target.get_size(), GfVec2i::new(512, 512));
    assert!(!target.is_msaa_enabled());

    let target = GlfDrawTarget::new(GfVec2i::new(512, 512), true);
    assert!(target.is_msaa_enabled());
    assert_eq!(target.get_num_samples(), 4);

    let mut target = GlfDrawTarget::new(GfVec2i::new(512, 512), false);
    target.add_attachment("color".to_string(), GlfAttachmentFormat::rgba8());
    target.add_attachment("depth".to_string(), GlfAttachmentFormat::depth24_stencil8());

    assert!(target.get_attachment("color").is_some());
    assert!(target.get_attachment("depth").is_some());
    assert_eq!(target.get_attachment_names().len(), 2);

    let mut target1 = GlfDrawTarget::new(GfVec2i::new(256, 256), false);
    target1.add_attachment("color".to_string(), GlfAttachmentFormat::rgba8());

    let target2 = GlfDrawTarget::new_with_shared_attachments(&target1);
    assert_eq!(target2.get_size(), target1.get_size());
    assert!(target2.get_attachment("color").is_some());

    let mut target = GlfDrawTarget::new(GfVec2i::new(256, 256), false);
    target.add_attachment("color".to_string(), GlfAttachmentFormat::rgba8());

    target.set_size(GfVec2i::new(512, 512));
    assert_eq!(target.get_size(), GfVec2i::new(512, 512));

    let attachment = target.get_attachment("color").unwrap();
    assert_eq!(attachment.get_size(), GfVec2i::new(512, 512));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attachment_format() {
        let format = GlfAttachmentFormat::new(0x1908, 0x1401, 0x8058);
        assert_eq!(format.format, 0x1908);
        assert_eq!(format.type_, 0x1401);
        assert_eq!(format.internal_format, 0x8058);
        assert!(!format.is_depth());

        let depth = GlfAttachmentFormat::depth24_stencil8();
        assert!(depth.is_depth());
    }

    // -----------------------------------------------------------------------
    // bind_depth (nested bind/unbind) tests — no GL context needed
    // -----------------------------------------------------------------------

    #[test]
    fn test_bind_depth_single() {
        // Single bind/unbind pair: depth goes 0 -> 1 -> 0.
        let mut target = GlfDrawTarget::new(GfVec2i::new(64, 64), false);
        assert!(!target.is_bound());

        target.bind();
        assert!(target.is_bound());
        assert_eq!(target.bind_depth, 1);

        target.unbind();
        assert!(!target.is_bound());
        assert_eq!(target.bind_depth, 0);
    }

    #[test]
    fn test_bind_depth_nested() {
        // C++ Bind() increments, Unbind() decrements.
        // Only the outermost unbind actually restores the FBO.
        let mut target = GlfDrawTarget::new(GfVec2i::new(64, 64), false);

        target.bind(); // depth = 1, should bind
        target.bind(); // depth = 2, no-op (nested)
        assert!(target.is_bound());
        assert_eq!(target.bind_depth, 2);

        target.unbind(); // depth = 1, still bound (nested unbind is no-op)
        assert!(target.is_bound());
        assert_eq!(target.bind_depth, 1);

        target.unbind(); // depth = 0, now actually unbinds
        assert!(!target.is_bound());
        assert_eq!(target.bind_depth, 0);
    }

    #[test]
    fn test_bind_depth_triple_nesting() {
        let mut target = GlfDrawTarget::new(GfVec2i::new(64, 64), false);
        target.bind();
        target.bind();
        target.bind();
        assert_eq!(target.bind_depth, 3);

        target.unbind();
        assert_eq!(target.bind_depth, 2);
        assert!(target.is_bound());

        target.unbind();
        assert_eq!(target.bind_depth, 1);
        assert!(target.is_bound());

        target.unbind();
        assert_eq!(target.bind_depth, 0);
        assert!(!target.is_bound());
    }
}
