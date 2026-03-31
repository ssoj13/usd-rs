//! Metal shader function. Port of pxr/imaging/hgiMetal/shaderFunction

use usd_hgi::{HgiShaderFunction, HgiShaderFunctionDesc};

/// Metal shader function (vertex/fragment/compute entry point).
/// Mirrors C++ HgiMetalShaderFunction.
#[derive(Debug)]
pub struct HgiMetalShaderFunction {
    desc: HgiShaderFunctionDesc,
    errors: String,
    // On real Metal: shader_id: id<MTLFunction>
}

impl HgiMetalShaderFunction {
    /// Creates a new Metal shader function from the given descriptor.
    /// On real Metal, this would compile MSL source and create an MTLFunction.
    pub fn new(desc: HgiShaderFunctionDesc) -> Self {
        Self {
            desc,
            errors: String::new(),
        }
    }

    /// Returns the Metal function handle.
    /// Mirrors C++ GetShaderId().
    /// Stub: returns 0 (no real Metal function).
    pub fn get_shader_id(&self) -> u64 {
        0
    }
}

impl HgiShaderFunction for HgiMetalShaderFunction {
    fn descriptor(&self) -> &HgiShaderFunctionDesc {
        &self.desc
    }
    fn is_valid(&self) -> bool {
        // On real Metal, would check if shader_id is non-nil
        false
    }
    fn compile_errors(&self) -> &str {
        &self.errors
    }
    fn byte_size_of_resource(&self) -> usize {
        0
    }
    fn raw_resource(&self) -> u64 {
        self.get_shader_id()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
