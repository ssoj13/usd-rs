//! Compute pipeline state descriptors

use super::handle::HgiHandle;
use super::shader_program::HgiShaderProgramHandle;

/// Describes the push/function constant buffer layout for compute shaders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HgiComputeShaderConstantsDesc {
    /// Size of the push constants block in bytes
    pub byte_size: u32,
}

impl HgiComputeShaderConstantsDesc {
    /// Create a new shader constants descriptor
    pub fn new(byte_size: u32) -> Self {
        Self { byte_size }
    }
}

/// Describes a compute pipeline state
#[derive(Debug, Clone)]
pub struct HgiComputePipelineDesc {
    /// Debug label for GPU debugging
    pub debug_name: String,

    /// Shader program (must contain a compute shader)
    pub shader_program: HgiShaderProgramHandle,

    /// Push/function constants descriptor
    pub shader_constants_desc: HgiComputeShaderConstantsDesc,
}

impl Default for HgiComputePipelineDesc {
    fn default() -> Self {
        Self {
            debug_name: String::new(),
            shader_program: HgiHandle::null(),
            shader_constants_desc: HgiComputeShaderConstantsDesc::default(),
        }
    }
}

impl HgiComputePipelineDesc {
    /// Create a new compute pipeline descriptor
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the debug name
    pub fn with_debug_name(mut self, name: impl Into<String>) -> Self {
        self.debug_name = name.into();
        self
    }

    /// Set the shader program
    pub fn with_shader_program(mut self, program: HgiShaderProgramHandle) -> Self {
        self.shader_program = program;
        self
    }

    /// Set the shader constants descriptor
    pub fn with_shader_constants(mut self, desc: HgiComputeShaderConstantsDesc) -> Self {
        self.shader_constants_desc = desc;
        self
    }

    /// Check if this is a valid descriptor
    pub fn is_valid(&self) -> bool {
        self.shader_program.is_valid()
    }
}

impl PartialEq for HgiComputePipelineDesc {
    fn eq(&self, other: &Self) -> bool {
        self.debug_name == other.debug_name
            && self.shader_program == other.shader_program
            && self.shader_constants_desc == other.shader_constants_desc
    }
}

/// GPU compute pipeline state object (abstract interface)
///
/// Represents a graphics platform independent compute pipeline.
/// Compute pipelines should be created via Hgi::create_compute_pipeline().
pub trait HgiComputePipeline: Send + Sync {
    /// Downcast to concrete type (for backend-specific operations)
    fn as_any(&self) -> &dyn std::any::Any;

    /// Get the descriptor that was used to create this compute pipeline
    fn descriptor(&self) -> &HgiComputePipelineDesc;

    /// Returns the backend's raw GPU resource handle
    ///
    /// Platform-specific return values:
    /// - OpenGL: returns 0 (OpenGL doesn't have compute pipeline objects)
    /// - Metal: returns the id<MTLComputePipelineState> as u64
    /// - Vulkan: returns the VkPipeline as u64
    /// - DX12: returns the ID3D12PipelineState pointer as u64
    fn raw_resource(&self) -> u64;
}

/// Type alias for compute pipeline handle
pub type HgiComputePipelineHandle = HgiHandle<dyn HgiComputePipeline>;

/// Vector of compute pipeline handles
pub type HgiComputePipelineHandleVector = Vec<HgiComputePipelineHandle>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_pipeline_desc_default() {
        let desc = HgiComputePipelineDesc::default();
        assert!(desc.debug_name.is_empty());
        assert!(!desc.is_valid());
    }

    #[test]
    fn test_compute_pipeline_desc_builder() {
        let desc = HgiComputePipelineDesc::new().with_debug_name("MyComputePipeline");

        assert_eq!(desc.debug_name, "MyComputePipeline");
    }

    // Mock implementation for testing
    struct MockComputePipeline {
        desc: HgiComputePipelineDesc,
    }

    impl HgiComputePipeline for MockComputePipeline {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn descriptor(&self) -> &HgiComputePipelineDesc {
            &self.desc
        }

        fn raw_resource(&self) -> u64 {
            0
        }
    }

    #[test]
    fn test_compute_pipeline_trait() {
        let desc = HgiComputePipelineDesc::new().with_debug_name("TestPipeline");
        let pipeline = MockComputePipeline { desc };

        assert_eq!(pipeline.descriptor().debug_name, "TestPipeline");
        assert_eq!(pipeline.raw_resource(), 0);
    }
}
