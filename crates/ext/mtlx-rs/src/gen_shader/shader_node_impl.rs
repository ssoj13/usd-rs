//! ShaderNodeImpl — base for node implementations that emit shader code.

use crate::core::ElementPtr;

use super::Shader;
use super::gen_context::ShaderImplContext;
use super::shader::ShaderStage;
use super::shader_node::ShaderNode;

/// Shader node implementation — handles code generation for a MaterialX node.
/// Each implementation corresponds to an Implementation or NodeGraph element.
pub trait ShaderNodeImpl: Send + Sync {
    /// Return the implementation name.
    fn get_name(&self) -> &str;

    /// Return hash for this implementation (for deduplication of function definitions).
    fn get_hash(&self) -> u64;

    /// Initialize from the interface element (Implementation or NodeGraph).
    fn initialize(&mut self, element: &ElementPtr, context: &dyn ShaderImplContext);

    /// Add additional inputs on a shader node. Default: no-op.
    fn add_inputs(&self, _node: &mut ShaderNode, _context: &dyn ShaderImplContext) {}

    /// Add additional classifications on a node. Default: no-op.
    fn add_classification(&self, _node: &mut ShaderNode) {}

    /// Set values on the shader node from the source element (C++ ShaderNodeImpl::setValues).
    /// Called during ShaderNode creation after initialize and addInputs.
    /// Used by HwImageNode for UDIM atlas UV normalization.
    fn set_values(
        &self,
        _element: &ElementPtr,
        _shader_node: &mut ShaderNode,
        _context: &dyn ShaderImplContext,
    ) {
    }

    /// Create shader variables (uniforms, stage inputs, connectors). Default: no-op.
    /// Called during createShader before emit.
    /// Takes node_name so impl can read node in narrow scope, then mutate shader without overlapping borrows.
    fn create_variables(
        &self,
        _node_name: &str,
        _context: &dyn ShaderImplContext,
        _shader: &mut Shader,
    ) {
    }

    /// Emit function definition for the node. Default: no-op.
    fn emit_function_definition(
        &self,
        _node: &ShaderNode,
        _context: &dyn ShaderImplContext,
        _stage: &mut ShaderStage,
    ) {
    }

    /// Emit the function call or inline code for the node. Default: no-op.
    fn emit_function_call(
        &self,
        _node: &ShaderNode,
        _context: &dyn ShaderImplContext,
        _stage: &mut ShaderStage,
    ) {
    }

    /// Emit output variable declarations. Default: no-op.
    fn emit_output_variables(
        &self,
        _node: &ShaderNode,
        _context: &dyn ShaderImplContext,
        _stage: &mut ShaderStage,
    ) {
    }

    /// Return reference to graph if this impl uses a NodeGraph, otherwise None. Default: None.
    fn get_graph(&self) -> Option<&super::shader_graph::ShaderGraph> {
        None
    }

    /// Returns true if input is editable by users. Default: true.
    fn is_editable(&self, _input_name: &str) -> bool {
        true
    }

    /// Helper: true if the first output of the node is a closure type.
    fn node_output_is_closure(&self, node: &ShaderNode) -> bool {
        let outputs: Vec<_> = node.get_outputs().collect();
        if outputs.is_empty() {
            return false;
        }
        outputs[0].get_type().is_closure()
    }

    /// For OsoNode: return (oso_name, oso_path). Default returns None.
    fn as_oso(&self) -> Option<(&str, &str)> {
        None
    }
}

/// A no-operation node — does nothing. Used for organizational nodes (backdrop, etc.).
#[derive(Debug, Default)]
pub struct NopNode {
    name: String,
    hash: u64,
}

impl NopNode {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::new())
    }
}

impl ShaderNodeImpl for NopNode {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_hash(&self) -> u64 {
        self.hash
    }

    fn initialize(&mut self, element: &ElementPtr, _context: &dyn ShaderImplContext) {
        self.name = element.borrow().get_name().to_string();
        self.hash = super::util::hash_string(&self.name);
    }
}
