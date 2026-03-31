//! GL Uniform Buffer Object (UBO) abstraction.
//!
//! Port of pxr/imaging/glf/uniformBlock.h / uniformBlock.cpp
//!
//! `GlfUniformBlock` wraps a single `GL_UNIFORM_BUFFER` object.
//! The caller owns the data layout; this struct only handles allocation,
//! upload, and binding to a numbered binding point.

use crate::binding_map::GlfBindingMap;

/// Manages one GL uniform buffer object.
///
/// Mirrors `GlfUniformBlock`.  Typical usage:
///
/// ```ignore
/// let mut ubo = GlfUniformBlock::new(Some("Transforms"));
/// ubo.update(bytemuck::bytes_of(&my_data));
/// ubo.bind(&mut binding_map, "Transforms");
/// ```
pub struct GlfUniformBlock {
    /// GL buffer object ID (0 when not allocated / feature disabled).
    buffer_id: u32,
    /// Currently allocated size in bytes on the GPU.
    size: usize,
    /// Optional label for GL object debugging.
    debug_label: Option<String>,
}

impl GlfUniformBlock {
    /// Create a new UBO.  The GPU buffer is allocated lazily on the first
    /// `update()` call, not here (mirrors C++ where `glGenBuffers` is called
    /// in the constructor but `glBufferData` happens in `Update`).
    pub fn new(label: Option<&str>) -> Self {
        #[cfg(feature = "opengl")]
        {
            let mut id: u32 = 0;
            unsafe {
                gl::GenBuffers(1, &mut id);
            }
            Self {
                buffer_id: id,
                size: 0,
                debug_label: label.map(str::to_owned),
            }
        }
        #[cfg(not(feature = "opengl"))]
        {
            let _ = label;
            Self {
                buffer_id: 0,
                size: 0,
                debug_label: label.map(str::to_owned),
            }
        }
    }

    /// Returns the current allocated size on the GPU (bytes).
    pub fn size(&self) -> usize {
        self.size
    }

    /// Returns the raw GL buffer ID.
    pub fn buffer_id(&self) -> u32 {
        self.buffer_id
    }

    /// Upload `data` into the buffer.
    ///
    /// If the size changed since the last call, the GPU-side storage is
    /// reallocated (`glBufferData`).  Otherwise only a sub-range update is
    /// performed (`glBufferSubData`).  Mirrors `GlfUniformBlock::Update()`.
    pub fn update(&mut self, data: &[u8]) {
        #[cfg(feature = "opengl")]
        {
            if self.buffer_id == 0 {
                return;
            }
            let new_size = data.len();
            unsafe {
                gl::BindBuffer(gl::UNIFORM_BUFFER, self.buffer_id);

                if self.size != new_size {
                    // Reallocate; pass NULL first so the driver can discard old data.
                    gl::BufferData(
                        gl::UNIFORM_BUFFER,
                        new_size as gl::types::GLsizeiptr,
                        std::ptr::null(),
                        gl::STATIC_DRAW,
                    );
                    self.size = new_size;
                }

                if !data.is_empty() {
                    // Bug 95969: BufferSubData with size 0 raises errors on some
                    // NVIDIA drivers — guard against it explicitly.
                    gl::BufferSubData(
                        gl::UNIFORM_BUFFER,
                        0,
                        new_size as gl::types::GLsizeiptr,
                        data.as_ptr() as *const _,
                    );
                }

                gl::BindBuffer(gl::UNIFORM_BUFFER, 0);
            }
        }
        #[cfg(not(feature = "opengl"))]
        {
            self.size = data.len();
        }
    }

    /// Bind this buffer to the uniform block binding point identified by
    /// `identifier` in `binding_map`.
    ///
    /// Mirrors `GlfUniformBlock::Bind()`.  The binding point number is
    /// allocated by the map on first use.
    pub fn bind(&self, binding_map: &mut GlfBindingMap, identifier: &str) {
        #[cfg(feature = "opengl")]
        {
            if self.buffer_id == 0 {
                return;
            }
            let token = usd_tf::Token::new(identifier);
            let binding = binding_map.get_uniform_binding(&token) as u32;

            unsafe {
                gl::BindBufferBase(gl::UNIFORM_BUFFER, binding, self.buffer_id);
            }

            // Set debug label now that the buffer is definitively created.
            if let Some(ref label) = self.debug_label {
                crate::diagnostic::debug_label_buffer(self.buffer_id, label);
            }
        }
        #[cfg(not(feature = "opengl"))]
        {
            let _ = binding_map;
            let _ = identifier;
        }
    }
}

impl Drop for GlfUniformBlock {
    fn drop(&mut self) {
        #[cfg(feature = "opengl")]
        if self.buffer_id != 0 {
            unsafe {
                gl::DeleteBuffers(1, &self.buffer_id);
            }
        }
    }
}

impl std::fmt::Debug for GlfUniformBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlfUniformBlock")
            .field("buffer_id", &self.buffer_id)
            .field("size", &self.size)
            .field("label", &self.debug_label)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_no_context() {
        // Without a real GL context the buffer_id may be 0 or non-zero
        // depending on the driver; what matters is no panic.
        let ubo = GlfUniformBlock::new(Some("test"));
        assert_eq!(ubo.size(), 0);
    }

    #[test]
    fn test_update_no_context() {
        let mut ubo = GlfUniformBlock::new(None);
        let data = [0u8; 64];
        // Without opengl feature this just records the size.
        ubo.update(&data);
        #[cfg(not(feature = "opengl"))]
        assert_eq!(ubo.size(), 64);
    }
}
