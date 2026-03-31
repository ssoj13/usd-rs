//! Shader generator base for backend-specific shader code generation
//!
//! Mirrors C++ HgiShaderGenerator from shaderGenerator.h.
//! Converts GLSLFX domain language to concrete shader languages (GLSL, MSL, etc).

use super::enums::HgiShaderStage;
use super::shader_function::HgiShaderFunctionDesc;

/// Base trait for shader function generation.
///
/// Given a descriptor, converts glslfx domain language to concrete shader
/// languages. Can be extended for different APIs (GL, Metal, Vulkan).
/// Its main role is to make GLSLFX a write-once language, regardless of API.
pub trait HgiShaderGenerator: Send + Sync {
    /// Execute shader generation. Populates the generated code.
    fn execute(&mut self);

    /// Return generated shader source code
    fn generated_shader_code(&self) -> &str;

    /// Return the shader stage being generated
    fn shader_stage(&self) -> HgiShaderStage;

    /// Return the shader code declarations (defines/types before bindings)
    fn shader_code_declarations(&self) -> &str;

    /// Return the raw shader code body
    fn shader_code(&self) -> &str;
}

/// Default shader generator implementation that stores descriptor state.
///
/// Backends should embed this or re-implement the trait from scratch.
pub struct HgiShaderGeneratorBase {
    /// The descriptor driving code generation
    pub descriptor: HgiShaderFunctionDesc,
    /// Generated shader code output
    pub generated_code: String,
}

impl HgiShaderGeneratorBase {
    /// Create a new base generator from a descriptor
    pub fn new(descriptor: HgiShaderFunctionDesc) -> Self {
        Self {
            descriptor,
            generated_code: String::new(),
        }
    }
}

impl HgiShaderGenerator for HgiShaderGeneratorBase {
    fn execute(&mut self) {
        // Base implementation: pass-through shader code as-is
        self.generated_code.clear();
        self.generated_code
            .push_str(&self.descriptor.shader_code_declarations);
        self.generated_code.push('\n');
        self.generated_code.push_str(&self.descriptor.shader_code);
    }

    fn generated_shader_code(&self) -> &str {
        &self.generated_code
    }

    fn shader_stage(&self) -> HgiShaderStage {
        self.descriptor.shader_stage
    }

    fn shader_code_declarations(&self) -> &str {
        &self.descriptor.shader_code_declarations
    }

    fn shader_code(&self) -> &str {
        &self.descriptor.shader_code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_generator() {
        let desc = HgiShaderFunctionDesc::new()
            .with_shader_stage(HgiShaderStage::FRAGMENT)
            .with_shader_code_declarations("#define FOO 1")
            .with_shader_code("void main() {}");

        let mut generator = HgiShaderGeneratorBase::new(desc);
        generator.execute();

        let code = generator.generated_shader_code();
        assert!(code.contains("#define FOO 1"));
        assert!(code.contains("void main() {}"));
        assert_eq!(generator.shader_stage(), HgiShaderStage::FRAGMENT);
    }
}
