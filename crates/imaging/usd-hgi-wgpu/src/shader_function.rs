//! wgpu shader function (shader module) wrapper

use usd_hgi::shader_function::{HgiShaderFunction, HgiShaderFunctionDesc};

/// wgpu shader function wrapping a wgpu::ShaderModule.
///
/// In wgpu, shaders are compiled from WGSL (or SPIR-V via naga).
/// The shader_code in the descriptor is expected to be WGSL source.
pub struct WgpuShaderFunction {
    desc: HgiShaderFunctionDesc,
    module: wgpu::ShaderModule,
    compile_error: String,
    valid: bool,
}

impl WgpuShaderFunction {
    /// Create a new wgpu shader function from an HGI descriptor.
    ///
    /// Compiles the shader code as WGSL and validates it using error scopes.
    /// Compilation errors are captured and stored for later retrieval.
    pub fn new(device: &wgpu::Device, desc: &HgiShaderFunctionDesc) -> Self {
        device.push_error_scope(wgpu::ErrorFilter::Validation);

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: if desc.debug_name.is_empty() {
                None
            } else {
                Some(&desc.debug_name)
            },
            source: wgpu::ShaderSource::Wgsl(desc.shader_code.clone().into()),
        });

        let error = pollster::block_on(device.pop_error_scope());
        let (compile_error, valid) = if let Some(err) = error {
            (err.to_string(), false)
        } else {
            (String::new(), true)
        };

        Self {
            desc: desc.clone(),
            module,
            compile_error,
            valid,
        }
    }

    /// Direct access to the underlying wgpu::ShaderModule.
    pub fn wgpu_module(&self) -> &wgpu::ShaderModule {
        &self.module
    }
}

impl HgiShaderFunction for WgpuShaderFunction {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiShaderFunctionDesc {
        &self.desc
    }

    fn is_valid(&self) -> bool {
        self.valid
    }

    fn compile_errors(&self) -> &str {
        &self.compile_error
    }

    fn byte_size_of_resource(&self) -> usize {
        self.desc.shader_code.len()
    }

    fn raw_resource(&self) -> u64 {
        0
    }
}
