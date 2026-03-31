//! Indirect command encoder for GPU-driven rendering
//!
//! Mirrors C++ HgiIndirectCommandEncoder from indirectCommandEncoder.h.
//! Records draw commands into indirect buffers for efficient batch replay.

use super::buffer::HgiBufferHandle;
use super::graphics_cmds::HgiGraphicsCmds;
use super::graphics_pipeline::HgiGraphicsPipelineHandle;
use super::resource_bindings::HgiResourceBindingsHandle;

/// Holds the state needed to replay an indirect draw batch
pub struct HgiIndirectCommands {
    /// Number of draw commands in the batch
    pub draw_count: u32,
    /// Graphics pipeline to bind
    pub graphics_pipeline: HgiGraphicsPipelineHandle,
    /// Resource bindings to bind
    pub resource_bindings: HgiResourceBindingsHandle,
}

impl HgiIndirectCommands {
    /// Create a new indirect commands batch
    pub fn new(
        draw_count: u32,
        graphics_pipeline: HgiGraphicsPipelineHandle,
        resource_bindings: HgiResourceBindingsHandle,
    ) -> Self {
        Self {
            draw_count,
            graphics_pipeline,
            resource_bindings,
        }
    }
}

/// Vertex buffer binding for indirect draw commands
#[derive(Debug, Clone)]
pub struct HgiVertexBufferBinding {
    /// Buffer handle
    pub buffer: HgiBufferHandle,
    /// Byte offset into the buffer
    pub byte_offset: u32,
    /// Binding index
    pub index: u32,
}

/// Abstract encoder for recording indirect draw command batches.
///
/// Stores draw params + resource bindings in HgiIndirectCommands for efficient
/// replay via ExecuteDraw. Currently used primarily on Metal.
pub trait HgiIndirectCommandEncoder: Send + Sync {
    /// Encode a batch of non-indexed draw commands from the draw parameter buffer.
    ///
    /// Returns an HgiIndirectCommands holding all state needed to replay.
    fn encode_draw(
        &mut self,
        pipeline: &HgiGraphicsPipelineHandle,
        resource_bindings: &HgiResourceBindingsHandle,
        vertex_bindings: &[HgiVertexBufferBinding],
        draw_parameter_buffer: &HgiBufferHandle,
        draw_buffer_byte_offset: u32,
        draw_count: u32,
        stride: u32,
    ) -> Box<HgiIndirectCommands>;

    /// Encode a batch of indexed draw commands from the draw parameter buffer.
    ///
    /// Returns an HgiIndirectCommands holding all state needed to replay.
    fn encode_draw_indexed(
        &mut self,
        pipeline: &HgiGraphicsPipelineHandle,
        resource_bindings: &HgiResourceBindingsHandle,
        vertex_bindings: &[HgiVertexBufferBinding],
        index_buffer: &HgiBufferHandle,
        draw_parameter_buffer: &HgiBufferHandle,
        draw_buffer_byte_offset: u32,
        draw_count: u32,
        stride: u32,
        patch_base_vertex_byte_offset: u32,
    ) -> Box<HgiIndirectCommands>;

    /// Execute an indirect command batch on the given graphics command buffer.
    ///
    /// Replays the draws recorded in `commands` via `gfx_cmds`.
    /// Matches C++ `ExecuteDraw(HgiGraphicsCmds*, HgiIndirectCommands const*)`.
    fn execute_draw(&mut self, gfx_cmds: &mut dyn HgiGraphicsCmds, commands: &HgiIndirectCommands);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handle::HgiHandle;

    #[test]
    fn test_indirect_commands() {
        let cmds = HgiIndirectCommands::new(10, HgiHandle::null(), HgiHandle::null());
        assert_eq!(cmds.draw_count, 10);
    }
}
