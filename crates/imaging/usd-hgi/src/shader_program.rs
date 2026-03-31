//! Shader program resources and descriptors

use super::handle::HgiHandle;
use super::shader_function::HgiShaderFunctionHandle;

/// Describes the properties needed to create a shader program
///
/// A shader program links together multiple shader functions (e.g., vertex + fragment).
#[derive(Debug, Clone)]
pub struct HgiShaderProgramDesc {
    /// Debug label for GPU debugging
    pub debug_name: String,

    /// Shader functions to link together
    pub shader_functions: Vec<HgiShaderFunctionHandle>,
}

impl Default for HgiShaderProgramDesc {
    fn default() -> Self {
        Self {
            debug_name: String::new(),
            shader_functions: Vec::new(),
        }
    }
}

impl HgiShaderProgramDesc {
    /// Create a new shader program descriptor
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the debug name
    pub fn with_debug_name(mut self, name: impl Into<String>) -> Self {
        self.debug_name = name.into();
        self
    }

    /// Add a shader function
    pub fn with_shader_function(mut self, function: HgiShaderFunctionHandle) -> Self {
        self.shader_functions.push(function);
        self
    }

    /// Set all shader functions at once
    pub fn with_shader_functions(mut self, functions: Vec<HgiShaderFunctionHandle>) -> Self {
        self.shader_functions = functions;
        self
    }

    /// Check if this is a valid descriptor
    pub fn is_valid(&self) -> bool {
        !self.shader_functions.is_empty() && self.shader_functions.iter().all(|f| f.is_valid())
    }
}

impl PartialEq for HgiShaderProgramDesc {
    fn eq(&self, other: &Self) -> bool {
        self.debug_name == other.debug_name && self.shader_functions == other.shader_functions
    }
}

/// GPU shader program resource (abstract interface)
///
/// Represents a graphics platform independent GPU shader program.
/// Shader programs should be created via Hgi::create_shader_program().
pub trait HgiShaderProgram: Send + Sync {
    /// Downcast to concrete type (for backend-specific operations)
    fn as_any(&self) -> &dyn std::any::Any;

    /// Get the descriptor that was used to create this shader program
    fn descriptor(&self) -> &HgiShaderProgramDesc;

    /// Returns whether the program linked successfully
    fn is_valid(&self) -> bool;

    /// Returns the shader program link errors (if any)
    fn link_errors(&self) -> &str;

    /// Returns the byte size of the shader program
    fn byte_size_of_resource(&self) -> usize;

    /// Returns the backend's raw GPU resource handle
    ///
    /// Platform-specific return values:
    /// - OpenGL: returns the GLuint program object
    /// - Metal: returns the id<MTLRenderPipelineState> or id<MTLComputePipelineState> as u64
    /// - Vulkan: returns the VkPipeline as u64
    /// - DX12: returns the ID3D12PipelineState pointer as u64
    fn raw_resource(&self) -> u64;
}

/// Type alias for shader program handle
pub type HgiShaderProgramHandle = HgiHandle<dyn HgiShaderProgram>;

/// Vector of shader program handles
pub type HgiShaderProgramHandleVector = Vec<HgiShaderProgramHandle>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_program_desc_default() {
        let desc = HgiShaderProgramDesc::default();
        assert!(desc.shader_functions.is_empty());
        assert!(!desc.is_valid());
    }

    #[test]
    fn test_shader_program_desc_basic() {
        let desc = HgiShaderProgramDesc::new().with_debug_name("MyProgram");

        assert_eq!(desc.debug_name, "MyProgram");
    }

    // Mock shader program for testing
    struct MockShaderProgram {
        desc: HgiShaderProgramDesc,
    }

    impl HgiShaderProgram for MockShaderProgram {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn descriptor(&self) -> &HgiShaderProgramDesc {
            &self.desc
        }
        fn is_valid(&self) -> bool {
            true
        }
        fn link_errors(&self) -> &str {
            ""
        }
        fn byte_size_of_resource(&self) -> usize {
            200
        }
        fn raw_resource(&self) -> u64 {
            0
        }
    }

    #[test]
    fn test_shader_program_trait() {
        let desc = HgiShaderProgramDesc::new().with_debug_name("TestProgram");
        let program = MockShaderProgram { desc };

        assert!(program.is_valid());
        assert_eq!(program.link_errors(), "");
        assert_eq!(program.descriptor().debug_name, "TestProgram");
    }
}
