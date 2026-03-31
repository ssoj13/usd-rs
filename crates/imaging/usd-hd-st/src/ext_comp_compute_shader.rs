#![allow(dead_code)]

//! External computation compute shader for Storm.
//!
//! Internal representation of a compute shader that wraps an HdExtComputation,
//! allowing the use of the code generation and resource binding system to
//! generate a compute shader program.
//!
//! Matches C++ `HdSt_ExtCompComputeShader`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Binding request for shader resources.
#[derive(Debug, Clone)]
pub struct ExtCompBindingRequest {
    /// Binding name
    pub name: Token,
    /// Data type token (e.g. "float", "vec3")
    pub data_type: Token,
    /// Binding index / location
    pub binding: i32,
}

/// External computation compute shader.
///
/// Wraps an external computation's kernel source and resource bindings
/// into a shader code object that can participate in Storm's shader
/// compilation and resource binding pipeline.
///
/// This is an internal type used by `ExtCompGpuComputationResource`.
pub struct ExtCompComputeShader {
    /// Path of the source ExtComputation prim
    comp_id: SdfPath,
    /// GLSL/WGSL kernel source code
    kernel_source: String,
    /// Binding requests for compute inputs/outputs
    bindings: Vec<ExtCompBindingRequest>,
}

impl ExtCompComputeShader {
    /// Create from an external computation's ID and kernel source.
    pub fn new(comp_id: SdfPath, kernel_source: String) -> Self {
        Self {
            comp_id,
            kernel_source,
            bindings: Vec::new(),
        }
    }

    /// Get the source for a given shader stage.
    ///
    /// Compute shaders only have a single stage, so this returns the
    /// kernel source regardless of `shader_stage_key`.
    pub fn get_source(&self, _shader_stage_key: &Token) -> &str {
        &self.kernel_source
    }

    /// Bind resources (called before dispatch).
    ///
    /// In the full implementation this would use the resource binder to
    /// set up SSBO / uniform bindings matching the shader layout.
    pub fn bind_resources(&self, _program: u32) {
        log::debug!(
            "ExtCompComputeShader::bind_resources: {} ({} bindings)",
            self.comp_id,
            self.bindings.len()
        );
    }

    /// Unbind resources (called after dispatch).
    pub fn unbind_resources(&self, _program: u32) {
        log::debug!("ExtCompComputeShader::unbind_resources: {}", self.comp_id);
    }

    /// Add custom binding requests for this shader.
    pub fn add_bindings(&mut self, requests: Vec<ExtCompBindingRequest>) {
        self.bindings.extend(requests);
    }

    /// Get all binding requests.
    pub fn get_bindings(&self) -> &[ExtCompBindingRequest] {
        &self.bindings
    }

    /// Compute a hash of this shader for caching / deduplication.
    ///
    /// Per C++ reference: only hash the kernel source string, NOT the comp_id.
    /// This enables program sharing when two different computations use the
    /// same kernel source code.
    pub fn compute_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.kernel_source.hash(&mut hasher);
        hasher.finish()
    }

    /// Get the ExtComputation prim path.
    pub fn get_ext_computation_id(&self) -> &SdfPath {
        &self.comp_id
    }

    /// Get the kernel source.
    pub fn get_kernel_source(&self) -> &str {
        &self.kernel_source
    }
}

impl std::fmt::Debug for ExtCompComputeShader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtCompComputeShader")
            .field("comp_id", &self.comp_id)
            .field("kernel_source_len", &self.kernel_source.len())
            .field("bindings", &self.bindings.len())
            .finish()
    }
}

/// Shared pointer alias.
pub type ExtCompComputeShaderSharedPtr = Arc<ExtCompComputeShader>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation() {
        let path = SdfPath::from_string("/comp").unwrap();
        let shader = ExtCompComputeShader::new(path.clone(), "void main() { }".to_string());

        assert_eq!(shader.get_ext_computation_id(), &path);
        assert_eq!(shader.get_kernel_source(), "void main() { }");
    }

    #[test]
    fn test_get_source() {
        let shader = ExtCompComputeShader::new(
            SdfPath::from_string("/comp").unwrap(),
            "layout(local_size_x=64) in; void main() {}".to_string(),
        );

        let stage = Token::new("computeShader");
        assert!(shader.get_source(&stage).contains("local_size_x"));
    }

    #[test]
    fn test_bindings() {
        let mut shader =
            ExtCompComputeShader::new(SdfPath::from_string("/comp").unwrap(), String::new());

        assert!(shader.get_bindings().is_empty());

        shader.add_bindings(vec![
            ExtCompBindingRequest {
                name: Token::new("points"),
                data_type: Token::new("vec3"),
                binding: 0,
            },
            ExtCompBindingRequest {
                name: Token::new("normals"),
                data_type: Token::new("vec3"),
                binding: 1,
            },
        ]);

        assert_eq!(shader.get_bindings().len(), 2);
    }

    #[test]
    fn test_hash_differs_by_kernel() {
        let s1 = ExtCompComputeShader::new(
            SdfPath::from_string("/comp").unwrap(),
            "kernel1".to_string(),
        );
        let s2 = ExtCompComputeShader::new(
            SdfPath::from_string("/comp").unwrap(),
            "kernel2".to_string(),
        );
        assert_ne!(s1.compute_hash(), s2.compute_hash());
    }

    #[test]
    fn test_hash_same_kernel_different_id() {
        // Per C++ ref: same kernel source = same hash, enabling program sharing
        let s1 = ExtCompComputeShader::new(
            SdfPath::from_string("/comp1").unwrap(),
            "kernel".to_string(),
        );
        let s2 = ExtCompComputeShader::new(
            SdfPath::from_string("/comp2").unwrap(),
            "kernel".to_string(),
        );
        assert_eq!(s1.compute_hash(), s2.compute_hash());
    }
}
