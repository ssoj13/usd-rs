//! Metal shader program. Port of pxr/imaging/hgiMetal/shaderProgram

use usd_hgi::{HgiShaderFunctionHandleVector, HgiShaderProgram, HgiShaderProgramDesc};

/// Metal shader program (linked vertex + fragment/compute).
/// Mirrors C++ HgiMetalShaderProgram.
#[derive(Debug)]
pub struct HgiMetalShaderProgram {
    desc: HgiShaderProgramDesc,
    errors: String,
    shader_functions: HgiShaderFunctionHandleVector,
    // On real Metal:
    // vertex_function: id<MTLFunction>,
    // fragment_function: id<MTLFunction>,
    // compute_function: id<MTLFunction>,
    // post_tess_vertex_function: id<MTLFunction>,
    // post_tess_control_function: id<MTLFunction>,
}

impl HgiMetalShaderProgram {
    /// Creates a new Metal shader program from the given descriptor.
    /// On real Metal, this would extract functions from the shader function handles.
    pub fn new(desc: HgiShaderProgramDesc) -> Self {
        let shader_functions: HgiShaderFunctionHandleVector = desc.shader_functions.clone();
        Self {
            desc,
            errors: String::new(),
            shader_functions,
        }
    }

    /// Returns the vertex function handle.
    /// Mirrors C++ GetVertexFunction().
    /// Stub: returns 0.
    pub fn get_vertex_function(&self) -> u64 {
        0
    }

    /// Returns the fragment function handle.
    /// Mirrors C++ GetFragmentFunction().
    /// Stub: returns 0.
    pub fn get_fragment_function(&self) -> u64 {
        0
    }

    /// Returns the compute function handle.
    /// Mirrors C++ GetComputeFunction().
    /// Stub: returns 0.
    pub fn get_compute_function(&self) -> u64 {
        0
    }

    /// Returns the post-tessellation vertex function handle.
    /// Mirrors C++ GetPostTessVertexFunction().
    /// Stub: returns 0.
    pub fn get_post_tess_vertex_function(&self) -> u64 {
        0
    }

    /// Returns the post-tessellation control function handle.
    /// Mirrors C++ GetPostTessControlFunction().
    /// Stub: returns 0.
    pub fn get_post_tess_control_function(&self) -> u64 {
        0
    }
}

impl HgiShaderProgram for HgiMetalShaderProgram {
    fn descriptor(&self) -> &HgiShaderProgramDesc {
        &self.desc
    }
    fn is_valid(&self) -> bool {
        // On real Metal, would check if all functions are non-nil
        false
    }
    fn link_errors(&self) -> &str {
        &self.errors
    }
    fn byte_size_of_resource(&self) -> usize {
        0
    }
    fn raw_resource(&self) -> u64 {
        0
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
