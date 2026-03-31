//! Graphics command buffer interface

use super::buffer::HgiBufferHandle;
use super::cmds::HgiCmds;
use super::enums::{HgiMemoryBarrier, HgiShaderStage};
use super::graphics_pipeline::HgiGraphicsPipelineHandle;
use super::resource_bindings::HgiResourceBindingsHandle;
use super::sampler::HgiSamplerHandle;
use super::texture::HgiTextureHandle;
use usd_gf::Vec4f;

/// Describes a viewport
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HgiViewport {
    /// X offset
    pub x: f32,
    /// Y offset
    pub y: f32,
    /// Width
    pub width: f32,
    /// Height
    pub height: f32,
    /// Min depth
    pub min_depth: f32,
    /// Max depth
    pub max_depth: f32,
}

impl HgiViewport {
    /// Creates a viewport with default depth range [0.0, 1.0].
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            min_depth: 0.0,
            max_depth: 1.0,
        }
    }
}

/// Describes a scissor rectangle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HgiScissor {
    /// X offset
    pub x: i32,
    /// Y offset
    pub y: i32,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
}

impl HgiScissor {
    /// Creates a scissor rectangle from position and size.
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Draw command parameters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HgiDrawOp {
    /// Number of vertices to draw
    pub vertex_count: u32,
    /// Base vertex index
    pub base_vertex: u32,
    /// Number of instances
    pub instance_count: u32,
    /// Base instance
    pub base_instance: u32,
}

impl Default for HgiDrawOp {
    fn default() -> Self {
        Self {
            vertex_count: 0,
            base_vertex: 0,
            instance_count: 1,
            base_instance: 0,
        }
    }
}

/// Indexed draw command parameters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HgiDrawIndexedOp {
    /// Number of indices to draw
    pub index_count: u32,
    /// Base index
    pub base_index: u32,
    /// Base vertex (added to each index)
    pub base_vertex: i32,
    /// Number of instances
    pub instance_count: u32,
    /// Base instance
    pub base_instance: u32,
}

impl Default for HgiDrawIndexedOp {
    fn default() -> Self {
        Self {
            index_count: 0,
            base_index: 0,
            base_vertex: 0,
            instance_count: 1,
            base_instance: 0,
        }
    }
}

/// Indirect draw command parameters
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HgiDrawIndirectOp {
    /// Buffer containing draw commands
    pub draw_buffer: HgiBufferHandle,
    /// Offset into draw buffer
    pub draw_buffer_byte_offset: usize,
    /// Number of draws
    pub draw_count: u32,
    /// Stride between draw commands
    pub stride: u32,
}

/// Graphics command buffer for rendering operations
///
/// Used to record rendering commands that will be submitted to the GPU.
pub trait HgiGraphicsCmds: HgiCmds {
    /// Set the graphics pipeline state
    fn bind_pipeline(&mut self, pipeline: &HgiGraphicsPipelineHandle);

    /// Bind resource bindings (buffers, textures, samplers)
    fn bind_resources(&mut self, resources: &HgiResourceBindingsHandle);

    /// Set push constant / function constant values
    ///
    /// Equivalent to glUniform / vkCmdPushConstants / Metal setBytes.
    /// `pipeline` is the pipeline that you are binding before the draw call.
    /// `stages` describes for what shader stage(s) you are setting the push constant values.
    /// `bind_index` is the binding point index in the pipeline's shader.
    /// `data` is the data you are copying into the push constants block.
    fn set_constant_values(
        &mut self,
        _pipeline: &HgiGraphicsPipelineHandle,
        _stages: HgiShaderStage,
        _bind_index: u32,
        _data: &[u8],
    ) {
        // Default: no-op. Backends override for push constant support.
    }

    /// Bind vertex buffers
    fn bind_vertex_buffers(&mut self, buffers: &[HgiBufferHandle], offsets: &[u64]);

    /// Set viewport
    fn set_viewport(&mut self, viewport: &HgiViewport);

    /// Set scissor rectangle
    fn set_scissor(&mut self, scissor: &HgiScissor);

    /// Set blend constant color
    fn set_blend_constant_color(&mut self, color: &Vec4f);

    /// Set stencil reference value
    fn set_stencil_reference_value(&mut self, value: u32);

    /// Draw primitives
    fn draw(&mut self, op: &HgiDrawOp);

    /// Draw indexed primitives
    fn draw_indexed(&mut self, index_buffer: &HgiBufferHandle, op: &HgiDrawIndexedOp);

    /// Draw using indirect buffer
    fn draw_indirect(&mut self, op: &HgiDrawIndirectOp);

    /// Draw indexed using indirect buffer
    fn draw_indexed_indirect(&mut self, index_buffer: &HgiBufferHandle, op: &HgiDrawIndirectOp);

    /// Insert a memory barrier
    fn memory_barrier(&mut self, barrier: HgiMemoryBarrier);

    /// Bind a texture+sampler bind group at a given group index.
    ///
    /// Used for group 3 (per-material textures). Each entry is a pair of
    /// (texture_handle, sampler_handle). The backend creates a bind group
    /// from these at submit time using the pipeline's layout for that group.
    ///
    /// Entries with null handles use a 1x1 white fallback texture.
    fn bind_texture_group(
        &mut self,
        _group_index: u32,
        _textures: &[HgiTextureHandle],
        _samplers: &[HgiSamplerHandle],
    ) {
        // Default: no-op. Override in wgpu backend.
    }

    /// Bind a storage buffer at a given group + binding index.
    ///
    /// Used for GPU instancing (instance transforms SSBO), pick/deep-resolve buffers,
    /// and other storage-buffer-backed paths.
    ///
    /// Read-only vs read-write access is defined by the pipeline's shader layout.
    /// The backend only needs to bind the buffer into the requested group/binding slot.
    fn bind_storage_buffer(&mut self, _group_index: u32, _binding: u32, _buffer: &HgiBufferHandle) {
        // Default: no-op. Override in wgpu backend.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport() {
        let viewport = HgiViewport::new(0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(viewport.width, 1920.0);
        assert_eq!(viewport.height, 1080.0);
        assert_eq!(viewport.min_depth, 0.0);
        assert_eq!(viewport.max_depth, 1.0);
    }

    #[test]
    fn test_scissor() {
        let scissor = HgiScissor::new(10, 20, 800, 600);
        assert_eq!(scissor.x, 10);
        assert_eq!(scissor.y, 20);
        assert_eq!(scissor.width, 800);
        assert_eq!(scissor.height, 600);
    }

    #[test]
    fn test_draw_op() {
        let op = HgiDrawOp {
            vertex_count: 3,
            base_vertex: 0,
            instance_count: 1,
            base_instance: 0,
        };
        assert_eq!(op.vertex_count, 3);
        assert_eq!(op.instance_count, 1);
    }

    #[test]
    fn test_draw_indexed_op() {
        let op = HgiDrawIndexedOp {
            index_count: 36,
            base_index: 0,
            base_vertex: 0,
            instance_count: 2,
            base_instance: 0,
        };
        assert_eq!(op.index_count, 36);
        assert_eq!(op.instance_count, 2);
    }
}
