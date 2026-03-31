//! wgpu shader program wrapper
//!
//! In wgpu, there is no separate "program" object like in OpenGL.
//! Shader modules are bound directly to pipeline descriptors.
//! This wrapper exists to satisfy the HGI trait interface.

use usd_hgi::shader_program::{HgiShaderProgram, HgiShaderProgramDesc};

/// wgpu shader program -- logical grouping of shader functions.
///
/// wgpu doesn't have a link step; shaders are bound at pipeline creation.
/// This struct holds the descriptor for HGI compatibility.
pub struct WgpuShaderProgram {
    desc: HgiShaderProgramDesc,
}

impl WgpuShaderProgram {
    /// Create a new shader program from an HGI descriptor.
    pub fn new(desc: &HgiShaderProgramDesc) -> Self {
        Self { desc: desc.clone() }
    }
}

impl HgiShaderProgram for WgpuShaderProgram {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiShaderProgramDesc {
        &self.desc
    }

    fn is_valid(&self) -> bool {
        !self.desc.shader_functions.is_empty()
    }

    fn link_errors(&self) -> &str {
        // No link step in wgpu
        ""
    }

    fn byte_size_of_resource(&self) -> usize {
        0
    }

    fn raw_resource(&self) -> u64 {
        0
    }
}
