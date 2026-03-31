//! WgslResourceBindingContext — texture2D + sampler separate bindings (per MaterialXGenGlsl ref).
//! WGSL/WebGPU uses separate texture and sampler.

use crate::gen_shader::{ResourceBindingContext, ShaderStage, VariableBlock};

/// WGSL resource binding context — texture and sampler as separate layout(binding=N).
#[derive(Debug)]
pub struct WgslResourceBindingContext {
    uniform_binding_location: usize,
    init_uniform_binding: usize,
}

impl WgslResourceBindingContext {
    pub fn new(uniform_binding_location: usize) -> Self {
        Self {
            uniform_binding_location,
            init_uniform_binding: uniform_binding_location,
        }
    }

    pub fn create(uniform_binding_location: usize) -> Box<dyn ResourceBindingContext> {
        Box::new(Self::new(uniform_binding_location))
    }

    pub fn create_default() -> Box<dyn ResourceBindingContext> {
        Self::create(0)
    }
}

fn glsl_type_name(mtlx: &str) -> &'static str {
    match mtlx {
        "float" => "float",
        "integer" => "int",
        "boolean" => "bool", // naga maps bool to u32 in uniform buffers automatically
        "vector2" => "vec2",
        "vector3" => "vec3",
        "vector4" => "vec4",
        "color3" => "vec3",
        "color4" => "vec4",
        "matrix33" => "mat3",
        "matrix44" => "mat4",
        "filename" => "sampler2D",
        "string" => "int",
        "surfaceshader" | "material" => "vec4",
        _ => "float",
    }
}

impl ResourceBindingContext for WgslResourceBindingContext {
    fn initialize(&mut self) {
        self.uniform_binding_location = self.init_uniform_binding;
    }

    fn emit_directives(&self, stage: &mut ShaderStage) {
        stage.append_line("#extension GL_ARB_shading_language_420pack : enable");
    }

    fn emit_resource_bindings(
        &mut self,
        uniforms: &VariableBlock,
        stage: &mut ShaderStage,
        uniform_qualifier: &str,
        _glsl_type_fn: &dyn Fn(&str) -> &'static str,
    ) {
        let value_vars: Vec<_> = uniforms
            .get_variable_order()
            .iter()
            .filter_map(|n| uniforms.find(n))
            .filter(|v| v.get_type().get_name() != "filename")
            .collect();

        if !value_vars.is_empty() {
            let binding = self.uniform_binding_location;
            self.uniform_binding_location += 1;
            let block_name = format!("{}_{}", uniforms.get_name(), stage.get_name());
            stage.append_line(&format!(
                "layout (std140, binding={}) {} {}",
                binding, uniform_qualifier, block_name
            ));
            stage.append_line("{");
            for v in &value_vars {
                let mtlx_ty = v.get_type().get_name();
                let ty = glsl_type_name(mtlx_ty);
                stage.append_line(&format!("    {} {};", ty, v.get_variable()));
            }
            stage.append_line("};");
            stage.append_line("");
        }

        // Filename: emit texture2D and sampler as separate bindings
        for v in uniforms
            .get_variable_order()
            .iter()
            .filter_map(|n| uniforms.find(n))
        {
            if v.get_type().get_name() == "filename" {
                let var = v.get_variable();
                let binding_tex = self.uniform_binding_location;
                self.uniform_binding_location += 1;
                let binding_samp = self.uniform_binding_location;
                self.uniform_binding_location += 1;
                stage.append_line(&format!(
                    "layout (binding={}) {} texture2D {}_texture;",
                    binding_tex, uniform_qualifier, var
                ));
                stage.append_line(&format!(
                    "layout (binding={}) {} sampler {}_sampler;",
                    binding_samp, uniform_qualifier, var
                ));
            }
        }
        stage.append_line("");
    }

    fn emit_structured_resource_bindings(
        &mut self,
        uniforms: &VariableBlock,
        stage: &mut ShaderStage,
        struct_instance_name: &str,
        array_suffix: &str,
        uniform_qualifier: &str,
        glsl_type_fn: &dyn Fn(&str) -> &'static str,
    ) {
        stage.append_line(&format!("struct {}", uniforms.get_name()));
        stage.append_line("{");
        for n in uniforms.get_variable_order() {
            if let Some(v) = uniforms.find(n) {
                let ty = glsl_type_fn(v.get_type().get_name());
                stage.append_line(&format!("    {} {};", ty, v.get_variable()));
            }
        }
        stage.append_line("};");
        stage.append_line("");

        let binding = self.uniform_binding_location;
        self.uniform_binding_location += 1;
        let block_name = format!("{}_{}", uniforms.get_name(), stage.get_name());
        stage.append_line(&format!(
            "layout (std140, binding={}) {} {}",
            binding, uniform_qualifier, block_name
        ));
        stage.append_line("{");
        stage.append_line(&format!(
            "    {} {}{};",
            uniforms.get_name(),
            struct_instance_name,
            array_suffix
        ));
        stage.append_line("};");
        stage.append_line("");
    }
}
