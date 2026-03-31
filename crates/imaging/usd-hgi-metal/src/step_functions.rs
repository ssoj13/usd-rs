//! Vertex buffer step functions for multi-draw indirect.
//! Port of pxr/imaging/hgiMetal/stepFunctions

use usd_hgi::{HgiGraphicsPipelineDesc, HgiVertexBufferStepFunction};

/// Parameters for a single vertex buffer step function.
/// Mirrors C++ HgiMetalStepFunctionDesc.
#[derive(Debug, Clone)]
pub struct HgiMetalStepFunctionDesc {
    /// Vertex buffer binding index
    pub binding_index: u32,
    /// Byte offset into the vertex buffer
    pub byte_offset: u32,
    /// Stride between vertices in bytes
    pub vertex_stride: u32,
}

impl HgiMetalStepFunctionDesc {
    pub fn new(binding_index: u32, byte_offset: u32, vertex_stride: u32) -> Self {
        Self {
            binding_index,
            byte_offset,
            vertex_stride,
        }
    }
}

/// Manages vertex buffer step functions for multi-draw indirect on Metal.
///
/// Metal does not support vertex attrib divisors, so per-draw-command
/// vertex attributes use constant step function with explicit offset
/// advancement. Similarly, per-patch-control-point attributes need
/// explicit offset management.
///
/// Mirrors C++ HgiMetalStepFunctions.
#[derive(Debug, Clone)]
pub struct HgiMetalStepFunctions {
    vertex_buffer_descs: Vec<HgiMetalStepFunctionDesc>,
    patch_base_descs: Vec<HgiMetalStepFunctionDesc>,
    draw_buffer_index: u32,
}

impl Default for HgiMetalStepFunctions {
    fn default() -> Self {
        Self::new()
    }
}

impl HgiMetalStepFunctions {
    /// Create empty step functions. Mirrors C++ default constructor.
    pub fn new() -> Self {
        Self {
            vertex_buffer_descs: Vec::new(),
            patch_base_descs: Vec::new(),
            draw_buffer_index: 0,
        }
    }

    /// Create step functions from a graphics pipeline descriptor and bindings.
    /// Mirrors C++ HgiMetalStepFunctions(graphicsDesc, bindings).
    pub fn from_desc(
        graphics_desc: &HgiGraphicsPipelineDesc,
        bindings: &[usd_hgi::HgiVertexBufferBinding],
    ) -> Self {
        let mut result = Self::new();
        result.init(graphics_desc);
        result.bind(bindings);
        result
    }

    /// Initialize step function descriptors from pipeline vertex buffer layouts.
    /// Mirrors C++ Init().
    pub fn init(&mut self, graphics_desc: &HgiGraphicsPipelineDesc) {
        self.vertex_buffer_descs.clear();
        self.patch_base_descs.clear();

        for (index, vb_desc) in graphics_desc.vertex_buffers.iter().enumerate() {
            match vb_desc.step_function {
                HgiVertexBufferStepFunction::PerDrawCommand => {
                    self.vertex_buffer_descs.push(HgiMetalStepFunctionDesc::new(
                        index as u32,
                        0,
                        vb_desc.vertex_stride,
                    ));
                }
                HgiVertexBufferStepFunction::PerPatchControlPoint => {
                    self.patch_base_descs.push(HgiMetalStepFunctionDesc::new(
                        index as u32,
                        0,
                        vb_desc.vertex_stride,
                    ));
                }
                _ => {}
            }
        }
    }

    /// Bind vertex buffer offsets from bindings.
    /// Mirrors C++ Bind().
    pub fn bind(&mut self, bindings: &[usd_hgi::HgiVertexBufferBinding]) {
        for desc in &mut self.vertex_buffer_descs {
            if let Some(binding) = bindings.iter().find(|b| b.index == desc.binding_index) {
                desc.byte_offset = binding.byte_offset;
            }
        }
        for desc in &mut self.patch_base_descs {
            if let Some(binding) = bindings.iter().find(|b| b.index == desc.binding_index) {
                desc.byte_offset = binding.byte_offset;
            }
        }
        self.draw_buffer_index = bindings.len() as u32;
    }

    /// Set vertex buffer offsets on a render encoder for per-draw-command attributes.
    /// Mirrors C++ SetVertexBufferOffsets().
    /// Stub: requires Metal render command encoder.
    pub fn set_vertex_buffer_offsets(&self, _base_instance: u32) {
        // Stub: on real Metal, calls setVertexBufferOffset for each desc
        for _desc in &self.vertex_buffer_descs {
            // encoder.setVertexBufferOffset(
            //     desc.byte_offset + desc.vertex_stride * base_instance,
            //     desc.binding_index
            // )
        }
    }

    /// Set patch base offsets on a render encoder.
    /// Mirrors C++ SetPatchBaseOffsets().
    /// Stub: requires Metal render command encoder.
    pub fn set_patch_base_offsets(&self, _base_instance: u32) {
        // Stub: on real Metal, calls setVertexBufferOffset for each patch desc
        for _desc in &self.patch_base_descs {
            // encoder.setVertexBufferOffset(
            //     desc.byte_offset + desc.vertex_stride * base_instance,
            //     desc.binding_index
            // )
        }
    }

    /// Returns the patch base step function descriptors.
    /// Mirrors C++ GetPatchBaseDescs().
    pub fn patch_base_descs(&self) -> &[HgiMetalStepFunctionDesc] {
        &self.patch_base_descs
    }

    /// Returns the draw buffer binding index.
    /// Mirrors C++ GetDrawBufferIndex().
    pub fn draw_buffer_index(&self) -> u32 {
        self.draw_buffer_index
    }
}
