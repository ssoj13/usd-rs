//! OpenGL GLSL shader generator
//!
//! Implements `HgiGLShaderGenerator` — converts `HgiShaderFunctionDesc`
//! into concrete GLSL source code.
//!
//! Matches C++ `HgiGLShaderGenerator` from `hgiGL/shaderGenerator.h/cpp`.

use usd_hgi::{
    HgiBindingType, HgiInterpolationType, HgiShaderFunctionDesc, HgiShaderGenerator,
    HgiShaderStage, HgiShaderTextureType, HgiStorageType,
};

use super::conversions::hgi_image_layout_format_qualifier;

/// GLSL shader generator for the OpenGL backend.
///
/// Takes a `HgiShaderFunctionDesc` and produces valid GLSL 4.50+ source code
/// including version header, extensions, macros, uniform blocks, texture/buffer
/// bindings, in/out declarations, and the original shader body.
pub struct HgiGLShaderGenerator {
    desc: HgiShaderFunctionDesc,
    /// GLSL version to emit (e.g. 450)
    glsl_version: u32,
    /// Capabilities flags (mirrors C++ HgiDeviceCapabilities bits)
    bindless_buffers: bool,
    bindless_textures: bool,
    shader_draw_parameters: bool,
    builtin_barycentrics: bool,
    /// Generated source
    generated_code: String,
}

impl HgiGLShaderGenerator {
    /// Create a generator with default GL 4.50 capabilities.
    pub fn new(desc: HgiShaderFunctionDesc) -> Self {
        Self {
            desc,
            glsl_version: 450,
            bindless_buffers: false,
            bindless_textures: false,
            shader_draw_parameters: false,
            builtin_barycentrics: false,
            generated_code: String::new(),
        }
    }

    /// Configure the GLSL version emitted by `#version`.
    pub fn with_glsl_version(mut self, version: u32) -> Self {
        self.glsl_version = version;
        self
    }

    /// Enable bindless buffer extension (GL_NV_shader_buffer_load).
    pub fn with_bindless_buffers(mut self, v: bool) -> Self {
        self.bindless_buffers = v;
        self
    }

    /// Enable bindless texture extension (GL_ARB_bindless_texture).
    pub fn with_bindless_textures(mut self, v: bool) -> Self {
        self.bindless_textures = v;
        self
    }

    /// Enable shader draw parameters (GL_ARB_shader_draw_parameters / GLSL 4.60).
    pub fn with_shader_draw_parameters(mut self, v: bool) -> Self {
        self.shader_draw_parameters = v;
        self
    }

    /// Enable NV built-in barycentrics extension.
    pub fn with_builtin_barycentrics(mut self, v: bool) -> Self {
        self.builtin_barycentrics = v;
        self
    }
}

// ---------------------------------------------------------------------------
// Helpers for GLSL code emission
// ---------------------------------------------------------------------------

impl HgiGLShaderGenerator {
    fn write_version(&self, out: &mut String) {
        out.push_str(&format!("#version {}\n", self.glsl_version));
    }

    fn write_extensions(&self, out: &mut String) {
        if self.bindless_buffers {
            out.push_str("#extension GL_NV_shader_buffer_load : require\n");
            out.push_str("#extension GL_NV_gpu_shader5 : require\n");
        }
        if self.bindless_textures {
            out.push_str("#extension GL_ARB_bindless_texture : require\n");
        }

        if self.desc.shader_stage.contains(HgiShaderStage::VERTEX) {
            if self.glsl_version < 460 && self.shader_draw_parameters {
                out.push_str("#extension GL_ARB_shader_draw_parameters : require\n");
            }
            if self.shader_draw_parameters {
                out.push_str("int HgiGetBaseVertex() {\n");
                if self.glsl_version < 460 {
                    out.push_str("  return gl_BaseVertexARB;\n");
                } else {
                    out.push_str("  return gl_BaseVertex;\n");
                }
                out.push_str("}\n");
            }
        }

        if self.desc.shader_stage.contains(HgiShaderStage::FRAGMENT) {
            if self.builtin_barycentrics {
                out.push_str("#extension GL_NV_fragment_shader_barycentric: require\n");
            }
        }
    }

    fn write_macros(&self, out: &mut String) {
        // Cross-language compatibility macros (same as C++ _WriteMacros)
        out.push_str(
            "#define REF(space,type) inout type\n\
             #define FORWARD_DECL(func_decl) func_decl;\n\
             #define ATOMIC_LOAD(a) (a)\n\
             #define ATOMIC_STORE(a, v) (a) = (v)\n\
             #define ATOMIC_ADD(a, v) atomicAdd(a, v)\n\
             #define ATOMIC_EXCHANGE(a, v) atomicExchange(a, v)\n\
             #define ATOMIC_COMP_SWAP(a, expected, desired) atomicCompSwap(a, expected, desired)\n\
             #define atomic_int int\n\
             #define atomic_uint uint\n\
             #define hd_SampleMask gl_SampleMask[0]\n\
             \n\
             #define HGI_HAS_DOUBLE_TYPE 1\n\
             \n",
        );
    }

    fn write_packed_type_definitions(out: &mut String) {
        out.push_str(
            "\nstruct hgi_ivec3 { int    x, y, z; };\n\
              struct hgi_vec3  { float  x, y, z; };\n\
              struct hgi_dvec3 { double x, y, z; };\n\
              struct hgi_mat3  { float  m00, m01, m02,\n\
                                        m10, m11, m12,\n\
                                        m20, m21, m22; };\n\
              struct hgi_dmat3 { double m00, m01, m02,\n\
                                        m10, m11, m12,\n\
                                        m20, m21, m22; };\n",
        );
    }

    /// Emit compute / tessellation / geometry layout qualifiers.
    fn write_layout_attributes(&self, out: &mut String) {
        let stage = self.desc.shader_stage;

        if stage.contains(HgiShaderStage::COMPUTE) {
            let ls = &self.desc.compute_descriptor.local_size;
            let x = ls[0].max(1);
            let y = ls[1].max(1);
            let z = ls[2].max(1);
            out.push_str(&format!(
                "layout(local_size_x = {x}, local_size_y = {y}, local_size_z = {z}) in;\n"
            ));
        } else if stage.contains(HgiShaderStage::FRAGMENT) {
            if self.desc.fragment_descriptor.early_fragment_tests {
                out.push_str("layout (early_fragment_tests) in;\n");
            }
        }
        // Tessellation and geometry layouts would go here
        // (not yet represented in our HgiShaderFunctionDesc)
    }

    /// Emit texture sampler / image declarations.
    fn write_textures(&self, out: &mut String) {
        for tex in &self.desc.textures {
            // Use bind_index from the descriptor for the layout binding qualifier
            let bind_idx = tex.bind_index;
            if tex.writable {
                // image2D / image3D etc. with layout qualifier
                let layout_fmt = hgi_image_layout_format_qualifier(tex.format);
                let img_type = match tex.dimensions {
                    1 => "image1D",
                    3 => "image3D",
                    _ => "image2D",
                };
                let array_part = if tex.array_size > 0 {
                    format!("[{}]", tex.array_size)
                } else {
                    String::new()
                };
                out.push_str(&format!(
                    "layout({layout_fmt}, binding = {bind_idx}) uniform {img_type} {name}{array_part};\n",
                    name = tex.name_in_shader,
                ));
            } else {
                // sampler2D / sampler2DShadow / sampler2DArray / samplerCube etc.
                let smp_type = match (tex.dimensions, tex.texture_type) {
                    (1, _) => "sampler1D",
                    (2, HgiShaderTextureType::ShadowTexture) => "sampler2DShadow",
                    (2, HgiShaderTextureType::ArrayTexture) => "sampler2DArray",
                    (2, HgiShaderTextureType::CubemapTexture) => "samplerCube",
                    (2, _) => "sampler2D",
                    (3, _) => "sampler3D",
                    (_, _) => "sampler2D",
                };
                let array_part = if tex.array_size > 0 {
                    format!("[{}]", tex.array_size)
                } else {
                    String::new()
                };
                out.push_str(&format!(
                    "layout(binding = {bind_idx}) uniform {smp_type} {name}{array_part};\n",
                    name = tex.name_in_shader,
                ));
            }
        }
    }

    /// Emit uniform block / SSBO declarations.
    fn write_buffers(&self, out: &mut String) {
        for buf in &self.desc.buffers {
            let is_uniform = matches!(
                buf.binding,
                HgiBindingType::UniformValue | HgiBindingType::UniformArray
            );
            let (layout_std, block_kw) = if is_uniform {
                ("std140", "uniform")
            } else {
                ("std430", "buffer")
            };
            let array_part = if buf.array_size > 0 {
                format!("[{}]", buf.array_size)
            } else {
                String::new()
            };
            // UBO: layout(std140, binding=N) uniform BlockName { T members; };
            // SSBO: layout(std430, binding=N) buffer BlockName { T members; };
            out.push_str(&format!(
                "layout({layout_std}, binding = {idx}) {block_kw} {name}Block {{\n  {ty} {name}{array_part};\n}};\n",
                idx = buf.bind_index,
                name = buf.name_in_shader,
                ty = buf.type_name,
            ));
        }
    }

    /// Emit constant params as a uniform block at binding 0.
    fn write_constant_params(&self, out: &mut String) {
        if self.desc.constant_params.is_empty() {
            return;
        }
        out.push_str("layout(std140, binding = 0) uniform ParamBuffer {\n");
        for p in &self.desc.constant_params {
            out.push_str(&format!("  {} {};\n", p.type_name, p.name_in_shader));
        }
        out.push_str("};\n");
    }

    /// Emit stage in/out variable declarations.
    fn write_in_outs(&self, out: &mut String) {
        // Built-in GL outputs that must not be re-declared
        const TAKEN_OUT: &[&str] = &[
            "gl_Position",
            "gl_FragColor",
            "gl_FragDepth",
            "gl_PointSize",
            "hd_SampleMask",
        ];

        for param in &self.desc.stage_inputs {
            if TAKEN_OUT.contains(&param.name_in_shader.as_str()) {
                continue;
            }
            let loc = if param.location >= 0 {
                format!("layout(location = {}) ", param.location)
            } else {
                String::new()
            };
            let interp = interpolation_qualifier(param.interpolation);
            let storage = storage_qualifier(param.storage);
            out.push_str(&format!(
                "{loc}{interp}{storage}in {} {};\n",
                param.type_name, param.name_in_shader
            ));
        }

        for param in &self.desc.stage_outputs {
            if TAKEN_OUT.contains(&param.name_in_shader.as_str()) {
                continue;
            }
            let loc = if param.location >= 0 {
                format!("layout(location = {}) ", param.location)
            } else {
                String::new()
            };
            let interp = interpolation_qualifier(param.interpolation);
            out.push_str(&format!(
                "{loc}{interp}out {} {};\n",
                param.type_name, param.name_in_shader
            ));
        }
    }
}

/// Return GLSL interpolation qualifier string for a param.
fn interpolation_qualifier(interp: HgiInterpolationType) -> &'static str {
    match interp {
        HgiInterpolationType::Flat => "flat ",
        HgiInterpolationType::NoPerspective => "noperspective ",
        _ => "",
    }
}

/// Return GLSL storage qualifier string for a param.
fn storage_qualifier(storage: HgiStorageType) -> &'static str {
    match storage {
        HgiStorageType::Patch => "patch ",
        _ => "",
    }
}

impl HgiShaderGenerator for HgiGLShaderGenerator {
    fn execute(&mut self) {
        let mut out = String::with_capacity(4096);

        // Order matches C++ HgiGLShaderGenerator::_Execute()
        self.write_version(&mut out);
        self.write_extensions(&mut out);
        self.write_macros(&mut out);
        Self::write_packed_type_definitions(&mut out);

        // Shader code declarations from descriptor (custom defines/types)
        out.push_str(&self.desc.shader_code_declarations);
        out.push('\n');

        // Stage-specific layout qualifiers
        self.write_layout_attributes(&mut out);

        out.push_str("\n// //////// Global Member Declarations ////////\n");
        self.write_textures(&mut out);
        self.write_buffers(&mut out);
        self.write_constant_params(&mut out);
        self.write_in_outs(&mut out);

        out.push('\n');

        // Shader body
        out.push_str(&self.desc.shader_code);

        self.generated_code = out;
    }

    fn generated_shader_code(&self) -> &str {
        &self.generated_code
    }

    fn shader_stage(&self) -> HgiShaderStage {
        self.desc.shader_stage
    }

    fn shader_code_declarations(&self) -> &str {
        &self.desc.shader_code_declarations
    }

    fn shader_code(&self) -> &str {
        &self.desc.shader_code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_shader_generation() {
        let desc = HgiShaderFunctionDesc::new()
            .with_shader_stage(HgiShaderStage::VERTEX)
            .with_shader_code("void main() { gl_Position = vec4(0.0); }");

        let mut generator = HgiGLShaderGenerator::new(desc);
        generator.execute();

        let code = generator.generated_shader_code();
        assert!(
            code.starts_with("#version 450"),
            "Missing version header: {code}"
        );
        assert!(code.contains("#define REF(space,type)"), "Missing macros");
        assert!(code.contains("void main()"), "Missing shader body");
    }

    #[test]
    fn test_fragment_shader_with_early_tests() {
        use usd_hgi::HgiShaderFunctionFragmentDesc;
        let mut desc = HgiShaderFunctionDesc::new()
            .with_shader_stage(HgiShaderStage::FRAGMENT)
            .with_shader_code("void main() {}");
        desc.fragment_descriptor = HgiShaderFunctionFragmentDesc {
            early_fragment_tests: true,
        };

        let mut generator = HgiGLShaderGenerator::new(desc);
        generator.execute();

        let code = generator.generated_shader_code();
        assert!(
            code.contains("early_fragment_tests"),
            "Missing early_fragment_tests"
        );
    }

    #[test]
    fn test_compute_shader_local_size() {
        use usd_hgi::HgiShaderFunctionComputeDesc;
        let mut desc = HgiShaderFunctionDesc::new()
            .with_shader_stage(HgiShaderStage::COMPUTE)
            .with_shader_code("void main() {}");
        desc.compute_descriptor = HgiShaderFunctionComputeDesc {
            local_size: [8, 8, 1],
        };

        let mut generator = HgiGLShaderGenerator::new(desc);
        generator.execute();

        let code = generator.generated_shader_code();
        assert!(
            code.contains("local_size_x = 8"),
            "Missing compute local_size: {code}"
        );
    }

    #[test]
    fn test_custom_glsl_version() {
        let desc = HgiShaderFunctionDesc::new()
            .with_shader_stage(HgiShaderStage::VERTEX)
            .with_shader_code("void main() {}");

        let mut generator = HgiGLShaderGenerator::new(desc).with_glsl_version(460);
        generator.execute();

        assert!(
            generator
                .generated_shader_code()
                .starts_with("#version 460")
        );
    }
}
