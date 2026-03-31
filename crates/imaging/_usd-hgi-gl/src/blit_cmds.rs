//! OpenGL blit (copy) commands implementation

#[cfg(feature = "opengl")]
use super::conversions::*;
use usd_hgi::*;

/// OpenGL blit commands buffer
///
/// Records copy operations between buffers, textures, and CPU memory.
/// Commands are deferred and executed on submission.
#[derive(Debug)]
pub struct HgiGLBlitCmds {
    /// Recorded commands
    commands: Vec<BlitCommand>,

    /// Whether commands have been submitted
    submitted: bool,
}

/// Individual blit command types
#[derive(Debug)]
#[allow(dead_code)]
enum BlitCommand {
    /// Copy from CPU to GPU buffer
    CopyBufferCpuToGpu(HgiBufferCpuToGpuOp),
    /// Copy from GPU buffer to GPU buffer
    CopyBufferGpuToGpu(HgiBufferGpuToGpuOp),
    /// Copy from CPU to GPU texture
    CopyTextureCpuToGpu(HgiTextureCpuToGpuOp),
    /// Copy from GPU texture to GPU texture
    CopyTextureGpuToGpu(HgiTextureGpuToGpuOp),
    /// Copy from GPU texture to CPU
    CopyTextureGpuToCpu(HgiTextureGpuToCpuOp),
    /// Copy from GPU buffer to GPU texture
    CopyBufferToTexture(HgiBufferToTextureOp),
    /// Copy from GPU texture to GPU buffer
    CopyTextureToBuffer(HgiTextureToBufferOp),
    /// Generate mipmaps for a texture
    GenerateMipmap(HgiTextureHandle),
    /// Fill buffer with constant byte value
    FillBuffer(HgiBufferHandle, u8),
}

impl HgiGLBlitCmds {
    /// Create new blit commands buffer
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            submitted: false,
        }
    }

    /// Execute all recorded commands
    #[cfg(feature = "opengl")]
    pub fn execute(&mut self) {
        if self.submitted {
            return;
        }

        for cmd in &self.commands {
            match cmd {
                BlitCommand::CopyBufferCpuToGpu(op) => {
                    self.execute_copy_buffer_cpu_to_gpu(op);
                }
                BlitCommand::CopyBufferGpuToGpu(op) => {
                    self.execute_copy_buffer_gpu_to_gpu(op);
                }
                BlitCommand::CopyTextureCpuToGpu(op) => {
                    self.execute_copy_texture_cpu_to_gpu(op);
                }
                BlitCommand::CopyTextureGpuToGpu(op) => {
                    self.execute_copy_texture_gpu_to_gpu(op);
                }
                BlitCommand::CopyTextureGpuToCpu(op) => {
                    self.execute_copy_texture_gpu_to_cpu(op);
                }
                BlitCommand::CopyBufferToTexture(op) => {
                    self.execute_copy_buffer_to_texture(op);
                }
                BlitCommand::CopyTextureToBuffer(op) => {
                    self.execute_copy_texture_to_buffer(op);
                }
                BlitCommand::GenerateMipmap(texture) => {
                    self.execute_generate_mipmap(texture);
                }
                BlitCommand::FillBuffer(buffer, value) => {
                    self.execute_fill_buffer(buffer, *value);
                }
            }
        }

        self.submitted = true;
    }

    /// Execute all recorded commands (stub when opengl feature disabled)
    #[cfg(not(feature = "opengl"))]
    pub fn execute(&mut self) {
        self.submitted = true;
    }

    /// Execute buffer CPU to GPU copy
    #[cfg(feature = "opengl")]
    fn execute_copy_buffer_cpu_to_gpu(&self, op: &HgiBufferCpuToGpuOp) {
        use gl::types::*;

        if let Some(buffer) = op.gpu_destination_buffer.get() {
            let buffer_id = buffer.raw_resource() as GLuint;
            if buffer_id != 0 && op.byte_size > 0 && !op.cpu_source_buffer.as_ptr().is_null() {
                unsafe {
                    gl::NamedBufferSubData(
                        buffer_id,
                        op.destination_byte_offset as GLintptr,
                        op.byte_size as GLsizeiptr,
                        op.cpu_source_buffer.as_ptr() as *const std::ffi::c_void,
                    );
                }
            }
        }
    }

    /// Execute buffer GPU to GPU copy
    #[cfg(feature = "opengl")]
    fn execute_copy_buffer_gpu_to_gpu(&self, op: &HgiBufferGpuToGpuOp) {
        use gl::types::*;

        let src = op.gpu_source_buffer.get();
        let dst = op.gpu_destination_buffer.get();

        if let (Some(src_buf), Some(dst_buf)) = (src, dst) {
            let src_id = src_buf.raw_resource() as GLuint;
            let dst_id = dst_buf.raw_resource() as GLuint;

            if src_id != 0 && dst_id != 0 {
                unsafe {
                    gl::CopyNamedBufferSubData(
                        src_id,
                        dst_id,
                        op.source_byte_offset as GLintptr,
                        op.destination_byte_offset as GLintptr,
                        op.byte_size as GLsizeiptr,
                    );
                }
            }
        }
    }

    /// Execute texture CPU to GPU copy
    #[cfg(feature = "opengl")]
    fn execute_copy_texture_cpu_to_gpu(&self, op: &HgiTextureCpuToGpuOp) {
        use gl::types::*;

        if let Some(texture) = op.gpu_destination_texture.get() {
            let tex_id = texture.raw_resource() as GLuint;
            if tex_id == 0 || op.buffer_byte_size == 0 || op.cpu_source_buffer.as_ptr().is_null() {
                return;
            }

            let desc = texture.descriptor();
            let pixel_format = hgi_format_to_gl_pixel_format(desc.format);
            let pixel_type = hgi_format_to_gl_pixel_type(desc.format);

            unsafe {
                match desc.texture_type {
                    HgiTextureType::Texture2D => {
                        gl::TextureSubImage2D(
                            tex_id,
                            op.mip_level as GLint,
                            op.destination_texel_offset.x,
                            op.destination_texel_offset.y,
                            desc.dimensions.x,
                            desc.dimensions.y,
                            pixel_format,
                            pixel_type,
                            op.cpu_source_buffer.as_ptr() as *const std::ffi::c_void,
                        );
                    }
                    HgiTextureType::Texture3D => {
                        gl::TextureSubImage3D(
                            tex_id,
                            op.mip_level as GLint,
                            op.destination_texel_offset.x,
                            op.destination_texel_offset.y,
                            op.destination_texel_offset.z,
                            desc.dimensions.x,
                            desc.dimensions.y,
                            desc.dimensions.z,
                            pixel_format,
                            pixel_type,
                            op.cpu_source_buffer.as_ptr() as *const std::ffi::c_void,
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    /// Execute texture GPU to GPU copy
    #[cfg(feature = "opengl")]
    fn execute_copy_texture_gpu_to_gpu(&self, op: &HgiTextureGpuToGpuOp) {
        use gl::types::*;

        let src = op.gpu_source_texture.get();
        let dst = op.gpu_destination_texture.get();

        if let (Some(src_tex), Some(dst_tex)) = (src, dst) {
            let src_id = src_tex.raw_resource() as GLuint;
            let dst_id = dst_tex.raw_resource() as GLuint;

            if src_id != 0 && dst_id != 0 {
                let src_target = hgi_texture_type_to_gl_target(src_tex.descriptor().texture_type);
                let dst_target = hgi_texture_type_to_gl_target(dst_tex.descriptor().texture_type);

                unsafe {
                    gl::CopyImageSubData(
                        src_id,
                        src_target,
                        op.source_mip_level as GLint,
                        op.source_texel_offset.x,
                        op.source_texel_offset.y,
                        op.source_texel_offset.z,
                        dst_id,
                        dst_target,
                        op.destination_mip_level as GLint,
                        op.destination_texel_offset.x,
                        op.destination_texel_offset.y,
                        op.destination_texel_offset.z,
                        op.copy_size.x,
                        op.copy_size.y,
                        op.copy_size.z,
                    );
                }
            }
        }
    }

    /// Execute texture GPU to CPU copy
    #[cfg(feature = "opengl")]
    fn execute_copy_texture_gpu_to_cpu(&self, op: &HgiTextureGpuToCpuOp) {
        use gl::types::*;

        if let Some(texture) = op.gpu_source_texture.get() {
            let tex_id = texture.raw_resource() as GLuint;
            if tex_id == 0 {
                return;
            }

            let desc = texture.descriptor();
            let pixel_format = hgi_format_to_gl_pixel_format(desc.format);
            let pixel_type = hgi_format_to_gl_pixel_type(desc.format);
            let buffer_size = hgi_format_byte_size(desc.format)
                * (desc.dimensions.x as usize)
                * (desc.dimensions.y.max(1) as usize)
                * (desc.dimensions.z.max(1) as usize);

            // Write readback data to caller-provided CPU buffer
            let dst_ptr = op.cpu_destination_buffer.as_ptr();
            if dst_ptr.is_null() {
                log::error!("GPU->CPU texture copy: null destination buffer");
                return;
            }

            unsafe {
                gl::GetTextureImage(
                    tex_id,
                    op.mip_level as GLint,
                    pixel_format,
                    pixel_type,
                    buffer_size as GLsizei,
                    dst_ptr as *mut std::ffi::c_void,
                );
            }
        }
    }

    /// Execute buffer to texture copy
    #[cfg(feature = "opengl")]
    fn execute_copy_buffer_to_texture(&self, op: &HgiBufferToTextureOp) {
        use gl::types::*;

        let buffer = op.gpu_source_buffer.get();
        let texture = op.gpu_destination_texture.get();

        if let (Some(buf), Some(tex)) = (buffer, texture) {
            let buf_id = buf.raw_resource() as GLuint;
            let tex_id = tex.raw_resource() as GLuint;

            if buf_id != 0 && tex_id != 0 {
                let desc = tex.descriptor();
                let pixel_format = hgi_format_to_gl_pixel_format(desc.format);
                let pixel_type = hgi_format_to_gl_pixel_type(desc.format);

                unsafe {
                    // Bind buffer as pixel unpack buffer
                    gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, buf_id);

                    gl::TextureSubImage2D(
                        tex_id,
                        op.destination_mip_level as GLint,
                        op.destination_texel_offset.x,
                        op.destination_texel_offset.y,
                        op.copy_size.x,
                        op.copy_size.y,
                        pixel_format,
                        pixel_type,
                        op.source_byte_offset as *const std::ffi::c_void,
                    );

                    // Unbind pixel unpack buffer
                    gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, 0);
                }
            }
        }
    }

    /// Execute texture to buffer copy
    #[cfg(feature = "opengl")]
    fn execute_copy_texture_to_buffer(&self, op: &HgiTextureToBufferOp) {
        use gl::types::*;

        let texture = op.gpu_source_texture.get();
        let buffer = op.gpu_destination_buffer.get();

        if let (Some(tex), Some(buf)) = (texture, buffer) {
            let tex_id = tex.raw_resource() as GLuint;
            let buf_id = buf.raw_resource() as GLuint;

            if tex_id != 0 && buf_id != 0 {
                let desc = tex.descriptor();
                let pixel_format = hgi_format_to_gl_pixel_format(desc.format);
                let pixel_type = hgi_format_to_gl_pixel_type(desc.format);

                unsafe {
                    // Bind buffer as pixel pack buffer
                    gl::BindBuffer(gl::PIXEL_PACK_BUFFER, buf_id);

                    // Compute byte size from copy_size and format
                    let fmt_size = hgi_format_byte_size(desc.format);
                    let byte_size = (fmt_size
                        * op.copy_size.x as usize
                        * op.copy_size.y.max(1) as usize
                        * op.copy_size.z.max(1) as usize)
                        as GLsizei;

                    gl::GetTextureSubImage(
                        tex_id,
                        op.mip_level as GLint,
                        op.source_texel_offset.x,
                        op.source_texel_offset.y,
                        op.source_texel_offset.z,
                        op.copy_size.x,
                        op.copy_size.y,
                        op.copy_size.z,
                        pixel_format,
                        pixel_type,
                        byte_size,
                        op.destination_byte_offset as *mut std::ffi::c_void,
                    );

                    // Unbind pixel pack buffer
                    gl::BindBuffer(gl::PIXEL_PACK_BUFFER, 0);
                }
            }
        }
    }

    /// Execute mipmap generation
    #[cfg(feature = "opengl")]
    fn execute_generate_mipmap(&self, texture: &HgiTextureHandle) {
        if let Some(tex) = texture.get() {
            let tex_id = tex.raw_resource() as u32;
            if tex_id != 0 {
                unsafe {
                    gl::GenerateTextureMipmap(tex_id);
                }
            }
        }
    }

    /// Execute buffer fill with a constant byte value across the entire buffer
    #[cfg(feature = "opengl")]
    fn execute_fill_buffer(&self, buffer: &HgiBufferHandle, value: u8) {
        use gl::types::*;

        if let Some(buf) = buffer.get() {
            let buf_id = buf.raw_resource() as GLuint;
            if buf_id != 0 {
                // GL_R8UI clears each byte individually via ClearNamedBufferData.
                // For a full-buffer fill matching C++ semantics (uint8_t value).
                let size = buf.byte_size_of_resource();
                let value_u32 = value as u32;
                unsafe {
                    gl::ClearNamedBufferSubData(
                        buf_id,
                        gl::R8UI,
                        0,
                        size as GLsizeiptr,
                        gl::RED_INTEGER,
                        gl::UNSIGNED_BYTE,
                        &value_u32 as *const u32 as *const std::ffi::c_void,
                    );
                }
            }
        }
    }
}

impl Default for HgiGLBlitCmds {
    fn default() -> Self {
        Self::new()
    }
}

impl HgiCmds for HgiGLBlitCmds {
    fn is_submitted(&self) -> bool {
        self.submitted
    }

    fn execute_submit(&mut self) {
        self.execute();
    }

    #[cfg(feature = "opengl")]
    fn push_debug_group(&mut self, label: &str) {
        use std::ffi::CString;
        if let Ok(c_label) = CString::new(label) {
            unsafe {
                gl::PushDebugGroup(
                    gl::DEBUG_SOURCE_APPLICATION,
                    0,
                    label.len() as i32,
                    c_label.as_ptr(),
                );
            }
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn push_debug_group(&mut self, _label: &str) {}

    #[cfg(feature = "opengl")]
    fn pop_debug_group(&mut self) {
        unsafe {
            gl::PopDebugGroup();
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn pop_debug_group(&mut self) {}

    #[cfg(feature = "opengl")]
    fn insert_debug_marker(&mut self, label: &str) {
        use std::ffi::CString;
        if let Ok(c_label) = CString::new(label) {
            unsafe {
                gl::DebugMessageInsert(
                    gl::DEBUG_SOURCE_APPLICATION,
                    gl::DEBUG_TYPE_MARKER,
                    0,
                    gl::DEBUG_SEVERITY_NOTIFICATION,
                    label.len() as i32,
                    c_label.as_ptr(),
                );
            }
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn insert_debug_marker(&mut self, _label: &str) {}
}

impl HgiBlitCmds for HgiGLBlitCmds {
    fn copy_buffer_cpu_to_gpu(&mut self, op: &HgiBufferCpuToGpuOp) {
        self.commands
            .push(BlitCommand::CopyBufferCpuToGpu(op.clone()));
    }

    fn copy_buffer_gpu_to_gpu(&mut self, op: &HgiBufferGpuToGpuOp) {
        self.commands
            .push(BlitCommand::CopyBufferGpuToGpu(op.clone()));
    }

    fn copy_texture_cpu_to_gpu(&mut self, op: &HgiTextureCpuToGpuOp) {
        self.commands
            .push(BlitCommand::CopyTextureCpuToGpu(op.clone()));
    }

    fn copy_texture_gpu_to_gpu(&mut self, op: &HgiTextureGpuToGpuOp) {
        self.commands
            .push(BlitCommand::CopyTextureGpuToGpu(op.clone()));
    }

    fn copy_texture_gpu_to_cpu(&mut self, op: &HgiTextureGpuToCpuOp) {
        self.commands
            .push(BlitCommand::CopyTextureGpuToCpu(op.clone()));
    }

    fn copy_buffer_to_texture(&mut self, op: &HgiBufferToTextureOp) {
        self.commands
            .push(BlitCommand::CopyBufferToTexture(op.clone()));
    }

    fn copy_texture_to_buffer(&mut self, op: &HgiTextureToBufferOp) {
        self.commands
            .push(BlitCommand::CopyTextureToBuffer(op.clone()));
    }

    fn generate_mipmap(&mut self, texture: &HgiTextureHandle) {
        self.commands
            .push(BlitCommand::GenerateMipmap(texture.clone()));
    }

    fn fill_buffer(&mut self, buffer: &HgiBufferHandle, value: u8) {
        self.commands
            .push(BlitCommand::FillBuffer(buffer.clone(), value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blit_cmds_creation() {
        let cmds = HgiGLBlitCmds::new();
        assert!(!cmds.is_submitted());
        assert_eq!(cmds.commands.len(), 0);
    }

    #[test]
    fn test_record_commands() {
        let mut cmds = HgiGLBlitCmds::new();

        let buffer = HgiBufferHandle::null();
        cmds.fill_buffer(&buffer, 0u8);

        assert_eq!(cmds.commands.len(), 1);
        assert!(!cmds.is_submitted());
    }
}
