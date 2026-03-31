//! MslResourceBindingContext -- Metal [[buffer(n)]], struct emission (ref: MaterialXGenMsl).
//! Emits struct definitions for uniform blocks; Metal bindings are in entry-point parameters.

use crate::gen_shader::{ResourceBindingContext, ShaderStage, VariableBlock};

/// Metal alignment values per type (ref: Apple Metal Shading Language Spec).
const BASE_ALIGNMENT: usize = 16;

/// Get alignment size for a MaterialX type name.
fn alignment_for_type(type_name: &str) -> usize {
    match type_name {
        "float" | "integer" | "boolean" => BASE_ALIGNMENT / 4, // 4 bytes
        "vector2" => BASE_ALIGNMENT / 2,                       // 8 bytes
        "color3" | "vector3" | "color4" | "vector4" => BASE_ALIGNMENT, // 16 bytes
        "matrix33" | "matrix44" => BASE_ALIGNMENT * 4,         // 64 bytes
        _ => BASE_ALIGNMENT,
    }
}

/// MSL resource binding context -- emits struct definitions for uniform blocks.
/// Actual [[buffer(n)]] bindings are applied at the shader entry-point level.
#[derive(Debug)]
pub struct MslResourceBindingContext {
    uniform_binding_location: usize,
    sampler_binding_location: usize,
    init_uniform_binding: usize,
    init_sampler_binding: usize,
    separate_binding_locations: bool,
}

impl MslResourceBindingContext {
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

impl ResourceBindingContext for MslResourceBindingContext {
    fn initialize(&mut self) {
        self.uniform_binding_location = self.init_uniform_binding;
        self.sampler_binding_location = self.init_sampler_binding;
    }

    fn emit_directives(&self, _stage: &mut ShaderStage) {
        // Metal needs no directives for bindings
    }

    fn emit_resource_bindings(
        &mut self,
        uniforms: &VariableBlock,
        stage: &mut ShaderStage,
        _uniform_qualifier: &str,
        msl_type_fn: &dyn Fn(&str) -> &'static str,
    ) {
        // Emit struct for value uniforms (non-filename) -- ref: MslResourceBindingContext.cpp
        // C++ reference does NOT emit filename vars separately; only value uniforms in struct
        let value_vars: Vec<_> = uniforms
            .get_variable_order()
            .iter()
            .filter_map(|n| uniforms.find(n))
            .filter(|v| v.get_type().get_name() != "filename")
            .collect();

        if !value_vars.is_empty() {
            let block_name = uniforms.get_name();
            stage.append_line(&format!("struct {}", block_name));
            stage.append_line("{");
            for v in &value_vars {
                let ty = msl_type_fn(v.get_type().get_name());
                stage.append_line(&format!("    {} {};", ty, v.get_variable()));
            }
            stage.append_line("};");
        }

        stage.append_line("");
    }

    fn emit_structured_resource_bindings(
        &mut self,
        uniforms: &VariableBlock,
        stage: &mut ShaderStage,
        struct_instance_name: &str,
        array_suffix: &str,
        _uniform_qualifier: &str,
        msl_type_fn: &dyn Fn(&str) -> &'static str,
    ) {
        // Compute alignment and size for each member (ref: MslResourceBindingContext.cpp)
        let mut member_order: Vec<(usize, usize)> = Vec::new(); // (alignment, index)
        let mut struct_size: usize = 0;

        let var_order = uniforms.get_variable_order();
        for i in 0..var_order.len() {
            if let Some(v) = uniforms.find(&var_order[i]) {
                let align = alignment_for_type(v.get_type().get_name());
                struct_size += align;
                member_order.push((align, i));
            }
        }

        // Align up to BASE_ALIGNMENT and compute padding floats
        let aligned_size = (struct_size + (BASE_ALIGNMENT - 1)) & !(BASE_ALIGNMENT - 1);
        let num_padding_floats = (aligned_size - struct_size) / 4;

        // Sort from largest alignment to smallest (ref: C++ std::sort descending)
        member_order.sort_by(|a, b| b.0.cmp(&a.0));

        // Emit the struct with sorted members
        let block_name = uniforms.get_name();
        stage.append_line(&format!("struct {}", block_name));
        stage.append_line("{");
        for &(_, var_idx) in &member_order {
            if let Some(v) = uniforms.find(&var_order[var_idx]) {
                let ty = msl_type_fn(v.get_type().get_name());
                stage.append_line(&format!("    {} {};", ty, v.get_variable()));
            }
        }

        // Emit padding floats for 16-byte alignment
        for i in 0..num_padding_floats {
            stage.append_line(&format!("    float pad{};", i));
        }
        stage.append_line("};");

        // Emit binding wrapper struct (ref: "struct LightData_ps { LightData u_lightData[MAX]; };")
        stage.append_line("");
        let instance_block = format!("{}_{}", block_name, stage.get_name());
        stage.append_line(&format!("struct {}", instance_block));
        stage.append_line("{");
        stage.append_line(&format!(
            "    {} {}{};",
            block_name, struct_instance_name, array_suffix
        ));
        stage.append_line("};");
    }
}
