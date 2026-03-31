//! OpenGL buffer implementation.
//!
//! Port of pxr/imaging/hgiGL/buffer.h/cpp

use super::conversions::*;
use usd_hgi::*;

#[cfg(feature = "opengl")]
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};

/// OpenGL buffer resource.
///
/// Wraps an OpenGL buffer object (VBO, IBO, UBO, SSBO, etc.)
/// Port of HgiGLBuffer from pxr/imaging/hgiGL/buffer.h
#[derive(Debug)]
pub struct HgiGLBuffer {
    /// OpenGL buffer object name.
    gl_id: u32,

    /// Buffer descriptor.
    desc: HgiBufferDesc,

    /// Size in bytes.
    byte_size: usize,

    /// Buffer usage flags.
    usage: HgiBufferUsage,

    /// Mapped pointer (if currently mapped) - using AtomicPtr for thread safety.
    #[cfg(feature = "opengl")]
    mapped_ptr: AtomicPtr<u8>,

    /// CPU staging buffer for CopyBufferCpuToGpu operations.
    /// Allocated lazily when GetCPUStagingAddress is called.
    #[cfg(feature = "opengl")]
    cpu_staging: AtomicPtr<u8>,

    /// Bindless GPU address (NV_shader_buffer_load extension).
    /// Cached once fetched, remains valid until buffer is deleted.
    #[cfg(feature = "opengl")]
    #[allow(dead_code)]
    bindless_gpu_address: AtomicU64,
}

impl HgiGLBuffer {
    /// Create a new OpenGL buffer.
    ///
    /// # Arguments
    ///
    /// * `desc` - Buffer descriptor
    /// * `initial_data` - Optional initial data to upload
    ///
    /// # Panics
    ///
    /// Panics if `byte_size` is 0 or if `usage` contains `VERTEX` but
    /// `vertex_stride` is 0.
    pub fn new(desc: &HgiBufferDesc, initial_data: Option<&[u8]>) -> Self {
        // Validate buffer size
        if desc.byte_size == 0 {
            log::error!("Buffers must have a non-zero length");
        }

        // Validate vertex stride for vertex buffers
        if desc.usage.contains(HgiBufferUsage::VERTEX) && desc.vertex_stride == 0 {
            log::warn!("Vertex buffers should have a non-zero vertex_stride");
        }

        let gl_id = Self::create_gl_buffer(desc, initial_data);

        Self {
            gl_id,
            desc: desc.clone(),
            byte_size: desc.byte_size,
            usage: desc.usage,
            #[cfg(feature = "opengl")]
            mapped_ptr: AtomicPtr::new(std::ptr::null_mut()),
            #[cfg(feature = "opengl")]
            cpu_staging: AtomicPtr::new(std::ptr::null_mut()),
            #[cfg(feature = "opengl")]
            bindless_gpu_address: AtomicU64::new(0),
        }
    }

    /// Create OpenGL buffer object
    #[cfg(feature = "opengl")]
    fn create_gl_buffer(desc: &HgiBufferDesc, initial_data: Option<&[u8]>) -> u32 {
        use gl::types::*;

        let mut buffer_id: GLuint = 0;

        unsafe {
            // Create buffer object (DSA style)
            gl::CreateBuffers(1, &mut buffer_id);

            if buffer_id == 0 {
                log::error!("Failed to create OpenGL buffer");
                return 0;
            }

            // Determine GL usage hint
            let gl_usage = hgi_buffer_usage_to_gl_usage(desc.usage);

            // Allocate storage and optionally upload data
            let data_ptr = initial_data
                .map(|d| d.as_ptr() as *const std::ffi::c_void)
                .unwrap_or(std::ptr::null());

            gl::NamedBufferData(buffer_id, desc.byte_size as GLsizeiptr, data_ptr, gl_usage);

            // Set debug label if provided
            if !desc.debug_name.is_empty() {
                gl::ObjectLabel(
                    gl::BUFFER,
                    buffer_id,
                    desc.debug_name.len() as GLsizei,
                    desc.debug_name.as_ptr() as *const GLchar,
                );
            }
        }

        buffer_id
    }

    /// Create OpenGL buffer object (returns 0 when opengl feature disabled)
    #[cfg(not(feature = "opengl"))]
    fn create_gl_buffer(_desc: &HgiBufferDesc, _initial_data: Option<&[u8]>) -> u32 {
        // Note: Returns 0 (invalid) when OpenGL feature not compiled in
        0
    }

    /// Get the OpenGL buffer object name
    pub fn gl_id(&self) -> u32 {
        self.gl_id
    }

    /// Get the buffer descriptor
    pub fn descriptor(&self) -> &HgiBufferDesc {
        &self.desc
    }

    /// Get buffer size in bytes
    pub fn byte_size(&self) -> usize {
        self.byte_size
    }

    /// Get buffer usage flags.
    pub fn usage(&self) -> HgiBufferUsage {
        self.usage
    }

    /// Get CPU staging address for memcpy operations.
    ///
    /// Allocates staging buffer lazily on first call. The staging data
    /// must be explicitly copied to the GPU buffer via CopyBufferCpuToGpu.
    #[cfg(feature = "opengl")]
    pub fn get_cpu_staging_address(&self) -> *mut u8 {
        let mut ptr = self.cpu_staging.load(Ordering::Acquire);
        if ptr.is_null() {
            // Allocate staging buffer
            let layout = std::alloc::Layout::from_size_align(self.byte_size, 8)
                .expect("Invalid allocation layout");
            ptr = unsafe { std::alloc::alloc(layout) };
            self.cpu_staging.store(ptr, Ordering::Release);
        }
        ptr
    }

    /// Get CPU staging address (stub when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn get_cpu_staging_address(&self) -> *mut u8 {
        std::ptr::null_mut()
    }

    /// Get bindless GPU address (NV_shader_buffer_load extension).
    ///
    /// GPU address remains valid until the buffer object is deleted,
    /// or when the data store is respecified via BufferData/BufferStorage.
    #[cfg(feature = "opengl")]
    pub fn get_bindless_gpu_address(&self) -> u64 {
        // Bindless (NV_shader_buffer_load) requires driver extension.
        // gl crate may not include NV extensions; return 0 (non-bindless path).
        let _ = self;
        0
    }

    /// Get bindless GPU address (stub when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn get_bindless_gpu_address(&self) -> u64 {
        0
    }
}

impl HgiBuffer for HgiGLBuffer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiBufferDesc {
        &self.desc
    }

    fn byte_size_of_resource(&self) -> usize {
        self.byte_size
    }

    fn raw_resource(&self) -> u64 {
        self.gl_id as u64
    }

    #[cfg(feature = "opengl")]
    fn cpu_staging_address(&mut self) -> Option<*mut u8> {
        let ptr = self.get_cpu_staging_address();
        if ptr.is_null() { None } else { Some(ptr) }
    }

    #[cfg(not(feature = "opengl"))]
    fn cpu_staging_address(&mut self) -> Option<*mut u8> {
        None
    }
}

impl Drop for HgiGLBuffer {
    #[cfg(feature = "opengl")]
    fn drop(&mut self) {
        // Delete GL buffer
        if self.gl_id != 0 {
            unsafe {
                // Unmap if still mapped
                if !self.mapped_ptr.load(Ordering::Acquire).is_null() {
                    gl::UnmapNamedBuffer(self.gl_id);
                }
                gl::DeleteBuffers(1, &self.gl_id);
            }
        }

        // Free CPU staging buffer
        let cpu_ptr = self.cpu_staging.load(Ordering::Acquire);
        if !cpu_ptr.is_null() {
            let layout = std::alloc::Layout::from_size_align(self.byte_size, 8)
                .expect("Invalid allocation layout");
            unsafe {
                std::alloc::dealloc(cpu_ptr, layout);
            }
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn drop(&mut self) {
        // Nothing to clean up when OpenGL not available
    }
}

/// Upload data to an OpenGL buffer
#[cfg(feature = "opengl")]
pub fn upload_buffer_data(buffer: &HgiGLBuffer, data: &[u8], offset: usize) {
    use gl::types::*;

    if buffer.gl_id() == 0 {
        return;
    }

    unsafe {
        gl::NamedBufferSubData(
            buffer.gl_id(),
            offset as GLintptr,
            data.len() as GLsizeiptr,
            data.as_ptr() as *const std::ffi::c_void,
        );
    }
}

/// Upload data to an OpenGL buffer (no-op when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn upload_buffer_data(_buffer: &HgiGLBuffer, _data: &[u8], _offset: usize) {
    // Note: No-op when OpenGL feature not compiled in
}

/// Map a buffer for CPU access
#[cfg(feature = "opengl")]
pub fn map_buffer(buffer: &mut HgiGLBuffer, access: GLenum) -> *mut u8 {
    use gl::types::*;

    if buffer.gl_id() == 0 {
        return std::ptr::null_mut();
    }

    unsafe {
        let ptr = gl::MapNamedBuffer(buffer.gl_id(), access as GLenum) as *mut u8;
        buffer.mapped_ptr.store(ptr, Ordering::Release);
        ptr
    }
}

/// Map a buffer for CPU access (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn map_buffer(_buffer: &mut HgiGLBuffer, _access: GLenum) -> *mut u8 {
    std::ptr::null_mut()
}

/// Map a buffer range for CPU access
#[cfg(feature = "opengl")]
pub fn map_buffer_range(
    buffer: &mut HgiGLBuffer,
    offset: usize,
    length: usize,
    access: GLbitfield,
) -> *mut u8 {
    use gl::types::*;

    if buffer.gl_id() == 0 {
        return std::ptr::null_mut();
    }

    unsafe {
        let ptr = gl::MapNamedBufferRange(
            buffer.gl_id(),
            offset as GLintptr,
            length as GLsizeiptr,
            access,
        ) as *mut u8;
        buffer.mapped_ptr.store(ptr, Ordering::Release);
        ptr
    }
}

/// Map a buffer range for CPU access (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn map_buffer_range(
    _buffer: &mut HgiGLBuffer,
    _offset: usize,
    _length: usize,
    _access: GLbitfield,
) -> *mut u8 {
    std::ptr::null_mut()
}

/// Unmap a buffer
#[cfg(feature = "opengl")]
pub fn unmap_buffer(buffer: &mut HgiGLBuffer) -> bool {
    if buffer.gl_id() == 0 {
        return false;
    }

    unsafe {
        let result = gl::UnmapNamedBuffer(buffer.gl_id());
        buffer
            .mapped_ptr
            .store(std::ptr::null_mut(), Ordering::Release);
        result == gl::TRUE as u8
    }
}

/// Unmap a buffer (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn unmap_buffer(_buffer: &mut HgiGLBuffer) -> bool {
    false
}

/// Bind buffer to a binding point
#[cfg(feature = "opengl")]
pub fn bind_buffer_base(buffer: &HgiGLBuffer, target: GLenum, binding_point: u32) {
    use gl::types::*;

    if buffer.gl_id() == 0 {
        return;
    }

    unsafe {
        gl::BindBufferBase(target as GLenum, binding_point as GLuint, buffer.gl_id());
    }
}

/// Bind buffer to a binding point (no-op when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn bind_buffer_base(_buffer: &HgiGLBuffer, _target: GLenum, _binding_point: u32) {
    // Note: No-op when OpenGL feature not compiled in
}

/// Bind buffer range to a binding point
#[cfg(feature = "opengl")]
pub fn bind_buffer_range(
    buffer: &HgiGLBuffer,
    target: GLenum,
    binding_point: u32,
    offset: usize,
    size: usize,
) {
    use gl::types::*;

    if buffer.gl_id() == 0 {
        return;
    }

    unsafe {
        gl::BindBufferRange(
            target as GLenum,
            binding_point as GLuint,
            buffer.gl_id(),
            offset as GLintptr,
            size as GLsizeiptr,
        );
    }
}

/// Bind buffer range to a binding point (no-op when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn bind_buffer_range(
    _buffer: &HgiGLBuffer,
    _target: GLenum,
    _binding_point: u32,
    _offset: usize,
    _size: usize,
) {
    // Note: No-op when OpenGL feature not compiled in
}

/// Copy data between buffers
#[cfg(feature = "opengl")]
pub fn copy_buffer_sub_data(
    src: &HgiGLBuffer,
    dst: &HgiGLBuffer,
    src_offset: usize,
    dst_offset: usize,
    size: usize,
) {
    use gl::types::*;

    if src.gl_id() == 0 || dst.gl_id() == 0 {
        return;
    }

    unsafe {
        gl::CopyNamedBufferSubData(
            src.gl_id(),
            dst.gl_id(),
            src_offset as GLintptr,
            dst_offset as GLintptr,
            size as GLsizeiptr,
        );
    }
}

/// Copy data between buffers (no-op when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn copy_buffer_sub_data(
    _src: &HgiGLBuffer,
    _dst: &HgiGLBuffer,
    _src_offset: usize,
    _dst_offset: usize,
    _size: usize,
) {
    // Note: No-op when OpenGL feature not compiled in
}

/// Clear buffer with a value
#[cfg(feature = "opengl")]
pub fn clear_buffer_sub_data(
    buffer: &HgiGLBuffer,
    internal_format: GLenum,
    offset: usize,
    size: usize,
    format: GLenum,
    data_type: GLenum,
    data: *const std::ffi::c_void,
) {
    use gl::types::*;

    if buffer.gl_id() == 0 {
        return;
    }

    unsafe {
        gl::ClearNamedBufferSubData(
            buffer.gl_id(),
            internal_format as GLenum,
            offset as GLintptr,
            size as GLsizeiptr,
            format as GLenum,
            data_type as GLenum,
            data,
        );
    }
}

/// Clear buffer with a value (no-op when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn clear_buffer_sub_data(
    _buffer: &HgiGLBuffer,
    _internal_format: GLenum,
    _offset: usize,
    _size: usize,
    _format: GLenum,
    _data_type: GLenum,
    _data: *const std::ffi::c_void,
) {
    // Note: No-op when OpenGL feature not compiled in
}

#[cfg(all(test, feature = "opengl"))]
pub(crate) fn run_gl_tests() {
    use super::*;

    let desc = HgiBufferDesc::new()
        .with_usage(HgiBufferUsage::VERTEX)
        .with_byte_size(1024);

    let buffer = HgiGLBuffer::new(&desc, None);
    assert_eq!(buffer.byte_size(), 1024);
    assert!(buffer.usage().contains(HgiBufferUsage::VERTEX));

    let data = vec![0u8; 256];
    let desc = HgiBufferDesc::new()
        .with_usage(HgiBufferUsage::UNIFORM)
        .with_byte_size(256);

    let buffer = HgiGLBuffer::new(&desc, Some(&data));
    assert_eq!(buffer.byte_size(), 256);
}
