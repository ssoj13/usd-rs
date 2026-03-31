//! VkResourceBindingContext -- Vulkan GLSL resource binding context.
//! By ref MaterialX VkResourceBindingContext.h/.cpp
//! Uses shared uniform_binding_location for both value uniforms and samplers
//! (unlike GlslResourceBindingContext which can have separate sampler bindings).

use crate::gen_shader::{ResourceBindingContext, ShaderStage, VariableBlock};

/// std140 alignment sizes for structured resource bindings.
/// std140 alignment -- matches C++ VkResourceBindingContext which uses baseAlignment for vector2.
fn std140_alignment(type_name: &str) -> usize {
    const BASE: usize = 16;
    match type_name {
        "float" | "integer" | "boolean" => BASE / 4,
        // C++ uses baseAlignment (16) for vector2, matching the alignment map
        "vector2" | "color3" | "color4" | "vector3" | "vector4" => BASE,
        "matrix33" | "matrix44" => BASE * 4,
        _ => BASE,
    }
}

/// Vulkan GLSL resource binding context -- single binding counter, #pragma shader_stage.
#[derive(Debug)]
pub struct VkResourceBindingContext {
    hw_uniform_bind_location: usize,
    hw_init_uniform_bind_location: usize,
}

impl VkResourceBindingContext {
    pub fn new(uniform_binding_location: usize) -> Self {
        Self {
            hw_uniform_bind_location: uniform_binding_location,
            hw_init_uniform_bind_location: uniform_binding_location,
        }
    }

    pub fn create(uniform_binding_location: usize) -> Box<dyn ResourceBindingContext> {
        Box::new(Self::new(uniform_binding_location))
    }

    pub fn create_default() -> Box<dyn ResourceBindingContext> {
        Self::create(0)
    }
}

impl ResourceBindingContext for VkResourceBindingContext {
    fn initialize(&mut self) {
        self.hw_uniform_bind_location = self.hw_init_uniform_bind_location;
    }

    fn emit_directives(&self, stage: &mut ShaderStage) {
        // Write shader stage directive for Vulkan compliance
        let shader_stage = match stage.get_name() {
            "vertex" => "vertex",
            "pixel" => "fragment",
            _ => "",
        };
        if !shader_stage.is_empty() {
            stage.append_line(&format!("#pragma shader_stage({})", shader_stage));
        }
    }

    fn emit_resource_bindings(
        &mut self,
        uniforms: &VariableBlock,
        stage: &mut ShaderStage,
        uniform_qualifier: &str,
        glsl_type_fn: &dyn Fn(&str) -> &'static str,
    ) {
        // First: value uniforms in a layout(std140) block
        let has_value_uniforms = uniforms
            .get_variable_order()
            .iter()
            .filter_map(|n| uniforms.find(n))
            .any(|v| v.get_type().get_name() != "filename");

        if has_value_uniforms {
            let binding = self.hw_uniform_bind_location;
            self.hw_uniform_bind_location += 1;
            let block_name = format!("{}_{}", uniforms.get_name(), stage.get_name());
            stage.append_line(&format!(
                "layout (std140, binding={}) {} {}",
                binding, uniform_qualifier, block_name
            ));
            stage.append_line("{");
            for v in uniforms
                .get_variable_order()
                .iter()
                .filter_map(|n| uniforms.find(n))
            {
                if v.get_type().get_name() != "filename" {
                    let ty = glsl_type_fn(v.get_type().get_name());
                    stage.append_line(&format!("    {} {};", ty, v.get_variable()));
                }
            }
            stage.append_line("};");
        }

        // Second: sampler uniforms as separate bindings
        for v in uniforms
            .get_variable_order()
            .iter()
            .filter_map(|n| uniforms.find(n))
        {
            if v.get_type().get_name() == "filename" {
                let binding = self.hw_uniform_bind_location;
                self.hw_uniform_bind_location += 1;
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
        // Compute alignment and total size for each member
        let base_alignment: usize = 16;
        let order = uniforms.get_variable_order();
        let n = uniforms.size();

        let mut member_order: Vec<(usize, usize)> = (0..n)
            .filter_map(|i| {
                order.get(i).and_then(|name| uniforms.find(name)).map(|v| {
                    let align = std140_alignment(v.get_type().get_name());
                    (align, i)
                })
            })
            .collect();

        let struct_size: usize = member_order.iter().map(|(a, _)| *a).sum();
        let aligned_size = (struct_size + base_alignment - 1) & !(base_alignment - 1);
        let num_padding_floats = (aligned_size - struct_size) / 4;

        // Sort largest alignment first (per std140 layout rules)
        member_order.sort_by(|a, b| b.0.cmp(&a.0));

        // Emit struct definition
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
        // Emit padding
        for i in 0..num_padding_floats {
            stage.append_line(&format!("    float pad{};", i));
        }
        stage.append_line("};");
        stage.append_line("");

        // Emit binding block wrapping the struct instance
        let binding = self.hw_uniform_bind_location;
        self.hw_uniform_bind_location += 1;
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen_shader::{ShaderStage, VariableBlock, type_desc_types};

    // -- std140_alignment tests --

    #[test]
    fn vk_std140_alignment_scalar() {
        assert_eq!(std140_alignment("float"), 4);
        assert_eq!(std140_alignment("integer"), 4);
        assert_eq!(std140_alignment("boolean"), 4);
    }

    #[test]
    fn vk_std140_alignment_vectors() {
        assert_eq!(std140_alignment("vector2"), 16);
        assert_eq!(std140_alignment("vector3"), 16);
        assert_eq!(std140_alignment("vector4"), 16);
        assert_eq!(std140_alignment("color3"), 16);
        assert_eq!(std140_alignment("color4"), 16);
    }

    #[test]
    fn vk_std140_alignment_matrices() {
        assert_eq!(std140_alignment("matrix33"), 64);
        assert_eq!(std140_alignment("matrix44"), 64);
    }

    // -- VkResourceBindingContext construction --

    #[test]
    fn vk_rbc_new_default() {
        let rbc = VkResourceBindingContext::new(0);
        assert_eq!(rbc.hw_uniform_bind_location, 0);
    }

    #[test]
    fn vk_rbc_new_custom() {
        let rbc = VkResourceBindingContext::new(5);
        assert_eq!(rbc.hw_uniform_bind_location, 5);
    }

    // -- initialize resets counter --

    #[test]
    fn vk_rbc_initialize_resets() {
        let mut rbc = VkResourceBindingContext::new(3);
        rbc.hw_uniform_bind_location = 99;
        rbc.initialize();
        assert_eq!(rbc.hw_uniform_bind_location, 3);
    }

    // -- emit_directives emits #pragma shader_stage --

    #[test]
    fn vk_rbc_emit_directives_vertex() {
        let rbc = VkResourceBindingContext::new(0);
        let mut stage = ShaderStage::new("vertex");
        rbc.emit_directives(&mut stage);
        let code = stage.get_source_code();
        assert!(
            code.contains("#pragma shader_stage(vertex)"),
            "vertex stage should emit shader_stage(vertex), got: {}",
            code,
        );
    }

    #[test]
    fn vk_rbc_emit_directives_pixel() {
        let rbc = VkResourceBindingContext::new(0);
        let mut stage = ShaderStage::new("pixel");
        rbc.emit_directives(&mut stage);
        let code = stage.get_source_code();
        assert!(
            code.contains("#pragma shader_stage(fragment)"),
            "pixel stage should emit shader_stage(fragment), got: {}",
            code,
        );
    }

    #[test]
    fn vk_rbc_emit_directives_unknown_stage() {
        let rbc = VkResourceBindingContext::new(0);
        let mut stage = ShaderStage::new("compute");
        rbc.emit_directives(&mut stage);
        let code = stage.get_source_code();
        assert!(
            !code.contains("#pragma"),
            "unknown stage should not emit pragma, got: {}",
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

    // -- emit_resource_bindings: single binding counter for both values and samplers --

    #[test]
    fn vk_rbc_emit_resource_bindings_value_uniforms() {
        let mut rbc = VkResourceBindingContext::new(0);
        let mut uniforms = VariableBlock::new("PubUniforms", "");
        uniforms.add(type_desc_types::float(), "u_val", None, false);
        uniforms.add(type_desc_types::color3(), "u_col", None, false);
        let mut stage = ShaderStage::new("pixel");
        rbc.emit_resource_bindings(&uniforms, &mut stage, "uniform", &glsl_type);
        let code = stage.get_source_code();
        assert!(
            code.contains("layout (std140, binding=0)"),
            "should have binding=0, got: {}",
            code
        );
        assert!(code.contains("float u_val;"));
        assert!(code.contains("vec3 u_col;"));
        assert_eq!(rbc.hw_uniform_bind_location, 1);
    }

    #[test]
    fn vk_rbc_emit_resource_bindings_sampler() {
        let mut rbc = VkResourceBindingContext::new(0);
        let mut uniforms = VariableBlock::new("Samplers", "");
        uniforms.add(type_desc_types::filename(), "u_tex", None, false);
        let mut stage = ShaderStage::new("pixel");
        rbc.emit_resource_bindings(&uniforms, &mut stage, "uniform", &glsl_type);
        let code = stage.get_source_code();
        assert!(
            code.contains("layout (binding=0)"),
            "sampler should have binding=0, got: {}",
            code
        );
        assert!(code.contains("sampler2D u_tex;"));
        assert_eq!(rbc.hw_uniform_bind_location, 1);
    }

    #[test]
    fn vk_rbc_emit_resource_bindings_mixed_single_counter() {
        let mut rbc = VkResourceBindingContext::new(0);
        let mut uniforms = VariableBlock::new("Mixed", "");
        uniforms.add(type_desc_types::float(), "u_f", None, false);
        uniforms.add(type_desc_types::filename(), "u_tex1", None, false);
        uniforms.add(type_desc_types::filename(), "u_tex2", None, false);
        let mut stage = ShaderStage::new("pixel");
        rbc.emit_resource_bindings(&uniforms, &mut stage, "uniform", &glsl_type);
        // binding=0 for value block, binding=1 and binding=2 for samplers
        assert_eq!(rbc.hw_uniform_bind_location, 3);
    }

    // -- emit_structured_resource_bindings --

    #[test]
    fn vk_rbc_emit_structured_bindings() {
        let mut rbc = VkResourceBindingContext::new(0);
        let mut uniforms = VariableBlock::new("LightData", "");
        uniforms.add(type_desc_types::vector3(), "dir", None, false);
        uniforms.add(type_desc_types::float(), "power", None, false);
        let mut stage = ShaderStage::new("pixel");
        rbc.emit_structured_resource_bindings(
            &uniforms, &mut stage, "u_lights", "[8]", "uniform", &glsl_type,
        );
        let code = stage.get_source_code();
        assert!(
            code.contains("struct LightData"),
            "should have struct, got: {}",
            code
        );
        assert!(code.contains("LightData u_lights[8];"));
        assert!(code.contains("layout (std140, binding=0)"));
        assert_eq!(rbc.hw_uniform_bind_location, 1);
    }

    #[test]
    fn vk_rbc_structured_padding() {
        let mut rbc = VkResourceBindingContext::new(0);
        let mut uniforms = VariableBlock::new("Tiny", "");
        uniforms.add(type_desc_types::float(), "x", None, false);
        let mut stage = ShaderStage::new("pixel");
        rbc.emit_structured_resource_bindings(
            &uniforms, &mut stage, "inst", "", "uniform", &glsl_type,
        );
        let code = stage.get_source_code();
        // float=4, aligned to 16 => 3 padding floats
        assert!(code.contains("pad0"), "should have padding, got: {}", code);
        assert!(code.contains("pad2"), "should have pad2");
    }
}
