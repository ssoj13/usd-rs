//! Metal shader code generator.
//! Port of pxr/imaging/hgiMetal/shaderGenerator
//!
//! Takes a HgiShaderFunctionDesc and generates Metal Shading Language code.

use crate::shader_section::HgiMetalShaderSection;
use usd_hgi::{HgiShaderFunctionDesc, HgiShaderGenerator, HgiShaderStage};

/// Metal shader code generator.
///
/// Converts GLSLFX domain language shader descriptions into
/// Metal Shading Language (MSL) code.
///
/// Mirrors C++ HgiMetalShaderGenerator.
pub struct HgiMetalShaderGenerator {
    descriptor: HgiShaderFunctionDesc,
    shader_sections: Vec<Box<dyn HgiMetalShaderSection>>,
    generated_code: String,
}

impl HgiMetalShaderGenerator {
    /// Create a new Metal shader generator from a descriptor.
    /// Mirrors C++ HgiMetalShaderGenerator(hgi, descriptor).
    pub fn new(descriptor: HgiShaderFunctionDesc) -> Self {
        let mut generator = Self {
            descriptor,
            shader_sections: Vec::new(),
            generated_code: String::new(),
        };
        generator.build_keyword_input_sections();
        generator
    }

    /// Get the shader sections for code generation.
    pub fn shader_sections(&self) -> &[Box<dyn HgiMetalShaderSection>] {
        &self.shader_sections
    }

    /// Get mutable shader sections.
    pub fn shader_sections_mut(&mut self) -> &mut Vec<Box<dyn HgiMetalShaderSection>> {
        &mut self.shader_sections
    }

    /// Add a shader section.
    pub fn add_section(&mut self, section: Box<dyn HgiMetalShaderSection>) {
        self.shader_sections.push(section);
    }

    /// Build keyword input shader sections from descriptor.
    fn build_keyword_input_sections(&mut self) {
        // Stub: on real Metal this would inspect descriptor.compute_descriptor
        // and create keyword input sections for thread_position_in_grid, etc.
    }

    /// Execute code generation, populating generated_code.
    fn execute_impl(&mut self) {
        self.generated_code.clear();
        self.generated_code.push_str("#include <metal_stdlib>\n");
        self.generated_code.push_str("using namespace metal;\n\n");

        // Global macros
        for section in &self.shader_sections {
            section.visit_global_macros(&mut self.generated_code);
        }

        // Global member declarations
        for section in &self.shader_sections {
            section.visit_global_member_declarations(&mut self.generated_code);
        }

        // Scope structs
        for section in &self.shader_sections {
            section.visit_scope_structs(&mut self.generated_code);
        }

        // Scope member declarations
        for section in &self.shader_sections {
            section.visit_scope_member_declarations(&mut self.generated_code);
        }

        // Scope function definitions
        for section in &self.shader_sections {
            section.visit_scope_function_definitions(&mut self.generated_code);
        }

        // Shader code declarations and body
        self.generated_code
            .push_str(&self.descriptor.shader_code_declarations);
        self.generated_code.push('\n');
        self.generated_code.push_str(&self.descriptor.shader_code);
    }
}

impl HgiShaderGenerator for HgiMetalShaderGenerator {
    fn execute(&mut self) {
        self.execute_impl();
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
