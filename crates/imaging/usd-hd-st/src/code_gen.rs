
//! HdSt_CodeGen - Shader composition engine for Storm.
//!
//! Composes final shader programs from multiple shader components:
//! geometric shader, material shader, lighting shader, render pass shader.
//!
//! For wgpu backend, this produces WGSL. The low-level WGSL generation
//! lives in `wgsl_code_gen.rs`; this module orchestrates the high-level
//! composition and caching of complete shader programs.

use crate::geometric_shader::HdStGeometricShader;
use crate::shader_code::{HdStShaderCode, ShaderStage};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use usd_tf::Token;

/// Composed shader stages ready for pipeline creation.
#[derive(Debug, Clone)]
pub struct CompiledShader {
    /// Vertex shader source (WGSL)
    pub vertex_source: String,
    /// Fragment shader source (WGSL)
    pub fragment_source: String,
    /// Compute shader source (WGSL), if any
    pub compute_source: Option<String>,
    /// Hash identifying this compiled variant
    pub hash: u64,
}

/// Resource metadata describing buffer/texture bindings for a shader program.
#[derive(Debug, Clone, Default)]
pub struct ResourceMetaData {
    /// Primvar bindings (name -> binding info)
    pub primvar_bindings: Vec<PrimvarBinding>,
    /// Drawing coordinate bindings
    pub drawing_coord_bindings: Vec<DrawingCoordBinding>,
    /// Shader parameter bindings
    pub shader_param_bindings: Vec<ShaderParamBinding>,
}

/// Primvar binding info for code generation.
#[derive(Debug, Clone)]
pub struct PrimvarBinding {
    /// Primvar name
    pub name: Token,
    /// Data type (e.g. "vec3<f32>")
    pub data_type: String,
    /// Binding location
    pub location: u32,
    /// Interpolation mode
    pub interp: PrimvarInterp,
}

/// Primvar interpolation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimvarInterp {
    Constant,
    Uniform,
    Vertex,
    Varying,
    FaceVarying,
}

/// Drawing coordinate binding.
#[derive(Debug, Clone)]
pub struct DrawingCoordBinding {
    pub name: String,
    pub component: String,
    pub location: u32,
}

/// Shader parameter binding.
#[derive(Debug, Clone)]
pub struct ShaderParamBinding {
    pub name: Token,
    pub data_type: String,
    pub binding: u32,
    pub group: u32,
}

/// Shader composition engine.
///
/// Composes WGSL shader source from modular components:
/// - Geometric shader (vertex transform, primitive type)
/// - Material shader (surface evaluation)
/// - Lighting shader (light loop)
/// - Render pass shader (AOV output, clipping, selection)
///
/// Caches compiled results by hash for deduplication.
pub struct HdStCodeGen {
    /// Geometric shader providing vertex/primitive processing
    geometric_shader: Option<Arc<HdStGeometricShader>>,
    /// Material shaders contributing surface evaluation code
    shaders: Vec<Arc<dyn HdStShaderCode>>,
    /// Material tag for sorting (e.g. "defaultMaterialTag", "translucent")
    material_tag: Token,
    /// Resource metadata (buffer layouts, binding info)
    meta_data: ResourceMetaData,
    /// Generated source buckets
    generated_defines: String,
    generated_declarations: String,
    generated_accessors: String,
    generated_vs: String,
    generated_fs: String,
    generated_cs: String,
    /// Whether clip planes are enabled for fragment discard
    has_clip_planes: bool,
    /// Number of active clip planes (0 if has_clip_planes is false)
    num_clip_planes: usize,
}

impl HdStCodeGen {
    /// Create a code gen instance for geometric rendering.
    pub fn new(
        geometric_shader: Arc<HdStGeometricShader>,
        shaders: Vec<Arc<dyn HdStShaderCode>>,
        material_tag: Token,
        meta_data: ResourceMetaData,
    ) -> Self {
        Self {
            geometric_shader: Some(geometric_shader),
            shaders,
            material_tag,
            meta_data,
            generated_defines: String::new(),
            generated_declarations: String::new(),
            generated_accessors: String::new(),
            generated_vs: String::new(),
            generated_fs: String::new(),
            generated_cs: String::new(),
            has_clip_planes: false,
            num_clip_planes: 0,
        }
    }

    /// Create a code gen instance for compute-only use.
    pub fn new_compute(
        shaders: Vec<Arc<dyn HdStShaderCode>>,
        meta_data: ResourceMetaData,
    ) -> Self {
        Self {
            geometric_shader: None,
            shaders,
            material_tag: Token::default(),
            meta_data,
            generated_defines: String::new(),
            generated_declarations: String::new(),
            generated_accessors: String::new(),
            generated_vs: String::new(),
            generated_fs: String::new(),
            generated_cs: String::new(),
            has_clip_planes: false,
            num_clip_planes: 0,
        }
    }

    /// Compute a hash identifying this shader configuration.
    pub fn compute_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.material_tag.hash(&mut hasher);

        if let Some(ref geo) = self.geometric_shader {
            geo.get_key().get_hash().hash(&mut hasher);
        }

        for shader in &self.shaders {
            shader.get_id().hash(&mut hasher);
        }

        self.has_clip_planes.hash(&mut hasher);
        hasher.finish()
    }

    /// Enable or disable clip plane support in generated shaders.
    ///
    /// `count` is the number of active planes; when > 0 the fragment shader
    /// will evaluate each plane equation and discard failing fragments.
    pub fn set_clip_planes(&mut self, enabled: bool) {
        self.has_clip_planes = enabled;
    }

    /// Set the number of active clip planes for WGSL code generation.
    pub fn set_clip_plane_count(&mut self, count: usize) {
        self.num_clip_planes = count;
        self.has_clip_planes = count > 0;
    }

    /// Compile the shader program, generating WGSL source for VS + FS stages.
    pub fn compile(&mut self) -> CompiledShader {
        self.generated_defines.clear();
        self.generated_declarations.clear();
        self.generated_accessors.clear();
        self.generated_vs.clear();
        self.generated_fs.clear();

        // Generate drawing coordinate accessors
        self.emit_drawing_coord();
        // Generate constant primvar accessors
        self.emit_constant_primvar();
        // Generate vertex/face-varying primvar accessors
        self.emit_vertex_primvar();
        // Generate shader parameter accessors (textures, fallbacks)
        self.emit_shader_params();

        // Compose vertex shader
        let vs = self.compose_vertex_shader();
        // Compose fragment shader
        let fs = self.compose_fragment_shader();

        let hash = self.compute_hash();

        CompiledShader {
            vertex_source: vs,
            fragment_source: fs,
            compute_source: None,
            hash,
        }
    }

    /// Compile a compute shader program.
    pub fn compile_compute(&mut self) -> CompiledShader {
        self.generated_defines.clear();
        self.generated_declarations.clear();
        self.generated_cs.clear();

        self.emit_compute_params();

        let cs = self.compose_compute_shader();
        let hash = self.compute_hash();

        CompiledShader {
            vertex_source: String::new(),
            fragment_source: String::new(),
            compute_source: Some(cs),
            hash,
        }
    }

    /// Get the generated vertex shader source.
    pub fn get_vertex_source(&self) -> &str {
        &self.generated_vs
    }

    /// Get the generated fragment shader source.
    pub fn get_fragment_source(&self) -> &str {
        &self.generated_fs
    }

    /// Get the generated compute shader source.
    pub fn get_compute_source(&self) -> &str {
        &self.generated_cs
    }

    // --- Private generation steps ---

    fn emit_drawing_coord(&mut self) {
        use std::fmt::Write;
        writeln!(self.generated_declarations,
            "// Drawing coordinates for instancing and primitive ID"
        ).unwrap();

        for binding in &self.meta_data.drawing_coord_bindings {
            writeln!(self.generated_accessors,
                "fn GetDrawingCoord_{name}() -> u32 {{ return drawing_coord.{comp}; }}",
                name = binding.name,
                comp = binding.component,
            ).unwrap();
        }
    }

    fn emit_constant_primvar(&mut self) {
        use std::fmt::Write;
        for pv in self.meta_data.primvar_bindings.iter()
            .filter(|p| p.interp == PrimvarInterp::Constant)
        {
            writeln!(self.generated_accessors,
                "fn HdGet_{name}() -> {ty} {{ return constant_primvars.{name}; }}",
                name = pv.name.as_str(),
                ty = pv.data_type,
            ).unwrap();
        }
    }

    fn emit_vertex_primvar(&mut self) {
        use std::fmt::Write;
        for pv in self.meta_data.primvar_bindings.iter()
            .filter(|p| matches!(p.interp, PrimvarInterp::Vertex | PrimvarInterp::FaceVarying))
        {
            writeln!(self.generated_accessors,
                "// Primvar accessor: {} (interp={:?})",
                pv.name.as_str(), pv.interp,
            ).unwrap();
        }
    }

    fn emit_shader_params(&mut self) {
        use std::fmt::Write;
        for param in &self.meta_data.shader_param_bindings {
            writeln!(self.generated_declarations,
                "// Shader param: {} @ group({}) binding({})",
                param.name.as_str(), param.group, param.binding,
            ).unwrap();
        }
    }

    fn emit_compute_params(&mut self) {
        use std::fmt::Write;
        writeln!(self.generated_declarations,
            "// Compute shader resource bindings"
        ).unwrap();
    }

    fn compose_vertex_shader(&mut self) -> String {
        use std::fmt::Write;
        let mut src = String::with_capacity(4096);
        writeln!(src, "// Storm WGSL Vertex Shader (composed by HdSt_CodeGen)").unwrap();
        writeln!(src).unwrap();

        src.push_str(&self.generated_defines);
        src.push_str(&self.generated_declarations);
        src.push_str(&self.generated_accessors);

        // Collect vertex source from geometric shader
        if let Some(ref geo) = self.geometric_shader {
            writeln!(src, "// -- Geometric shader vertex stage --").unwrap();
            src.push_str(&geo.get_source(ShaderStage::Vertex));
        }

        // Collect vertex source from each shader
        for shader in &self.shaders {
            let vs = shader.get_source(ShaderStage::Vertex);
            if !vs.is_empty() {
                writeln!(src, "// -- Shader contribution (vertex) --").unwrap();
                src.push_str(&vs);
            }
        }

        self.generated_vs = src.clone();
        src
    }

    fn compose_fragment_shader(&mut self) -> String {
        use std::fmt::Write;
        let mut src = String::with_capacity(4096);
        writeln!(src, "// Storm WGSL Fragment Shader (composed by HdSt_CodeGen)").unwrap();
        writeln!(src).unwrap();

        src.push_str(&self.generated_defines);
        src.push_str(&self.generated_declarations);
        src.push_str(&self.generated_accessors);

        if self.has_clip_planes {
            // WGSL has no gl_ClipDistance (an OpenGL/GLSL-only feature).
            // Port of C++ RenderPass.ApplyClipPlanes (renderPass.glslfx):
            //   gl_ClipDistance[i] = dot(HdGet_clipPlanes(i), Peye);
            // In WGSL we discard in the fragment shader instead.
            // Plane equation uniforms (clip_plane_N: vec4<f32>) are supplied
            // by HdStRenderPassShader for each active plane.
            // eye_pos is the vertex position in eye/camera space (vec4).
            writeln!(src, "// Clip plane discard (software WGSL port of gl_ClipDistance)").unwrap();
            writeln!(src, "// Port of C++ RenderPass.ApplyClipPlanes: dot(plane, Peye) < 0 => discard.").unwrap();
            writeln!(src, "fn apply_clip_planes(eye_pos: vec4<f32>) {{").unwrap();
            for i in 0..self.num_clip_planes {
                // Keep fragment if dot(plane_equation, eye_pos) >= 0; discard otherwise.
                writeln!(src, "    if (dot(clip_plane_{i}, eye_pos) < 0.0) {{ discard; }}").unwrap();
            }
            writeln!(src, "}}").unwrap();
        }

        for shader in &self.shaders {
            let fs = shader.get_source(ShaderStage::Fragment);
            if !fs.is_empty() {
                writeln!(src, "// -- Shader contribution (fragment) --").unwrap();
                src.push_str(&fs);
            }
        }

        self.generated_fs = src.clone();
        src
    }

    fn compose_compute_shader(&mut self) -> String {
        use std::fmt::Write;
        let mut src = String::with_capacity(2048);
        writeln!(src, "// Storm WGSL Compute Shader (composed by HdSt_CodeGen)").unwrap();
        writeln!(src).unwrap();

        src.push_str(&self.generated_defines);
        src.push_str(&self.generated_declarations);

        for shader in &self.shaders {
            let cs = shader.get_source(ShaderStage::Compute);
            if !cs.is_empty() {
                src.push_str(&cs);
            }
        }

        self.generated_cs = src.clone();
        src
    }
}

/// Shader program cache keyed by composition hash.
pub struct ShaderCache {
    cache: HashMap<u64, Arc<CompiledShader>>,
}

impl ShaderCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Look up a cached shader by hash.
    pub fn get(&self, hash: u64) -> Option<Arc<CompiledShader>> {
        self.cache.get(&hash).cloned()
    }

    /// Insert a compiled shader into the cache.
    pub fn insert(&mut self, shader: CompiledShader) -> Arc<CompiledShader> {
        let hash = shader.hash;
        let arc = Arc::new(shader);
        self.cache.insert(hash, arc.clone());
        arc
    }

    /// Remove stale entries.
    pub fn gc(&mut self, live_hashes: &[u64]) {
        self.cache.retain(|k, _| live_hashes.contains(k));
    }

    /// Number of cached programs.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for ShaderCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_metadata_default() {
        let meta = ResourceMetaData::default();
        assert!(meta.primvar_bindings.is_empty());
        assert!(meta.drawing_coord_bindings.is_empty());
        assert!(meta.shader_param_bindings.is_empty());
    }

    #[test]
    fn test_shader_cache() {
        let mut cache = ShaderCache::new();
        assert!(cache.is_empty());

        let shader = CompiledShader {
            vertex_source: "vs".into(),
            fragment_source: "fs".into(),
            compute_source: None,
            hash: 42,
        };

        let _arc = cache.insert(shader);
        assert_eq!(cache.len(), 1);
        assert!(cache.get(42).is_some());
        assert!(cache.get(99).is_none());

        cache.gc(&[42]);
        assert_eq!(cache.len(), 1);
        cache.gc(&[]);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_code_gen_compute() {
        let mut codegen = HdStCodeGen::new_compute(vec![], ResourceMetaData::default());
        let result = codegen.compile_compute();
        assert!(result.compute_source.is_some());
        assert!(result.vertex_source.is_empty());
    }
}
