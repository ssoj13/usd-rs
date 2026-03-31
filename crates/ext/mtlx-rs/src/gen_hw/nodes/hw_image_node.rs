//! HwImageNode — texture image node with UV scale/offset support for HW shaders.
//! Based on MaterialX HwImageNode.cpp.
//!
//! Extends SourceCodeNode by adding `uv_scale` and `uv_offset` inputs for UDIM
//! atlas normalization (hwNormalizeUdimTexCoords). The actual texture sampling
//! is performed by the parent SourceCodeNode's function definition/call logic.

use crate::core::{ElementPtr, Value, Vector2};
use crate::gen_shader::{
    Shader, ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, SourceCodeNode,
    type_desc_types,
};

const UV_SCALE: &str = "uv_scale";
const UV_OFFSET: &str = "uv_offset";

/// Image node for HW shaders — delegates all code-gen to SourceCodeNode,
/// and adds `uv_scale` / `uv_offset` inputs for optional UDIM atlas UV remapping.
pub struct HwImageNode {
    /// Inner SourceCodeNode for file/sourcecode emission
    inner: Box<dyn ShaderNodeImpl>,
}

impl std::fmt::Debug for HwImageNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HwImageNode")
            .field("name", &self.inner.get_name())
            .finish()
    }
}

impl HwImageNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self {
            inner: SourceCodeNode::create(),
        })
    }
}

impl ShaderNodeImpl for HwImageNode {
    fn get_name(&self) -> &str {
        self.inner.get_name()
    }

    fn get_hash(&self) -> u64 {
        self.inner.get_hash()
    }

    fn initialize(&mut self, element: &ElementPtr, context: &dyn ShaderImplContext) {
        self.inner.initialize(element, context);
    }

    /// Add `uv_scale` (vec2, default 1,1) and `uv_offset` (vec2, default 0,0).
    /// These extra inputs are used when UDIM UVs are remapped to a texture atlas.
    fn add_inputs(&self, node: &mut ShaderNode, _context: &dyn ShaderImplContext) {
        // uv_scale: default (1, 1) — no scaling
        let scale_in = node.add_input(UV_SCALE, type_desc_types::vector2());
        scale_in
            .port_mut()
            .set_value(Some(Value::Vector2(Vector2([1.0, 1.0]))), true);

        // uv_offset: default (0, 0) — no offset
        let offset_in = node.add_input(UV_OFFSET, type_desc_types::vector2());
        offset_in
            .port_mut()
            .set_value(Some(Value::Vector2(Vector2([0.0, 0.0]))), true);
    }

    fn create_variables(
        &self,
        node_name: &str,
        context: &dyn ShaderImplContext,
        shader: &mut Shader,
    ) {
        self.inner.create_variables(node_name, context, shader);
    }

    fn emit_function_definition(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        self.inner.emit_function_definition(node, context, stage);
    }

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        self.inner.emit_function_call(node, context, stage);
    }

    fn emit_output_variables(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        self.inner.emit_output_variables(node, context, stage);
    }

    fn is_editable(&self, input_name: &str) -> bool {
        // uv_scale and uv_offset are internal — not user-editable
        if input_name == UV_SCALE || input_name == UV_OFFSET {
            return false;
        }
        self.inner.is_editable(input_name)
    }
}
