//! OpenGL compute pipeline implementation

use usd_hgi::*;

/// OpenGL compute pipeline state
///
/// In OpenGL, compute pipeline is essentially just a shader program.
/// This struct wraps the compute shader program and provides dispatch state.
#[derive(Debug)]
pub struct HgiGLComputePipeline {
    /// Pipeline descriptor
    desc: HgiComputePipelineDesc,
}

impl HgiGLComputePipeline {
    /// Create a new OpenGL compute pipeline
    pub fn new(desc: &HgiComputePipelineDesc) -> Self {
        Self { desc: desc.clone() }
    }

    /// Get the pipeline descriptor
    pub fn descriptor(&self) -> &HgiComputePipelineDesc {
        &self.desc
    }

    /// Get the compute shader program
    pub fn shader_program(&self) -> &HgiShaderProgramHandle {
        &self.desc.shader_program
    }
}

impl HgiComputePipeline for HgiGLComputePipeline {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiComputePipelineDesc {
        &self.desc
    }

    fn raw_resource(&self) -> u64 {
        // Return shader program ID if available
        if let Some(program) = self.desc.shader_program.get() {
            program.raw_resource()
        } else {
            0
        }
    }
}

/// Bind compute pipeline
#[cfg(feature = "opengl")]
pub fn bind_compute_pipeline(pipeline: &HgiGLComputePipeline) {
    if let Some(program) = pipeline.shader_program().get() {
        let program_id = program.raw_resource() as u32;
        if program_id != 0 {
            unsafe {
                gl::UseProgram(program_id);
            }
        }
    }
}

/// Bind compute pipeline (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn bind_compute_pipeline(_pipeline: &HgiGLComputePipeline) {}

/// Dispatch compute work
#[cfg(feature = "opengl")]
pub fn dispatch_compute(num_work_groups_x: u32, num_work_groups_y: u32, num_work_groups_z: u32) {
    unsafe {
        gl::DispatchCompute(num_work_groups_x, num_work_groups_y, num_work_groups_z);
    }
}

/// Dispatch compute work (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn dispatch_compute(_num_work_groups_x: u32, _num_work_groups_y: u32, _num_work_groups_z: u32) {
}

/// Dispatch compute work indirectly from a buffer
#[cfg(feature = "opengl")]
pub fn dispatch_compute_indirect(buffer_offset: usize) {
    unsafe {
        gl::DispatchComputeIndirect(buffer_offset as isize);
    }
}

/// Dispatch compute work indirectly (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn dispatch_compute_indirect(_buffer_offset: usize) {}

/// Insert a memory barrier for compute shader synchronization
#[cfg(feature = "opengl")]
pub fn memory_barrier(barriers: u32) {
    unsafe {
        gl::MemoryBarrier(barriers);
    }
}

/// Insert a memory barrier (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn memory_barrier(_barriers: u32) {}

/// Common barrier bits for compute shaders
pub mod barrier_bits {
    /// Barrier for shader storage buffer access
    pub const SHADER_STORAGE_BARRIER: u32 = 0x00002000; // GL_SHADER_STORAGE_BARRIER_BIT
    /// Barrier for image load/store
    pub const SHADER_IMAGE_ACCESS_BARRIER: u32 = 0x00000020; // GL_SHADER_IMAGE_ACCESS_BARRIER_BIT
    /// Barrier for texture fetch
    pub const TEXTURE_FETCH_BARRIER: u32 = 0x00000008; // GL_TEXTURE_FETCH_BARRIER_BIT
    /// Barrier for all operations
    pub const ALL_BARRIER: u32 = 0xFFFFFFFF; // GL_ALL_BARRIER_BITS
}

#[cfg(all(test, feature = "opengl"))]
pub(crate) fn run_gl_tests() {
    use super::*;

    let desc = HgiComputePipelineDesc::new().with_debug_name("TestComputePipeline".to_string());

    let pipeline = HgiGLComputePipeline::new(&desc);
    assert_eq!(pipeline.descriptor().debug_name, "TestComputePipeline");

    let desc = HgiComputePipelineDesc::new();
    let pipeline = HgiGLComputePipeline::new(&desc);

    // These are stubs, just verify they don't crash
    bind_compute_pipeline(&pipeline);
    dispatch_compute(1, 1, 1);
    dispatch_compute_indirect(0);
    memory_barrier(barrier_bits::ALL_BARRIER);
}
