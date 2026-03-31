//! GlslResourceBindingContext — GLSL layout bindings for uniforms and samplers.
//! По рефу MaterialX GlslResourceBindingContext.cpp

use crate::gen_shader::{ResourceBindingContext, ShaderStage, VariableBlock};

/// std140 alignment sizes per type (bytes); base alignment = 16.
/// Ref: ARB_uniform_buffer_object spec, emitStructuredResourceBindings C++.
/// std140 alignment -- matches C++ GlslResourceBindingContext alignment map.
/// C++ uses baseAlignment (16) for vector2 as well.
fn std140_alignment(type_name: &str) -> usize {
    match type_name {
        "float" | "integer" | "boolean" => 4, // baseAlignment/4
        "vector2" | "color3" | "color4" | "vector3" | "vector4" => 16, // baseAlignment
        "matrix33" | "matrix44" => 64,        // baseAlignment*4
        _ => 16,                              // unknown => full vec4 slot
    }
}

/// GLSL resource binding context — emits layout(binding=N) for Vulkan/GL compliance.
#[derive(Debug)]
pub struct GlslResourceBindingContext {
    uniform_binding_location: usize,
    sampler_binding_location: usize,
    init_uniform_binding: usize,
    init_sampler_binding: usize,
    separate_binding_locations: bool,
}

impl GlslResourceBindingContext {
    pub fn new(uniform_binding_location: usize, sampler_binding_location: usize) -> Self {
        Self {
            uniform_binding_location,
            sampler_binding_location,
            init_uniform_binding: uniform_binding_location,
            init_sampler_binding: sampler_binding_location,
            separate_binding_locations: false,
        }
    }

    pub fn create(
        uniform_binding_location: usize,
        sampler_binding_location: usize,
    ) -> Box<dyn ResourceBindingContext> {
        Box::new(Self::new(
            uniform_binding_location,
            sampler_binding_location,
        ))
    }

    pub fn create_default() -> Box<dyn ResourceBindingContext> {
        Self::create(0, 0)
    }

    pub fn enable_separate_binding_locations(&mut self, separate: bool) {
        self.separate_binding_locations = separate;
    }
}

impl ResourceBindingContext for GlslResourceBindingContext {
    fn initialize(&mut self) {
        self.uniform_binding_location = self.init_uniform_binding;
        self.sampler_binding_location = self.init_sampler_binding;
    }

    fn emit_directives(&self, stage: &mut ShaderStage) {
        // Write shader stage directives for Vulkan compliance if separate binding locations
        if self.separate_binding_locations {
            let shader_stage = match stage.get_name() {
                "vertex" => "vertex",
                "pixel" => "fragment",
                _ => "",
            };
            if !shader_stage.is_empty() {
                stage.append_line(&format!("#pragma shader_stage({})", shader_stage));
            }
        }
        // GL_ARB_shading_language_420pack for layout(binding=)
        stage.append_line("#extension GL_ARB_shading_language_420pack : enable");
    }

    fn emit_resource_bindings(
        &mut self,
        uniforms: &VariableBlock,
        stage: &mut ShaderStage,
        uniform_qualifier: &str,
        glsl_type_fn: &dyn Fn(&str) -> &'static str,
    ) {
        // First: value uniforms in a layout(std140, binding=N) block
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
                let ty = glsl_type_fn(v.get_type().get_name());
                stage.append_line(&format!("    {} {};", ty, v.get_variable()));
            }
            stage.append_line("};");
            stage.append_line("");
        }

        // Second: sampler (filename) uniforms as separate layout(binding=N)
        for v in uniforms
            .get_variable_order()
            .iter()
            .filter_map(|n| uniforms.find(n))
        {
            if v.get_type().get_name() == "filename" {
                let binding = if self.separate_binding_locations {
                    let b = self.uniform_binding_location;
                    self.uniform_binding_location += 1;
                    b
                } else {
                    let b = self.sampler_binding_location;
                    self.sampler_binding_location += 1;
                    b
                };
                let ty = glsl_type_fn(v.get_type().get_name());
                stage.append_line(&format!(
                    "layout (binding={}) {} {} {};",
                    binding,
                    uniform_qualifier,
                    ty,
                    v.get_variable()
                ));
            }
        }

        if !value_vars.is_empty()
            || uniforms.get_variable_order().iter().any(|n| {
                uniforms
                    .find(n)
                    .map(|v| v.get_type().get_name() == "filename")
                    .unwrap_or(false)
            })
        {
            stage.append_line("");
        }
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
        // --- std140 alignment (ref: GlslResourceBindingContext.cpp emitStructuredResourceBindings) ---
        // Compute alignment and total size for each member.
        // alignment, original uniform index
        let base_alignment: usize = 16;
        let n = uniforms.size();
        let order = uniforms.get_variable_order();

        let mut member_order: Vec<(usize, usize)> = (0..n)
            .filter_map(|i| {
                order.get(i).and_then(|name| uniforms.find(name)).map(|v| {
                    let align = std140_alignment(v.get_type().get_name());
                    (align, i)
                })
            })
            .collect();

        // Total struct size before padding
        let struct_size: usize = member_order.iter().map(|(a, _)| *a).sum();

        // Number of padding floats needed to align struct to base_alignment boundary
        let aligned_size = (struct_size + base_alignment - 1) & !(base_alignment - 1);
        let num_padding_floats = (aligned_size - struct_size) / 4;

        // Sort largest alignment first — per std140 layout rules
        member_order.sort_by(|a, b| b.0.cmp(&a.0));

        // Emit the struct definition with members in sorted order
        stage.append_line(&format!("struct {}", uniforms.get_name()));
        stage.append_line("{");
        for (_, var_idx) in &member_order {
            if let Some(name) = order.get(*var_idx) {
                if let Some(v) = uniforms.find(name) {
                    let ty = glsl_type_fn(v.get_type().get_name());
                    stage.append_line(&format!("    {} {};", ty, v.get_variable()));
                }
            }
        }
        // Emit padding floats so the struct fits std140 alignment
        for i in 0..num_padding_floats {
            stage.append_line(&format!("    float pad{};", i));
        }
        stage.append_line("};");
        stage.append_line("");

        // Emit layout(std140, binding=N) uniform block wrapping an instance of the struct
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen_shader::{ShaderStage, VariableBlock, type_desc_types};

    // -- std140_alignment tests --

    #[test]
    fn glsl_std140_alignment_scalar() {
        assert_eq!(std140_alignment("float"), 4);
        assert_eq!(std140_alignment("integer"), 4);
        assert_eq!(std140_alignment("boolean"), 4);
    }

    #[test]
    fn glsl_std140_alignment_vectors() {
        assert_eq!(std140_alignment("vector2"), 16);
        assert_eq!(std140_alignment("vector3"), 16);
        assert_eq!(std140_alignment("vector4"), 16);
        assert_eq!(std140_alignment("color3"), 16);
        assert_eq!(std140_alignment("color4"), 16);
    }

    #[test]
    fn glsl_std140_alignment_matrices() {
        assert_eq!(std140_alignment("matrix33"), 64);
        assert_eq!(std140_alignment("matrix44"), 64);
    }

    #[test]
    fn glsl_std140_alignment_unknown_defaults_to_16() {
        assert_eq!(std140_alignment("custom_type"), 16);
    }

    // -- Construction --

    #[test]
    fn glsl_rbc_new_default() {
        let rbc = GlslResourceBindingContext::new(0, 0);
        assert_eq!(rbc.uniform_binding_location, 0);
        assert_eq!(rbc.sampler_binding_location, 0);
        assert!(!rbc.separate_binding_locations);
    }

    #[test]
    fn glsl_rbc_new_custom() {
        let rbc = GlslResourceBindingContext::new(3, 7);
        assert_eq!(rbc.uniform_binding_location, 3);
        assert_eq!(rbc.sampler_binding_location, 7);
    }

    #[test]
    fn glsl_rbc_enable_separate_binding() {
        let mut rbc = GlslResourceBindingContext::new(0, 0);
        rbc.enable_separate_binding_locations(true);
        assert!(rbc.separate_binding_locations);
    }

    // -- initialize resets counters --

    #[test]
    fn glsl_rbc_initialize_resets() {
        let mut rbc = GlslResourceBindingContext::new(2, 5);
        rbc.uniform_binding_location = 99;
        rbc.sampler_binding_location = 88;
        rbc.initialize();
        assert_eq!(rbc.uniform_binding_location, 2);
        assert_eq!(rbc.sampler_binding_location, 5);
    }

    // -- emit_directives --

    #[test]
    fn glsl_rbc_emit_directives_420pack() {
        let rbc = GlslResourceBindingContext::new(0, 0);
        let mut stage = ShaderStage::new("pixel");
        rbc.emit_directives(&mut stage);
        let code = stage.get_source_code();
        assert!(
            code.contains("GL_ARB_shading_language_420pack"),
            "should emit 420pack extension, got: {}",
            code
        );
    }

    // -- Helper for type mapping --
    fn glsl_type(name: &str) -> &'static str {
        match name {
            "float" => "float",
            "integer" => "int",
            "boolean" => "int",
            "vector2" => "vec2",
            "vector3" | "color3" => "vec3",
            "vector4" | "color4" => "vec4",
            "matrix33" => "mat3",
            "matrix44" => "mat4",
            "filename" => "sampler2D",
            _ => "float",
        }
    }

    // -- emit_resource_bindings: value uniforms --

    #[test]
    fn glsl_rbc_emit_value_uniforms() {
        let mut rbc = GlslResourceBindingContext::new(0, 0);
        let mut uniforms = VariableBlock::new("PubUniforms", "");
        uniforms.add(type_desc_types::float(), "u_val", None, false);
        uniforms.add(type_desc_types::color3(), "u_col", None, false);
        let mut stage = ShaderStage::new("pixel");
        rbc.emit_resource_bindings(&uniforms, &mut stage, "uniform", &glsl_type);
        let code = stage.get_source_code();
        assert!(code.contains("layout (std140, binding=0)"), "got: {}", code);
        assert!(code.contains("float u_val;"), "got: {}", code);
        assert!(code.contains("vec3 u_col;"), "got: {}", code);
        assert_eq!(rbc.uniform_binding_location, 1);
    }

    // -- emit_resource_bindings: sampler uniforms (separate=false) --

    #[test]
    fn glsl_rbc_emit_sampler_uniforms_shared() {
        let mut rbc = GlslResourceBindingContext::new(0, 0);
        let mut uniforms = VariableBlock::new("Samplers", "");
        uniforms.add(type_desc_types::filename(), "u_tex", None, false);
        let mut stage = ShaderStage::new("pixel");
        rbc.emit_resource_bindings(&uniforms, &mut stage, "uniform", &glsl_type);
        let code = stage.get_source_code();
        assert!(code.contains("layout (binding=0)"), "got: {}", code);
        assert!(code.contains("sampler2D u_tex;"), "got: {}", code);
        assert_eq!(rbc.sampler_binding_location, 1);
        assert_eq!(rbc.uniform_binding_location, 0);
    }

    // -- emit_resource_bindings: separate binding locations --

    #[test]
    fn glsl_rbc_emit_sampler_separate_binding() {
        let mut rbc = GlslResourceBindingContext::new(0, 10);
        rbc.enable_separate_binding_locations(true);
        let mut uniforms = VariableBlock::new("Samplers", "");
        uniforms.add(type_desc_types::filename(), "u_tex", None, false);
        let mut stage = ShaderStage::new("pixel");
        rbc.emit_resource_bindings(&uniforms, &mut stage, "uniform", &glsl_type);
        // When separate=true, samplers use uniform_binding_location
        assert_eq!(rbc.uniform_binding_location, 1);
        assert_eq!(rbc.sampler_binding_location, 10); // unchanged
    }

    // -- emit_resource_bindings: mixed value + sampler --

    #[test]
    fn glsl_rbc_emit_mixed_uniforms() {
        let mut rbc = GlslResourceBindingContext::new(0, 0);
        let mut uniforms = VariableBlock::new("Mixed", "");
        uniforms.add(type_desc_types::float(), "u_f", None, false);
        uniforms.add(type_desc_types::filename(), "u_tex1", None, false);
        uniforms.add(type_desc_types::filename(), "u_tex2", None, false);
        let mut stage = ShaderStage::new("pixel");
        rbc.emit_resource_bindings(&uniforms, &mut stage, "uniform", &glsl_type);
        assert_eq!(rbc.uniform_binding_location, 1);
        assert_eq!(rbc.sampler_binding_location, 2);
    }

    // -- emit_structured_resource_bindings --

    #[test]
    fn glsl_rbc_emit_structured_basic() {
        let mut rbc = GlslResourceBindingContext::new(0, 0);
        let mut uniforms = VariableBlock::new("LightData", "");
        uniforms.add(type_desc_types::vector3(), "dir", None, false);
        uniforms.add(type_desc_types::float(), "power", None, false);
        let mut stage = ShaderStage::new("pixel");
        rbc.emit_structured_resource_bindings(
            &uniforms, &mut stage, "u_lights", "[8]", "uniform", &glsl_type,
        );
        let code = stage.get_source_code();
        assert!(code.contains("struct LightData"), "got: {}", code);
        assert!(code.contains("LightData u_lights[8];"), "got: {}", code);
        assert!(code.contains("layout (std140, binding=0)"), "got: {}", code);
        assert_eq!(rbc.uniform_binding_location, 1);
    }

    #[test]
    fn glsl_rbc_structured_padding() {
        let mut rbc = GlslResourceBindingContext::new(0, 0);
        let mut uniforms = VariableBlock::new("Tiny", "");
        uniforms.add(type_desc_types::float(), "x", None, false);
        let mut stage = ShaderStage::new("pixel");
        rbc.emit_structured_resource_bindings(
            &uniforms, &mut stage, "inst", "", "uniform", &glsl_type,
        );
        let code = stage.get_source_code();
        assert!(code.contains("pad0"), "should have pad0, got: {}", code);
        assert!(code.contains("pad2"), "should have pad2, got: {}", code);
    }

    #[test]
    fn glsl_rbc_structured_no_padding_for_vec4() {
        let mut rbc = GlslResourceBindingContext::new(0, 0);
        let mut uniforms = VariableBlock::new("Aligned", "");
        uniforms.add(type_desc_types::vector4(), "v", None, false);
        let mut stage = ShaderStage::new("pixel");
        rbc.emit_structured_resource_bindings(
            &uniforms, &mut stage, "inst", "", "uniform", &glsl_type,
        );
        let code = stage.get_source_code();
        assert!(
            !code.contains("pad"),
            "vec4 should not need padding, got: {}",
            code
        );
    }

    #[test]
    fn glsl_rbc_create_default() {
        let rbc = GlslResourceBindingContext::create_default();
        let mut stage = ShaderStage::new("vertex");
        rbc.emit_directives(&mut stage);
        assert!(stage.get_source_code().contains("420pack"));
    }

    #[test]
    fn glsl_rbc_create_custom() {
        let _rbc = GlslResourceBindingContext::create(5, 10);
    }
}
