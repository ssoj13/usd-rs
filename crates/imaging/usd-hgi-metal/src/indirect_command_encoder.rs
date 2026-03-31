//! Metal indirect command buffer encoder.
//! Port of pxr/imaging/hgiMetal/indirectCommandEncoder

use usd_hgi::{
    HgiBufferHandle, HgiGraphicsCmds, HgiGraphicsPipelineHandle, HgiIndirectCommandEncoder,
    HgiIndirectCommands, HgiResourceBindingsHandle, HgiVertexBufferBinding,
};

/// Metal implementation of indirect command buffers (ICB).
///
/// Encodes draw commands into Metal indirect command buffers for
/// efficient GPU-driven rendering. Only supported on Apple Silicon
/// with macOS 12.3+.
///
/// Mirrors C++ HgiMetalIndirectCommandEncoder.
pub struct HgiMetalIndirectCommandEncoder {
    // On real Metal these would be:
    // device: id<MTLDevice>,
    // library: id<MTLLibrary>,
    // functions: Vec<FunctionState>,
    // buffer_storage_mode: MTLResourceOptions,
    // triangle_tess_factors: id<MTLBuffer>,
    // quad_tess_factors: id<MTLBuffer>,
    // command_buffer_pool: BTreeMap<u32, Vec<...>>,
    // argument_buffer_pool: BTreeMap<u32, Vec<...>>,
    _placeholder: (),
}

impl HgiMetalIndirectCommandEncoder {
    /// Create a new indirect command encoder.
    /// Mirrors C++ HgiMetalIndirectCommandEncoder(Hgi* hgi).
    pub fn new() -> Self {
        Self { _placeholder: () }
    }
}

impl Default for HgiMetalIndirectCommandEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl HgiIndirectCommandEncoder for HgiMetalIndirectCommandEncoder {
    fn encode_draw(
        &mut self,
        _pipeline: &HgiGraphicsPipelineHandle,
        _resource_bindings: &HgiResourceBindingsHandle,
        _vertex_bindings: &[HgiVertexBufferBinding],
        _draw_parameter_buffer: &HgiBufferHandle,
        _draw_buffer_byte_offset: u32,
        _draw_count: u32,
        _stride: u32,
    ) -> Box<HgiIndirectCommands> {
        unimplemented!("HgiMetalIndirectCommandEncoder::encode_draw requires Metal API")
    }

    fn encode_draw_indexed(
        &mut self,
        _pipeline: &HgiGraphicsPipelineHandle,
        _resource_bindings: &HgiResourceBindingsHandle,
        _vertex_bindings: &[HgiVertexBufferBinding],
        _index_buffer: &HgiBufferHandle,
        _draw_parameter_buffer: &HgiBufferHandle,
        _draw_buffer_byte_offset: u32,
        _draw_count: u32,
        _stride: u32,
        _patch_base_vertex_byte_offset: u32,
    ) -> Box<HgiIndirectCommands> {
        unimplemented!("HgiMetalIndirectCommandEncoder::encode_draw_indexed requires Metal API")
    }

    fn execute_draw(
        &mut self,
        _gfx_cmds: &mut dyn HgiGraphicsCmds,
        _commands: &HgiIndirectCommands,
    ) {
        unimplemented!("HgiMetalIndirectCommandEncoder::execute_draw requires Metal API")
    }
}
