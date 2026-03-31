//! NagaWgslShaderGenerator — native WGSL output via naga transpilation.
//!
//! Delegates MaterialX graph -> GLSL code generation to the existing
//! WgslShaderGenerator (texture/sampler split), then transpiles each stage
//! to native WGSL via naga.

use crate::core::ElementPtr;
use crate::gen_glsl::WgslResourceBindingContext;
use crate::gen_glsl::WgslShaderGenerator;
use crate::gen_hw::HwShaderGenerator;
use crate::gen_shader::{
    GenContext, Shader, ShaderGenerator, TypeSystem, VariableBlock, shader_stage,
};

use super::naga_transpiler::{self, ShaderStage};

/// Target identifier for native WGSL generator.
pub const TARGET: &str = "wgsl_native";
/// Version string (naga-transpiled output, not a GLSL version).
pub const VERSION: &str = "1.0";

/// Native WGSL shader generator — produces real WGSL via WgslShaderGen + naga.
///
/// Pipeline: MaterialX Document -> WgslShaderGenerator (GLSL 450 with split
/// texture/sampler) -> naga preprocessor -> naga GLSL frontend -> WGSL backend
pub struct NagaWgslShaderGenerator {
    type_system: TypeSystem,
}

impl NagaWgslShaderGenerator {
    pub fn new(_type_system: TypeSystem) -> Self {
        Self {
            type_system: TypeSystem::new(),
        }
    }

    pub fn create(type_system: Option<TypeSystem>) -> Self {
        Self::new(type_system.unwrap_or_else(TypeSystem::new))
    }

    pub fn get_target(&self) -> &str {
        TARGET
    }

    pub fn get_version(&self) -> &str {
        VERSION
    }

    /// Generate native WGSL shader from a MaterialX element.
    ///
    /// 1. Creates WgslShaderGenerator (GLSL 450 with texture/sampler split)
    /// 2. Generates naga-compatible GLSL
    /// 3. Preprocesses (#define expansion, interface block flattening)
    /// 4. Transpiles each stage to WGSL via naga
    pub fn generate(
        &self,
        name: &str,
        element: &ElementPtr,
        options: &crate::gen_shader::GenOptions,
    ) -> Shader {
        // Use WgslShaderGenerator which emits split texture2D + sampler bindings
        // and uses GenContext's RBC (not VkShaderGenerator's internal one)
        let wgsl_gen = WgslShaderGenerator::new(TypeSystem::new());
        let mut ctx = GenContext::new(wgsl_gen);
        ctx.options = options.clone();
        // Initialize search paths + CMS/unit systems so #include resolution works
        ctx.ensure_default_color_and_unit_systems();
        ctx.set_resource_binding_context(Box::new(WgslResourceBindingContext::new(0)));

        // Step 1: Generate GLSL 450 with texture/sampler split
        let glsl_shader = ctx.generator.generate(name, element, &ctx);

        // Step 2: Preprocess + transpile each stage to WGSL
        self.transpile_shader(name, glsl_shader)
    }

    /// Transpile all stages of a Shader from GLSL 450 to WGSL.
    fn transpile_shader(&self, name: &str, mut shader: Shader) -> Shader {
        for stage in &mut shader.stages {
            let glsl_source = stage.get_source_code();
            if glsl_source.is_empty() {
                continue;
            }

            let naga_stage = if stage.name == shader_stage::VERTEX {
                ShaderStage::Vertex
            } else {
                ShaderStage::Fragment
            };

            match naga_transpiler::glsl_to_wgsl(glsl_source, naga_stage) {
                Ok(wgsl) => {
                    stage.set_source_code(wgsl);
                }
                Err(e) => {
                    log::warn!(
                        "NagaWgslShaderGenerator: failed to transpile stage '{}' of '{}': {}",
                        stage.name,
                        name,
                        e
                    );
                    // Keep GLSL source as-is for debugging
                }
            }
        }
        shader
    }
}

impl ShaderGenerator for NagaWgslShaderGenerator {
    fn get_type_system(&self) -> &TypeSystem {
        &self.type_system
    }

    fn target(&self) -> &str {
        TARGET
    }
}

impl HwShaderGenerator for NagaWgslShaderGenerator {
    fn get_vertex_data_prefix(&self, vertex_data: &VariableBlock) -> String {
        // Same as VK — struct.member access
        format!("{}.", vertex_data.get_instance())
    }
}
