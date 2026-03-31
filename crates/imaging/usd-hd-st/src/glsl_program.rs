
//! HdStGLSLProgram - GLSL program wrapper.
//!
//! Manages GLSL shader compilation, linking, and uniform binding.
//! Provides a high-level interface over GPU shader programs.

use crate::shader_code::{HdStShaderCodeSharedPtr, ShaderStage};
use std::collections::HashMap;
use std::sync::Arc;
use usd_tf::Token;
use usd_vt::Value;

/// GLSL program compilation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileStatus {
    /// Not yet compiled
    NotCompiled,
    /// Compilation in progress
    Compiling,
    /// Compilation succeeded
    Success,
    /// Compilation failed
    Failed,
}

/// GLSL shader program.
///
/// Manages the lifecycle of a compiled GPU shader program:
/// - Compiles individual shader stages from source
/// - Links stages into a complete program
/// - Validates the program
/// - Manages uniform and texture bindings
///
/// # Compilation Pipeline
///
/// 1. Add shader stages (vertex, fragment, etc.)
/// 2. Compile each stage
/// 3. Link stages into program
/// 4. Validate program
/// 5. Extract uniform locations
///
/// # GPU Backend
///
/// This is a high-level wrapper. Actual GPU operations would be
/// performed through Hgi (Hydra Graphics Interface).
#[derive(Debug)]
pub struct HdStGLSLProgram {
    /// Unique program ID
    id: u64,

    /// GPU program handle (placeholder until Hgi integration)
    gpu_handle: u64,

    /// Shader stages
    stages: HashMap<ShaderStage, String>,

    /// Compilation status
    compile_status: CompileStatus,

    /// Link status
    link_status: CompileStatus,

    /// Uniform locations cache
    uniform_locations: HashMap<Token, i32>,

    /// Texture unit assignments
    texture_units: HashMap<Token, u32>,

    /// Compilation error log
    error_log: String,
}

impl HdStGLSLProgram {
    /// Create a new GLSL program.
    pub fn new(id: u64) -> Self {
        Self {
            id,
            gpu_handle: 0,
            stages: HashMap::new(),
            compile_status: CompileStatus::NotCompiled,
            link_status: CompileStatus::NotCompiled,
            uniform_locations: HashMap::new(),
            texture_units: HashMap::new(),
            error_log: String::new(),
        }
    }

    /// Create from shader code objects.
    pub fn from_shader_code(shaders: &[HdStShaderCodeSharedPtr]) -> Self {
        let id = shaders
            .iter()
            .map(|s| s.get_hash())
            .fold(0u64, |acc, h| acc ^ h);

        let mut program = Self::new(id);

        // Collect sources from all shader code objects
        for shader in shaders {
            for stage in &[
                ShaderStage::Vertex,
                ShaderStage::TessControl,
                ShaderStage::TessEval,
                ShaderStage::Geometry,
                ShaderStage::Fragment,
            ] {
                let source = shader.get_source(*stage);
                if !source.is_empty() {
                    program.add_stage(*stage, source);
                }
            }
        }

        program
    }

    /// Get program ID.
    pub fn get_id(&self) -> u64 {
        self.id
    }

    /// Get GPU handle.
    pub fn get_gpu_handle(&self) -> u64 {
        self.gpu_handle
    }

    /// Add a shader stage.
    pub fn add_stage(&mut self, stage: ShaderStage, source: String) {
        self.stages.insert(stage, source);
        self.compile_status = CompileStatus::NotCompiled;
    }

    /// Get shader source for a stage.
    pub fn get_stage_source(&self, stage: ShaderStage) -> Option<&String> {
        self.stages.get(&stage)
    }

    /// Compile all shader stages.
    ///
    /// Validates that all stages have non-empty source and marks the program
    /// as compiled. In a WGSL-based pipeline the "compilation" step is deferred
    /// to pipeline creation by HgiWgpu; this function acts as a pre-flight check.
    /// Matches C++ `HdStGLSLProgram::CompileShader` (glslProgram.cpp).
    pub fn compile(&mut self) -> bool {
        if self.stages.is_empty() {
            self.error_log = "No shader stages to compile".to_string();
            self.compile_status = CompileStatus::Failed;
            return false;
        }

        self.compile_status = CompileStatus::Compiling;

        // Validate all stages have non-empty source
        for (stage, source) in &self.stages {
            if source.trim().is_empty() {
                self.error_log = format!("Empty source for stage: {:?}", stage);
                self.compile_status = CompileStatus::Failed;
                return false;
            }
        }

        // Note: Real implementation would compile each stage through Hgi
        self.compile_status = CompileStatus::Success;
        true
    }

    /// Link compiled shader stages into a program.
    ///
    /// Matches C++ `HdStGLSLProgram::Link()` — validates that the program
    /// has the required shader stages before linking. For rendering pipelines,
    /// both vertex and fragment stages are required. Compute pipelines need
    /// only a compute stage.
    pub fn link(&mut self) -> bool {
        if self.compile_status != CompileStatus::Success {
            self.error_log = "Cannot link: compilation not successful".to_string();
            self.link_status = CompileStatus::Failed;
            return false;
        }

        if self.stages.is_empty() {
            self.error_log = "At least one shader has to be compiled before linking.".to_string();
            self.link_status = CompileStatus::Failed;
            return false;
        }

        // Validate required stages:
        // - Compute pipelines: only compute stage needed
        // - Rendering pipelines: vertex + fragment both required
        let has_compute = self.stages.contains_key(&ShaderStage::Compute);
        let has_vertex = self.stages.contains_key(&ShaderStage::Vertex);
        let has_fragment = self.stages.contains_key(&ShaderStage::Fragment);

        if !has_compute && !(has_vertex && has_fragment) {
            let mut missing = Vec::new();
            if !has_vertex {
                missing.push("vertex");
            }
            if !has_fragment {
                missing.push("fragment");
            }
            self.error_log = format!(
                "Failed to link shader: missing required stage(s): {}",
                missing.join(", ")
            );
            self.link_status = CompileStatus::Failed;
            return false;
        }

        // Note: Real implementation would link program through Hgi
        self.link_status = CompileStatus::Success;
        self.gpu_handle = self.id; // Placeholder GPU handle
        true
    }

    /// Validate the linked program.
    pub fn validate(&self) -> bool {
        if self.link_status != CompileStatus::Success {
            return false;
        }

        // Note: Real implementation would validate through Hgi
        // Check for required stages
        self.stages.contains_key(&ShaderStage::Vertex)
            && self.stages.contains_key(&ShaderStage::Fragment)
    }

    /// Get compilation status.
    pub fn get_compile_status(&self) -> CompileStatus {
        self.compile_status
    }

    /// Get link status.
    pub fn get_link_status(&self) -> CompileStatus {
        self.link_status
    }

    /// Check if program is ready to use.
    pub fn is_valid(&self) -> bool {
        self.compile_status == CompileStatus::Success
            && self.link_status == CompileStatus::Success
            && self.validate()
    }

    /// Get error log.
    pub fn get_error_log(&self) -> &str {
        &self.error_log
    }

    /// Get uniform location.
    ///
    /// Returns cached location or queries GPU program.
    pub fn get_uniform_location(&mut self, name: &Token) -> Option<i32> {
        if let Some(&loc) = self.uniform_locations.get(name) {
            return Some(loc);
        }

        // Note: Real implementation would query GPU program
        // For now, return placeholder
        let loc = self.uniform_locations.len() as i32;
        self.uniform_locations.insert(name.clone(), loc);
        Some(loc)
    }

    /// Bind a uniform value.
    pub fn set_uniform(&mut self, name: &Token, _value: &Value) {
        // Note: Real implementation would set uniform through Hgi
        self.get_uniform_location(name);
    }

    /// Assign texture unit to a texture sampler.
    pub fn set_texture_unit(&mut self, name: Token, unit: u32) {
        self.texture_units.insert(name, unit);
    }

    /// Get texture unit for a sampler.
    pub fn get_texture_unit(&self, name: &Token) -> Option<u32> {
        self.texture_units.get(name).copied()
    }

    /// Bind the program for rendering.
    pub fn bind(&self) {
        // Note: Real implementation would bind through Hgi
    }

    /// Unbind the program.
    pub fn unbind(&self) {
        // Note: Real implementation would unbind through Hgi
    }

    /// Get all shader stages.
    pub fn get_stages(&self) -> Vec<ShaderStage> {
        self.stages.keys().copied().collect()
    }
}

/// Shared pointer to GLSL program.
pub type HdStGLSLProgramSharedPtr = Arc<HdStGLSLProgram>;

/// GLSL program builder for convenient construction.
pub struct HdStGLSLProgramBuilder {
    id: u64,
    stages: HashMap<ShaderStage, String>,
}

impl HdStGLSLProgramBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            id: 0,
            stages: HashMap::new(),
        }
    }

    /// Set program ID.
    pub fn id(mut self, id: u64) -> Self {
        self.id = id;
        self
    }

    /// Add vertex shader.
    pub fn vertex(mut self, source: String) -> Self {
        self.stages.insert(ShaderStage::Vertex, source);
        self
    }

    /// Add fragment shader.
    pub fn fragment(mut self, source: String) -> Self {
        self.stages.insert(ShaderStage::Fragment, source);
        self
    }

    /// Add geometry shader.
    pub fn geometry(mut self, source: String) -> Self {
        self.stages.insert(ShaderStage::Geometry, source);
        self
    }

    /// Build the GLSL program.
    pub fn build(self) -> HdStGLSLProgram {
        let mut program = HdStGLSLProgram::new(self.id);
        for (stage, source) in self.stages {
            program.add_stage(stage, source);
        }
        program
    }
}

impl Default for HdStGLSLProgramBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_status() {
        assert_eq!(CompileStatus::NotCompiled, CompileStatus::NotCompiled);
        assert_ne!(CompileStatus::Success, CompileStatus::Failed);
    }

    #[test]
    fn test_glsl_program_creation() {
        let program = HdStGLSLProgram::new(42);
        assert_eq!(program.get_id(), 42);
        assert_eq!(program.get_compile_status(), CompileStatus::NotCompiled);
        assert!(!program.is_valid());
    }

    #[test]
    fn test_add_stage() {
        let mut program = HdStGLSLProgram::new(1);
        program.add_stage(
            ShaderStage::Vertex,
            "#version 450\nvoid main() {}".to_string(),
        );

        assert!(program.get_stage_source(ShaderStage::Vertex).is_some());
        assert!(program.get_stage_source(ShaderStage::Fragment).is_none());
    }

    #[test]
    fn test_compile() {
        let mut program = HdStGLSLProgram::new(1);
        program.add_stage(
            ShaderStage::Vertex,
            "#version 450\nvoid main() {}".to_string(),
        );
        program.add_stage(
            ShaderStage::Fragment,
            "#version 450\nout vec4 color; void main() { color = vec4(1.0); }".to_string(),
        );

        assert!(program.compile());
        assert_eq!(program.get_compile_status(), CompileStatus::Success);
    }

    #[test]
    fn test_link() {
        let mut program = HdStGLSLProgram::new(1);
        program.add_stage(
            ShaderStage::Vertex,
            "#version 450\nvoid main() {}".to_string(),
        );
        program.add_stage(
            ShaderStage::Fragment,
            "#version 450\nout vec4 color; void main() {}".to_string(),
        );

        assert!(program.compile());
        assert!(program.link());
        assert_eq!(program.get_link_status(), CompileStatus::Success);
        assert!(program.is_valid());
    }

    #[test]
    fn test_validation() {
        let mut program = HdStGLSLProgram::new(1);
        program.add_stage(
            ShaderStage::Vertex,
            "#version 450\nvoid main() {}".to_string(),
        );

        assert!(program.compile());
        assert!(!program.link()); // Missing fragment shader
    }

    #[test]
    fn test_uniform_locations() {
        let mut program = HdStGLSLProgram::new(1);
        let token = Token::new("color");

        let loc1 = program.get_uniform_location(&token);
        assert!(loc1.is_some());

        let loc2 = program.get_uniform_location(&token);
        assert_eq!(loc1, loc2); // Should be cached
    }

    #[test]
    fn test_texture_units() {
        let mut program = HdStGLSLProgram::new(1);
        let sampler = Token::new("diffuseMap");

        program.set_texture_unit(sampler.clone(), 0);
        assert_eq!(program.get_texture_unit(&sampler), Some(0));
    }

    #[test]
    fn test_builder() {
        let program = HdStGLSLProgramBuilder::new()
            .id(123)
            .vertex("#version 450\nvoid main() {}".to_string())
            .fragment("#version 450\nout vec4 color; void main() {}".to_string())
            .build();

        assert_eq!(program.get_id(), 123);
        assert!(program.get_stage_source(ShaderStage::Vertex).is_some());
        assert!(program.get_stage_source(ShaderStage::Fragment).is_some());
    }

    #[test]
    fn test_get_stages() {
        let mut program = HdStGLSLProgram::new(1);
        program.add_stage(
            ShaderStage::Vertex,
            "#version 450\nvoid main() {}".to_string(),
        );
        program.add_stage(
            ShaderStage::Fragment,
            "#version 450\nout vec4 color; void main() {}".to_string(),
        );

        let stages = program.get_stages();
        assert_eq!(stages.len(), 2);
        assert!(stages.contains(&ShaderStage::Vertex));
        assert!(stages.contains(&ShaderStage::Fragment));
    }
}
