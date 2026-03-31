#![allow(dead_code)]

//! HdStComputation - GPU computation interface for Storm.
//!
//! Provides GPU-based computation capabilities for procedural geometry,
//! aggregations, and data transformations. Computations are executed
//! on the GPU before rendering.

use std::sync::{Arc, LazyLock};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

// Tokens for computation types
static COMPUTE_NORMALS: LazyLock<Token> = LazyLock::new(|| Token::new("computeNormals"));
static COMPUTE_TANGENTS: LazyLock<Token> = LazyLock::new(|| Token::new("computeTangents"));
static COMPUTE_SUBDIVIDE: LazyLock<Token> = LazyLock::new(|| Token::new("computeSubdivide"));

/// GPU computation descriptor.
///
/// Describes a GPU computation including inputs, outputs,
/// and shader source/configuration.
#[derive(Debug, Clone)]
pub struct HdStComputationDesc {
    /// Computation type token
    pub comp_type: Token,
    /// Input buffer names
    pub inputs: Vec<Token>,
    /// Output buffer names
    pub outputs: Vec<Token>,
    /// Compute shader source (GLSL/HLSL/MSL)
    pub shader_source: String,
}

impl HdStComputationDesc {
    /// Create a new computation descriptor.
    pub fn new(comp_type: Token) -> Self {
        Self {
            comp_type,
            inputs: Vec::new(),
            outputs: Vec::new(),
            shader_source: String::new(),
        }
    }

    /// Add an input buffer.
    pub fn add_input(&mut self, name: Token) {
        self.inputs.push(name);
    }

    /// Add an output buffer.
    pub fn add_output(&mut self, name: Token) {
        self.outputs.push(name);
    }

    /// Set shader source.
    pub fn set_shader(&mut self, source: String) {
        self.shader_source = source;
    }
}

/// Storm GPU computation.
///
/// Represents a GPU computation executed via compute shaders.
/// Computations can be chained and are scheduled during the sync phase.
///
/// # Examples
///
/// ```ignore
/// let mut comp = HdStComputation::new(path, Token::new("computeNormals"));
/// comp.add_input(Token::new("positions"));
/// comp.add_output(Token::new("normals"));
/// comp.sync();
/// comp.execute();
/// ```
#[derive(Debug)]
pub struct HdStComputation {
    /// Prim path (for debugging/identification)
    path: SdfPath,

    /// Computation descriptor
    desc: HdStComputationDesc,

    /// Compiled compute shader handle (GPU resource)
    shader_handle: u64,

    /// Whether computation needs recompilation
    dirty: bool,

    /// Number of invocations (elements to process)
    element_count: usize,
}

impl HdStComputation {
    /// Create a new GPU computation.
    pub fn new(path: SdfPath, comp_type: Token) -> Self {
        Self {
            path,
            desc: HdStComputationDesc::new(comp_type),
            shader_handle: 0,
            dirty: true,
            element_count: 0,
        }
    }

    /// Get prim path.
    pub fn get_path(&self) -> &SdfPath {
        &self.path
    }

    /// Get computation descriptor.
    pub fn get_desc(&self) -> &HdStComputationDesc {
        &self.desc
    }

    /// Get mutable computation descriptor.
    pub fn get_desc_mut(&mut self) -> &mut HdStComputationDesc {
        self.dirty = true;
        &mut self.desc
    }

    /// Add input buffer.
    pub fn add_input(&mut self, name: Token) {
        self.desc.add_input(name);
        self.dirty = true;
    }

    /// Add output buffer.
    pub fn add_output(&mut self, name: Token) {
        self.desc.add_output(name);
        self.dirty = true;
    }

    /// Set shader source.
    pub fn set_shader(&mut self, source: String) {
        self.desc.set_shader(source);
        self.dirty = true;
    }

    /// Set number of elements to process.
    pub fn set_element_count(&mut self, count: usize) {
        self.element_count = count;
    }

    /// Get number of elements.
    pub fn get_element_count(&self) -> usize {
        self.element_count
    }

    /// Check if computation is dirty.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark computation as dirty.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.shader_handle = 0; // Invalidate compiled shader
    }

    /// Sync the computation.
    ///
    /// Compiles compute shader if dirty.
    /// In full implementation, this would use Hgi to compile GLSL/HLSL/MSL.
    pub fn sync(&mut self) {
        if !self.dirty {
            return;
        }

        // Note: Full implementation would:
        // 1. Compile compute shader via Hgi
        // 2. Create input/output buffer bindings
        // 3. Setup dispatch parameters
        //
        // Placeholder: assign dummy shader handle
        self.shader_handle = 1;
        self.dirty = false;

        log::debug!(
            "HdStComputation::sync: {} (type: {})",
            self.path,
            self.desc.comp_type
        );
    }

    /// Execute the computation on GPU.
    ///
    /// Dispatches compute shader with configured inputs/outputs.
    /// In full implementation, this would submit GPU commands via Hgi.
    pub fn execute(&self) {
        if self.shader_handle == 0 {
            log::warn!(
                "HdStComputation::execute: shader not compiled for {}",
                self.path
            );
            return;
        }

        // Note: Full implementation would:
        // 1. Bind input buffers
        // 2. Bind output buffers
        // 3. Dispatch compute shader (work groups based on element_count)
        // 4. Insert memory barriers for output buffers
        //
        // Placeholder: log execution
        log::debug!(
            "HdStComputation::execute: {} ({} elements)",
            self.path,
            self.element_count
        );
    }

    /// Get compiled shader handle.
    pub fn get_shader_handle(&self) -> u64 {
        self.shader_handle
    }

    /// Check if shader is compiled.
    pub fn is_compiled(&self) -> bool {
        self.shader_handle != 0
    }
}

/// Shared pointer to Storm computation.
pub type HdStComputationSharedPtr = Arc<HdStComputation>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_computation_creation() {
        let path = SdfPath::from_string("/comp").unwrap();
        let comp = HdStComputation::new(path.clone(), COMPUTE_NORMALS.clone());

        assert_eq!(comp.get_path(), &path);
        assert_eq!(comp.get_desc().comp_type, *COMPUTE_NORMALS);
        assert!(comp.is_dirty());
        assert!(!comp.is_compiled());
    }

    #[test]
    fn test_computation_descriptor() {
        let mut desc = HdStComputationDesc::new(COMPUTE_NORMALS.clone());
        desc.add_input(Token::new("positions"));
        desc.add_output(Token::new("normals"));
        desc.set_shader("void main() {}".to_string());

        assert_eq!(desc.inputs.len(), 1);
        assert_eq!(desc.outputs.len(), 1);
        assert!(!desc.shader_source.is_empty());
    }

    #[test]
    fn test_computation_sync() {
        let path = SdfPath::from_string("/comp").unwrap();
        let mut comp = HdStComputation::new(path, COMPUTE_NORMALS.clone());

        comp.add_input(Token::new("positions"));
        comp.add_output(Token::new("normals"));
        comp.set_element_count(100);

        assert!(comp.is_dirty());

        comp.sync();

        assert!(!comp.is_dirty());
        assert!(comp.is_compiled());
        assert_eq!(comp.get_element_count(), 100);
    }

    #[test]
    fn test_computation_execute() {
        let path = SdfPath::from_string("/comp").unwrap();
        let mut comp = HdStComputation::new(path, COMPUTE_NORMALS.clone());

        comp.set_element_count(256);
        comp.sync();

        // Should not panic
        comp.execute();
    }

    #[test]
    fn test_mark_dirty() {
        let path = SdfPath::from_string("/comp").unwrap();
        let mut comp = HdStComputation::new(path, COMPUTE_NORMALS.clone());

        comp.sync();
        assert!(!comp.is_dirty());

        comp.mark_dirty();
        assert!(comp.is_dirty());
        assert!(!comp.is_compiled());
    }

    #[test]
    fn test_multiple_inputs_outputs() {
        let path = SdfPath::from_string("/comp").unwrap();
        let mut comp = HdStComputation::new(path, COMPUTE_TANGENTS.clone());

        comp.add_input(Token::new("positions"));
        comp.add_input(Token::new("normals"));
        comp.add_input(Token::new("uvs"));
        comp.add_output(Token::new("tangents"));
        comp.add_output(Token::new("bitangents"));

        assert_eq!(comp.get_desc().inputs.len(), 3);
        assert_eq!(comp.get_desc().outputs.len(), 2);
    }
}
