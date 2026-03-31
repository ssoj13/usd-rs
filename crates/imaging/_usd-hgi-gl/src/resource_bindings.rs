//! OpenGL resource bindings implementation.
//!
//! Port of pxr/imaging/hgiGL/resourceBindings.h/cpp

use usd_hgi::*;

#[cfg(feature = "opengl")]
use gl::types::*;

/// OpenGL implementation of HgiResourceBindings.
///
/// Manages binding of textures, samplers, images, and buffers to GPU.
/// Port of HgiGLResourceBindings from pxr/imaging/hgiGL/resourceBindings.h
#[derive(Debug)]
pub struct HgiGLResourceBindings {
    /// Resource bindings descriptor.
    desc: HgiResourceBindingsDesc,
}

impl HgiGLResourceBindings {
    /// Create new resource bindings from descriptor.
    pub fn new(desc: &HgiResourceBindingsDesc) -> Self {
        Self { desc: desc.clone() }
    }

    /// Get the resource bindings descriptor.
    pub fn descriptor(&self) -> &HgiResourceBindingsDesc {
        &self.desc
    }

    /// Bind resources to GPU.
    ///
    /// Binds all textures, samplers, images, and buffers described
    /// in the descriptor to their respective binding points.
    #[cfg(feature = "opengl")]
    pub fn bind_resources(&self) {
        // Pre-allocate vectors for batch binding
        let tex_count = self.desc.texture_bindings.len();
        let mut textures: Vec<GLuint> = vec![0; tex_count];
        let mut samplers: Vec<GLuint> = vec![0; tex_count];
        let mut images: Vec<GLuint> = vec![0; tex_count];

        let mut has_tex = false;
        let mut has_sampler = false;
        let mut has_image = false;

        // Bind Textures, images and samplers
        for tex_desc in &self.desc.texture_bindings {
            // OpenGL does not support arrays-of-textures bound to a unit.
            // (Which is different from texture-arrays. See Vulkan/Metal)
            // We use the specified binding index for the first texture in a bind
            // desc, then increment by one for each subsequent.

            let unit = tex_desc.binding_index as usize + tex_desc.textures.len();
            if textures.len() < unit {
                textures.resize(unit, 0);
                samplers.resize(unit, 0);
                images.resize(unit, 0);
            }

            match tex_desc.resource_type {
                HgiBindResourceType::SampledImage | HgiBindResourceType::CombinedSamplerImage => {
                    // Texture sampling (for graphics pipeline)
                    has_tex = true;
                    let mut binding_idx = tex_desc.binding_index as usize;
                    for tex_handle in &tex_desc.textures {
                        if let Some(tex) = tex_handle.get() {
                            textures[binding_idx] = tex.raw_resource() as GLuint;
                        }
                        binding_idx += 1;
                    }
                }
                HgiBindResourceType::StorageImage => {
                    // Image load/store (usually for compute pipeline)
                    has_image = true;
                    let mut binding_idx = tex_desc.binding_index as usize;
                    for tex_handle in &tex_desc.textures {
                        if let Some(tex) = tex_handle.get() {
                            images[binding_idx] = tex.raw_resource() as GLuint;
                        }
                        binding_idx += 1;
                    }
                }
                _ => {
                    log::error!("Unsupported texture bind resource type");
                }
            }

            // 'StorageImage' types do not need a sampler, so check if we have one.
            if !tex_desc.samplers.is_empty() {
                has_sampler = true;
                let mut binding_idx = tex_desc.binding_index as usize;
                for smp_handle in &tex_desc.samplers {
                    if let Some(smp) = smp_handle.get() {
                        samplers[binding_idx] = smp.raw_resource() as GLuint;
                    }
                    binding_idx += 1;
                }
            }
        }

        unsafe {
            if has_tex && !textures.is_empty() {
                gl::BindTextures(0, textures.len() as GLsizei, textures.as_ptr());
            }

            if has_sampler && !samplers.is_empty() {
                gl::BindSamplers(0, samplers.len() as GLsizei, samplers.as_ptr());
            }

            // 'texture units' are separate from 'texture image units' in OpenGL.
            // glBindImageTextures should not reset textures bound with glBindTextures.
            if has_image && !images.is_empty() {
                gl::BindImageTextures(0, images.len() as GLsizei, images.as_ptr());
            }
        }

        // Bind Buffers
        for buf_desc in &self.desc.buffer_bindings {
            // OpenGL does not support arrays-of-buffers bound to a unit.
            if buf_desc.buffers.len() != 1 {
                log::warn!("OpenGL requires exactly one buffer per binding");
                continue;
            }

            if buf_desc.buffers.len() != buf_desc.offsets.len() {
                log::error!("Invalid number of buffer offsets");
                continue;
            }

            if !buf_desc.sizes.is_empty() && buf_desc.buffers.len() != buf_desc.sizes.len() {
                log::error!("Invalid number of buffer sizes");
                continue;
            }

            let buf_handle = &buf_desc.buffers[0];
            let Some(buffer) = buf_handle.get() else {
                continue;
            };

            let buffer_id = buffer.raw_resource() as GLuint;
            let offset = buf_desc.offsets[0] as GLintptr;
            let size = if buf_desc.sizes.is_empty() {
                0
            } else {
                buf_desc.sizes[0] as GLsizeiptr
            };
            let binding_index = buf_desc.binding_index;

            if offset != 0 && size == 0 {
                log::error!("Invalid size for buffer with offset");
                continue;
            }

            let target = match buf_desc.resource_type {
                HgiBindResourceType::UniformBuffer => gl::UNIFORM_BUFFER,
                HgiBindResourceType::StorageBuffer => gl::SHADER_STORAGE_BUFFER,
                _ => {
                    log::error!("Unknown buffer type to bind");
                    continue;
                }
            };

            unsafe {
                if size != 0 {
                    gl::BindBufferRange(target, binding_index, buffer_id, offset, size);
                } else {
                    gl::BindBufferBase(target, binding_index, buffer_id);
                }
            }
        }
    }

    /// Bind resources to GPU (no-op when opengl feature disabled).
    #[cfg(not(feature = "opengl"))]
    pub fn bind_resources(&self) {
        // Note: No-op when OpenGL not compiled in
    }
}

impl HgiResourceBindings for HgiGLResourceBindings {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiResourceBindingsDesc {
        &self.desc
    }

    fn raw_resource(&self) -> u64 {
        // OpenGL doesn't have a single resource ID for bindings
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_bindings_creation() {
        let desc = HgiResourceBindingsDesc::new();
        let bindings = HgiGLResourceBindings::new(&desc);
        assert!(bindings.descriptor().texture_bindings.is_empty());
        assert!(bindings.descriptor().buffer_bindings.is_empty());
    }
}
