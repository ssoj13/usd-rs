//! Vulkan GLSL shader generator.
//!
//! Port of pxr/imaging/hgiVulkan/shaderGenerator.h/.cpp
//!
//! Takes an `HgiShaderFunctionDesc` and generates a complete GLSL source
//! string by composing typed shader sections.

use ash::vk;
use std::collections::HashMap;

use usd_hgi::{
    HgiBindingType, HgiDeviceCapabilities, HgiShaderFunctionBufferDesc, HgiShaderFunctionDesc,
    HgiShaderFunctionParamBlockDesc, HgiShaderFunctionParamDesc, HgiShaderFunctionTextureDesc,
    HgiShaderSectionAttribute, HgiShaderStage, HgiStorageType,
};

use crate::capabilities::HgiVulkanCapabilities;
use crate::conversions::HgiVulkanConversions;
use crate::descriptor_set_layouts::{HgiVulkanDescriptorSetInfo, HgiVulkanDescriptorSetInfoVector};
use crate::shader_section::{
    HgiVulkanBlockShaderSection, HgiVulkanBufferShaderSection, HgiVulkanKeywordShaderSection,
    HgiVulkanMemberShaderSection, HgiVulkanTextureShaderSection,
};

// Sub-module types not re-exported at the crate root of usd_hgi.
use usd_hgi::shader_function::{
    GeometryInPrimitiveType, GeometryOutPrimitiveType, TessellationOrdering, TessellationPatchType,
    TessellationSpacing,
};

// ---------------------------------------------------------------------------
// Packed-type struct definitions emitted verbatim into every shader
// ---------------------------------------------------------------------------

const PACKED_TYPE_DEFINITIONS: &str = r#"
struct hgi_ivec3 { int    x, y, z; };
struct hgi_vec3  { float  x, y, z; };
struct hgi_dvec3 { double x, y, z; };
struct hgi_mat3  { float  m00, m01, m02,
                          m10, m11, m12,
                          m20, m21, m22; };
struct hgi_dmat3 { double m00, m01, m02,
                          m10, m11, m12,
                          m20, m21, m22; };
"#;

// ---------------------------------------------------------------------------
// Section enum — owns all section variants in a single Vec
// ---------------------------------------------------------------------------

/// Owns one shader section variant.
///
/// Using an enum instead of trait objects avoids the raw-pointer dance that
/// `HgiVulkanInterstageBlockShaderSection` uses for member references; we can
/// embed a plain `usize` member-index instead.
enum Section {
    Block(HgiVulkanBlockShaderSection),
    Member(HgiVulkanMemberShaderSection),
    Keyword(HgiVulkanKeywordShaderSection),
    Texture(HgiVulkanTextureShaderSection),
    Buffer(HgiVulkanBufferShaderSection),
    InterstageBlock {
        block_name: String,
        instance_name: String,
        attributes: Vec<HgiShaderSectionAttribute>,
        qualifier: String,
        array_size: String,
        /// Indices into the owning generator's `sections` Vec pointing at
        /// `Section::Member` entries that belong to this block.
        member_indices: Vec<usize>,
    },
}

// ---------------------------------------------------------------------------
// Keyword token map (matches C++ HgiShaderKeywordTokens)
// ---------------------------------------------------------------------------

/// Maps C++ HgiShaderKeywordTokens role strings to GLSL built-in names.
fn keyword_map() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("hdPosition", "gl_Position");
    m.insert("hdPointCoord", "gl_PointCoord");
    m.insert("hdClipDistance", "gl_ClipDistance");
    m.insert("hdCullDistance", "gl_CullDistance");
    m.insert("hdVertexID", "gl_VertexIndex");
    m.insert("hdInstanceID", "gl_InstanceIndex");
    m.insert("hdPrimitiveID", "gl_PrimitiveID");
    m.insert("hdSampleID", "gl_SampleID");
    m.insert("hdSamplePosition", "gl_SamplePosition");
    m.insert("hdFragCoord", "gl_FragCoord");
    m.insert("hdBaseVertex", "gl_BaseVertex");
    m.insert("hdBaseInstance", "HgiGetBaseInstance()");
    m.insert("hdFrontFacing", "gl_FrontFacing");
    m.insert("hdLayer", "gl_Layer");
    m.insert("hdViewportIndex", "gl_ViewportIndex");
    m.insert("hdGlobalInvocationID", "gl_GlobalInvocationID");
    m.insert("hdBaryCoordNoPersp", "gl_BaryCoordNoPerspEXT");
    m.insert("hdSampleMaskIn", "gl_SampleMaskIn[0]");
    m
}

/// Out params that are pure built-ins — no declaration emitted.
const TAKEN_OUT_PARAMS: &[&str] = &[
    "gl_Position",
    "gl_FragColor",
    "gl_FragDepth",
    "gl_PointSize",
    "gl_CullDistance",
    "hd_SampleMask",
];

/// Out params that are built-in but still require a declaration (e.g. for array size).
const TAKEN_OUT_PARAMS_TO_DECLARE: &[&str] = &["gl_ClipDistance"];

// ---------------------------------------------------------------------------
// HgiVulkanShaderGenerator
// ---------------------------------------------------------------------------

/// Generates GLSL source from an `HgiShaderFunctionDesc`.
///
/// The generator owns all shader sections and writes them into a single string
/// via `execute()`.
pub struct HgiVulkanShaderGenerator {
    /// Shader stage bitflag from the descriptor.
    shader_stage: HgiShaderStage,
    /// Preprocessor declarations from the descriptor (emitted before sections).
    shader_code_declarations: String,
    /// Main shader body (emitted last).
    shader_code: String,

    /// `layout(...)` preamble lines specific to the shader stage.
    shader_layout_attributes: Vec<String>,

    /// All sections in creation order.
    sections: Vec<Section>,

    /// Descriptor set bindings collected during construction.
    descriptor_set_info: HgiVulkanDescriptorSetInfoVector,
    /// True when new bindings have been added since the last `descriptor_set_info()` call.
    descriptor_set_layouts_added: bool,

    /// Next free binding index for textures (set to max(buffer bind indices) + 1).
    texture_bind_index_start: u32,
    /// Auto-increment counter for `in` location attributes.
    in_location_index: u32,
    /// Auto-increment counter for `out` location attributes.
    out_location_index: u32,

    /// Vulkan capabilities used for version / extension queries.
    capabilities: HgiVulkanCapabilities,
}

impl HgiVulkanShaderGenerator {
    /// Build a generator from a shader function descriptor.
    ///
    /// Processes descriptor fields in the same order as C++:
    /// `_WriteConstantParams` → `_WriteBuffers` → `_WriteTextures` → inputs → outputs.
    /// This guarantees identical binding index assignment to the C++ implementation.
    pub fn new(desc: &HgiShaderFunctionDesc, capabilities: HgiVulkanCapabilities) -> Self {
        let mut this = Self {
            shader_stage: desc.shader_stage,
            shader_code_declarations: desc.shader_code_declarations.clone(),
            shader_code: desc.shader_code.clone(),
            shader_layout_attributes: Vec::new(),
            sections: Vec::new(),
            descriptor_set_info: Vec::new(),
            descriptor_set_layouts_added: false,
            texture_bind_index_start: 0,
            in_location_index: 0,
            out_location_index: 0,
            capabilities,
        };

        this.build_layout_attributes(desc);

        // Order matters: buffers before textures so texture bind indices start
        // right after the last buffer index, mirroring HgiVulkanResourceBindings.
        this.write_constant_params(&desc.constant_params);
        this.write_buffers(&desc.buffers);
        this.write_textures(&desc.textures);
        this.write_in_outs(&desc.stage_inputs, "in");
        this.write_in_out_blocks(&desc.stage_input_blocks, "in");
        this.write_in_outs(&desc.stage_outputs, "out");
        this.write_in_out_blocks(&desc.stage_output_blocks, "out");

        this
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Execute shader generation and return the complete GLSL source string.
    pub fn execute(&self) -> String {
        let mut out = String::with_capacity(4096);
        self.write_version(&mut out);
        self.write_extensions(&mut out);
        self.write_macros(&mut out);
        out.push_str(PACKED_TYPE_DEFINITIONS);
        out.push_str(&self.shader_code_declarations);

        for attr in &self.shader_layout_attributes {
            out.push_str(attr);
        }

        out.push_str("\n// //////// Global Includes ////////\n");
        // No section type currently emits includes.

        out.push_str("\n// //////// Global Macros ////////\n");
        // No section type currently emits global macros.

        out.push_str("\n// //////// Global Structs ////////\n");
        // No section type currently emits structs.

        out.push_str("\n// //////// Global Member Declarations ////////\n");
        for section in &self.sections {
            self.visit_global_member_declarations(section, &mut out);
        }

        out.push_str("\n// //////// Global Function Definitions ////////\n");
        for section in &self.sections {
            self.visit_global_function_definitions(section, &mut out);
        }

        out.push('\n');
        out.push_str(&self.shader_code);
        out
    }

    /// Returns the descriptor set info collected during construction.
    ///
    /// Bindings are sorted by index on first call after new bindings were added.
    pub fn descriptor_set_info(&mut self) -> &HgiVulkanDescriptorSetInfoVector {
        if self.descriptor_set_layouts_added && !self.descriptor_set_info.is_empty() {
            self.descriptor_set_layouts_added = false;
            if let Some(set) = self.descriptor_set_info.first_mut() {
                set.bindings.sort_by_key(|b| b.binding);
            }
        }
        &self.descriptor_set_info
    }

    // -----------------------------------------------------------------------
    // Visit helpers (read sections and write GLSL)
    // -----------------------------------------------------------------------

    fn visit_global_member_declarations(&self, section: &Section, out: &mut String) {
        match section {
            Section::Block(s) => {
                s.visit_global_member_declarations(out);
            }
            Section::Member(s) => {
                s.visit_global_member_declarations(out);
            }
            Section::Keyword(s) => {
                s.visit_global_member_declarations(out);
            }
            Section::Texture(s) => {
                s.visit_global_member_declarations(out);
            }
            Section::Buffer(s) => {
                s.visit_global_member_declarations(out);
            }
            Section::InterstageBlock {
                block_name,
                instance_name,
                attributes,
                qualifier,
                array_size,
                member_indices,
            } => {
                self.write_interstage_block_declaration(
                    block_name,
                    instance_name,
                    attributes,
                    qualifier,
                    array_size,
                    member_indices,
                    out,
                );
            }
        }
    }

    fn visit_global_function_definitions(&self, section: &Section, out: &mut String) {
        if let Section::Texture(s) = section {
            s.visit_global_function_definitions(out);
        }
    }

    // -----------------------------------------------------------------------
    // Stage-specific layout attribute construction
    // -----------------------------------------------------------------------

    fn build_layout_attributes(&mut self, desc: &HgiShaderFunctionDesc) {
        if desc.shader_stage.contains(HgiShaderStage::COMPUTE) {
            let mut x = desc.compute_descriptor.local_size[0];
            let mut y = desc.compute_descriptor.local_size[1];
            let mut z = desc.compute_descriptor.local_size[2];
            if x == 0 || y == 0 || z == 0 {
                x = 1;
                y = 1;
                z = 1;
            }
            self.shader_layout_attributes.push(format!(
                "layout(local_size_x = {x}, local_size_y = {y}, local_size_z = {z}) in;\n"
            ));
        } else if desc
            .shader_stage
            .contains(HgiShaderStage::TESSELLATION_CONTROL)
        {
            self.shader_layout_attributes.push(format!(
                "layout (vertices = {}) out;\n",
                desc.tessellation_descriptor.num_verts_per_patch_out
            ));
        } else if desc
            .shader_stage
            .contains(HgiShaderStage::TESSELLATION_EVAL)
        {
            match desc.tessellation_descriptor.patch_type {
                TessellationPatchType::Triangles => {
                    self.shader_layout_attributes
                        .push("layout (triangles) in;\n".into());
                }
                TessellationPatchType::Quads => {
                    self.shader_layout_attributes
                        .push("layout (quads) in;\n".into());
                }
                TessellationPatchType::Isolines => {
                    self.shader_layout_attributes
                        .push("layout (isolines) in;\n".into());
                }
            }
            match desc.tessellation_descriptor.spacing {
                TessellationSpacing::Equal => {
                    self.shader_layout_attributes
                        .push("layout (equal_spacing) in;\n".into());
                }
                TessellationSpacing::FractionalEven => {
                    self.shader_layout_attributes
                        .push("layout (fractional_even_spacing) in;\n".into());
                }
                TessellationSpacing::FractionalOdd => {
                    self.shader_layout_attributes
                        .push("layout (fractional_odd_spacing) in;\n".into());
                }
            }
            // Winding order is intentionally flipped — see HgiVulkanGraphicsCmds::SetViewport.
            match desc.tessellation_descriptor.ordering {
                TessellationOrdering::CW => {
                    self.shader_layout_attributes
                        .push("layout (ccw) in;\n".into());
                }
                TessellationOrdering::CCW => {
                    self.shader_layout_attributes
                        .push("layout (cw) in;\n".into());
                }
            }
        } else if desc.shader_stage.contains(HgiShaderStage::GEOMETRY) {
            match desc.geometry_descriptor.in_primitive_type {
                GeometryInPrimitiveType::Points => {
                    self.shader_layout_attributes
                        .push("layout (points) in;\n".into());
                }
                GeometryInPrimitiveType::Lines => {
                    self.shader_layout_attributes
                        .push("layout (lines) in;\n".into());
                }
                GeometryInPrimitiveType::LinesAdjacency => {
                    self.shader_layout_attributes
                        .push("layout (lines_adjacency) in;\n".into());
                }
                GeometryInPrimitiveType::Triangles => {
                    self.shader_layout_attributes
                        .push("layout (triangles) in;\n".into());
                }
                GeometryInPrimitiveType::TrianglesAdjacency => {
                    self.shader_layout_attributes
                        .push("layout (triangles_adjacency) in;\n".into());
                }
            }
            let max_v = &desc.geometry_descriptor.out_max_vertices;
            match desc.geometry_descriptor.out_primitive_type {
                GeometryOutPrimitiveType::Points => {
                    self.shader_layout_attributes
                        .push(format!("layout (points, max_vertices = {max_v}) out;\n"));
                }
                GeometryOutPrimitiveType::LineStrip => {
                    self.shader_layout_attributes.push(format!(
                        "layout (line_strip, max_vertices = {max_v}) out;\n"
                    ));
                }
                GeometryOutPrimitiveType::TriangleStrip => {
                    self.shader_layout_attributes.push(format!(
                        "layout (triangle_strip, max_vertices = {max_v}) out;\n"
                    ));
                }
            }
        } else if desc.shader_stage.contains(HgiShaderStage::FRAGMENT) {
            if desc.fragment_descriptor.early_fragment_tests {
                self.shader_layout_attributes
                    .push("layout (early_fragment_tests) in;\n".into());
            }
        }
    }

    // -----------------------------------------------------------------------
    // Section constructors (mirror C++ _Write* methods)
    // -----------------------------------------------------------------------

    fn write_constant_params(&mut self, params: &[HgiShaderFunctionParamDesc]) {
        if params.is_empty() {
            return;
        }
        self.sections
            .push(Section::Block(HgiVulkanBlockShaderSection::new(
                "ParamBuffer",
                params.to_vec(),
            )));
    }

    fn write_textures(&mut self, textures: &[HgiShaderFunctionTextureDesc]) {
        for desc in textures {
            let bind_index = self.texture_bind_index_start + desc.bind_index;

            let mut attrs: Vec<HgiShaderSectionAttribute> = Vec::new();

            if desc.writable {
                // Format qualifier must come before binding for writable images.
                let fmt_qual = HgiVulkanConversions::get_image_layout_format_qualifier(desc.format);
                attrs.push(HgiShaderSectionAttribute {
                    identifier: fmt_qual,
                    index: String::new(),
                });
            }
            attrs.push(HgiShaderSectionAttribute {
                identifier: "binding".to_string(),
                index: bind_index.to_string(),
            });

            let descriptor_type = if desc.writable {
                vk::DescriptorType::STORAGE_IMAGE
            } else {
                vk::DescriptorType::COMBINED_IMAGE_SAMPLER
            };
            let descriptor_count = if desc.array_size > 0 {
                desc.array_size as u32
            } else {
                1
            };

            self.sections
                .push(Section::Texture(HgiVulkanTextureShaderSection::new(
                    &desc.name_in_shader,
                    bind_index,
                    desc.dimensions,
                    desc.format,
                    desc.texture_type,
                    desc.array_size as u32,
                    desc.writable,
                    attrs,
                    "",
                )));

            self.add_descriptor_set_layout_binding(bind_index, descriptor_type, descriptor_count);
        }
    }

    fn write_buffers(&mut self, buffers: &[HgiShaderFunctionBufferDesc]) {
        for buf in buffers {
            let is_uniform = matches!(
                buf.binding,
                HgiBindingType::UniformValue | HgiBindingType::UniformArray
            );
            let array_size_str = if buf.array_size > 0 {
                buf.array_size.to_string()
            } else {
                String::new()
            };
            let bind_index = buf.bind_index;

            let attrs: Vec<HgiShaderSectionAttribute> = if is_uniform {
                vec![
                    HgiShaderSectionAttribute {
                        identifier: "std140".to_string(),
                        index: String::new(),
                    },
                    HgiShaderSectionAttribute {
                        identifier: "binding".to_string(),
                        index: bind_index.to_string(),
                    },
                ]
            } else {
                vec![
                    HgiShaderSectionAttribute {
                        identifier: "std430".to_string(),
                        index: String::new(),
                    },
                    HgiShaderSectionAttribute {
                        identifier: "binding".to_string(),
                        index: bind_index.to_string(),
                    },
                ]
            };

            // Uniform buffers are never writable.
            let writable = !is_uniform && buf.writable;

            self.sections
                .push(Section::Buffer(HgiVulkanBufferShaderSection::new(
                    &buf.name_in_shader,
                    bind_index,
                    &buf.type_name,
                    buf.binding,
                    array_size_str,
                    writable,
                    attrs,
                )));

            // Textures must start after the last buffer binding index.
            self.texture_bind_index_start = self.texture_bind_index_start.max(bind_index + 1);

            let descriptor_type = if is_uniform {
                vk::DescriptorType::UNIFORM_BUFFER
            } else {
                vk::DescriptorType::STORAGE_BUFFER
            };
            self.add_descriptor_set_layout_binding(bind_index, descriptor_type, 1);
        }
    }

    fn write_in_outs(&mut self, params: &[HgiShaderFunctionParamDesc], qualifier: &str) {
        let keywords = keyword_map();
        let in_qualifier = qualifier == "in";
        let out_qualifier = qualifier == "out";

        for param in params {
            let name = &param.name_in_shader;

            // Skip pure built-in out params that must not be re-declared.
            if out_qualifier && TAKEN_OUT_PARAMS.contains(&name.as_str()) {
                continue;
            }

            // Some built-ins (e.g. gl_ClipDistance) still need a declaration for array size.
            if out_qualifier && TAKEN_OUT_PARAMS_TO_DECLARE.contains(&name.as_str()) {
                let idx = self.out_location_index;
                self.out_location_index += 1;
                let attrs = vec![HgiShaderSectionAttribute {
                    identifier: "location".to_string(),
                    index: idx.to_string(),
                }];
                self.sections
                    .push(Section::Member(HgiVulkanMemberShaderSection::new(
                        name,
                        &param.type_name,
                        param.interpolation,
                        param.sampling,
                        param.storage,
                        attrs,
                        qualifier,
                        "",
                        &param.array_size,
                        "",
                    )));
                continue;
            }

            // For in params, map role to built-in keyword alias.
            if in_qualifier {
                if let Some(&keyword) = keywords.get(param.role.as_str()) {
                    // Only emit an alias if the declared name differs from the built-in.
                    if name != keyword {
                        self.sections
                            .push(Section::Keyword(HgiVulkanKeywordShaderSection::new(
                                name,
                                &param.type_name,
                                keyword,
                            )));
                    }
                    continue;
                }
            }

            // Determine location attribute.
            let attrs = if param.location != -1 {
                vec![HgiShaderSectionAttribute {
                    identifier: "location".to_string(),
                    index: param.location.to_string(),
                }]
            } else if param.interstage_slot != -1 {
                vec![HgiShaderSectionAttribute {
                    identifier: "location".to_string(),
                    index: param.interstage_slot.to_string(),
                }]
            } else {
                let idx = if in_qualifier {
                    let i = self.in_location_index;
                    self.in_location_index += 1;
                    i
                } else {
                    let i = self.out_location_index;
                    self.out_location_index += 1;
                    i
                };
                vec![HgiShaderSectionAttribute {
                    identifier: "location".to_string(),
                    index: idx.to_string(),
                }]
            };

            self.sections
                .push(Section::Member(HgiVulkanMemberShaderSection::new(
                    name,
                    &param.type_name,
                    param.interpolation,
                    param.sampling,
                    param.storage,
                    attrs,
                    qualifier,
                    "",
                    &param.array_size,
                    "",
                )));
        }
    }

    fn write_in_out_blocks(&mut self, blocks: &[HgiShaderFunctionParamBlockDesc], qualifier: &str) {
        let in_qualifier = qualifier == "in";
        let out_qualifier = qualifier == "out";

        for block in blocks {
            // Snapshot location before processing this block's members.
            let location_index = if in_qualifier {
                self.in_location_index
            } else {
                self.out_location_index
            };

            let mut member_indices: Vec<usize> = Vec::with_capacity(block.members.len());

            for member in &block.members {
                let idx = self.sections.len();
                self.sections
                    .push(Section::Member(HgiVulkanMemberShaderSection::new(
                        &member.name,
                        &member.type_name,
                        member.interpolation,
                        member.sampling,
                        HgiStorageType::Default,
                        vec![], // no layout attrs on block members
                        qualifier,
                        "",
                        "",
                        &block.instance_name,
                    )));
                member_indices.push(idx);

                if in_qualifier {
                    self.in_location_index += 1;
                } else if out_qualifier {
                    self.out_location_index += 1;
                }
            }

            let attrs = if block.interstage_slot != -1 {
                vec![HgiShaderSectionAttribute {
                    identifier: "location".to_string(),
                    index: block.interstage_slot.to_string(),
                }]
            } else {
                vec![HgiShaderSectionAttribute {
                    identifier: "location".to_string(),
                    index: location_index.to_string(),
                }]
            };

            self.sections.push(Section::InterstageBlock {
                block_name: block.block_name.clone(),
                instance_name: block.instance_name.clone(),
                attributes: attrs,
                qualifier: qualifier.to_string(),
                array_size: block.array_size.clone(),
                member_indices,
            });
        }
    }

    // -----------------------------------------------------------------------
    // Interstage block declaration emitter
    // -----------------------------------------------------------------------

    fn write_interstage_block_declaration(
        &self,
        block_name: &str,
        instance_name: &str,
        attributes: &[HgiShaderSectionAttribute],
        qualifier: &str,
        array_size: &str,
        member_indices: &[usize],
        out: &mut String,
    ) {
        write_layout_attributes(attributes, out);
        out.push_str(qualifier);
        out.push(' ');
        out.push_str(block_name);
        out.push_str(" {\n");

        for &idx in member_indices {
            if let Section::Member(m) = &self.sections[idx] {
                out.push_str("  ");
                m.write_interpolation(out);
                m.write_sampling(out);
                m.write_storage(out);
                m.write_type(out);
                out.push(' ');
                m.write_identifier(out);
                out.push_str(";\n");
            }
        }

        out.push_str("} ");
        if !instance_name.is_empty() {
            out.push_str(instance_name);
        }
        if !array_size.is_empty() {
            out.push('[');
            out.push_str(array_size);
            out.push(']');
        }
        out.push_str(";\n");
    }

    // -----------------------------------------------------------------------
    // Descriptor set layout bookkeeping
    // -----------------------------------------------------------------------

    fn add_descriptor_set_layout_binding(
        &mut self,
        binding_index: u32,
        descriptor_type: vk::DescriptorType,
        descriptor_count: u32,
    ) {
        if self.descriptor_set_info.is_empty() {
            // All bindings live in descriptor set 0 (single set model).
            self.descriptor_set_info.push(HgiVulkanDescriptorSetInfo {
                set_number: 0,
                bindings: Vec::new(),
            });
        }

        let stage_flags = HgiVulkanConversions::get_shader_stages(self.shader_stage);

        let binding = vk::DescriptorSetLayoutBinding::default()
            .binding(binding_index)
            .descriptor_type(descriptor_type)
            .descriptor_count(descriptor_count)
            .stage_flags(stage_flags);

        // SAFETY: `binding` has no immutable samplers (p_immutable_samplers = null),
        // so erasing the borrow lifetime to 'static is safe — the binding carries
        // no actual references.
        let binding: vk::DescriptorSetLayoutBinding<'static> =
            unsafe { std::mem::transmute(binding) };

        self.descriptor_set_info[0].bindings.push(binding);
        self.descriptor_set_layouts_added = true;
    }

    // -----------------------------------------------------------------------
    // GLSL header emitters
    // -----------------------------------------------------------------------

    fn write_version(&self, out: &mut String) {
        let version = self.capabilities.get_shader_version();
        out.push_str(&format!("#version {version}\n"));
    }

    fn write_extensions(&self, out: &mut String) {
        let version = self.capabilities.get_shader_version();
        let caps = self.capabilities.base_capabilities();
        let shader_draw = caps.supports(HgiDeviceCapabilities::SHADER_DRAW_PARAMETERS);
        let builtin_bary = caps.supports(HgiDeviceCapabilities::BUILTIN_BARYCENTRICS);

        if self.shader_stage.contains(HgiShaderStage::VERTEX) {
            if version < 460 && shader_draw {
                out.push_str("#extension GL_ARB_shader_draw_parameters : require\n");
            }
            if shader_draw {
                out.push_str("int HgiGetBaseVertex() {\n");
                if version < 460 {
                    out.push_str("  return gl_BaseVertexARB;\n");
                } else {
                    out.push_str("  return gl_BaseVertex;\n");
                }
                out.push_str("}\n");

                out.push_str("int HgiGetBaseInstance() {\n");
                if version < 460 {
                    out.push_str("  return gl_BaseInstanceARB;\n");
                } else {
                    out.push_str("  return gl_BaseInstance;\n");
                }
                out.push_str("}\n");
            }
        }

        if self.shader_stage.contains(HgiShaderStage::FRAGMENT) && builtin_bary {
            out.push_str("#extension GL_EXT_fragment_shader_barycentric: require\n");
        }
    }

    fn write_macros(&self, out: &mut String) {
        out.push_str(concat!(
            "#define REF(space,type) inout type\n",
            "#define FORWARD_DECL(func_decl) func_decl\n",
            "#define ATOMIC_LOAD(a) (a)\n",
            "#define ATOMIC_STORE(a, v) (a) = (v)\n",
            "#define ATOMIC_ADD(a, v) atomicAdd(a, v)\n",
            "#define ATOMIC_EXCHANGE(a, v) atomicExchange(a, v)\n",
            "#define ATOMIC_COMP_SWAP(a, expected, desired) atomicCompSwap(a, expected, desired)\n",
            "#define atomic_int int\n",
            "#define atomic_uint uint\n",
            "#define hd_SampleMask gl_SampleMask[0]\n",
        ));
        out.push_str("\n#define HGI_HAS_DOUBLE_TYPE 1\n\n");
    }
}

// ---------------------------------------------------------------------------
// write_layout_attributes — module-local helper
// ---------------------------------------------------------------------------

fn write_layout_attributes(attributes: &[HgiShaderSectionAttribute], out: &mut String) {
    if attributes.is_empty() {
        return;
    }
    out.push_str("layout(");
    for (i, attr) in attributes.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&attr.identifier);
        if !attr.index.is_empty() {
            out.push_str(" = ");
            out.push_str(&attr.index);
        }
    }
    out.push_str(") ");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hgi::{HgiShaderFunctionDesc, HgiShaderStage};

    fn default_caps() -> HgiVulkanCapabilities {
        HgiVulkanCapabilities::default()
    }

    #[test]
    fn vertex_shader_has_version_line() {
        let desc = HgiShaderFunctionDesc {
            shader_stage: HgiShaderStage::VERTEX,
            shader_code: "void main() {}".to_string(),
            ..Default::default()
        };
        let shader_gen = HgiVulkanShaderGenerator::new(&desc, default_caps());
        let code = shader_gen.execute();
        assert!(code.starts_with("#version 450\n"), "got: {code}");
    }

    #[test]
    fn macros_block_present() {
        let desc = HgiShaderFunctionDesc {
            shader_stage: HgiShaderStage::FRAGMENT,
            shader_code: "void main() {}".to_string(),
            ..Default::default()
        };
        let shader_gen = HgiVulkanShaderGenerator::new(&desc, default_caps());
        let code = shader_gen.execute();
        assert!(
            code.contains("#define REF(space,type) inout type"),
            "got: {code}"
        );
        assert!(
            code.contains("#define HGI_HAS_DOUBLE_TYPE 1"),
            "got: {code}"
        );
    }

    #[test]
    fn packed_type_definitions_present() {
        let desc = HgiShaderFunctionDesc {
            shader_stage: HgiShaderStage::VERTEX,
            shader_code: "void main() {}".to_string(),
            ..Default::default()
        };
        let shader_gen = HgiVulkanShaderGenerator::new(&desc, default_caps());
        let code = shader_gen.execute();
        assert!(code.contains("struct hgi_vec3"), "got: {code}");
        assert!(code.contains("struct hgi_mat3"), "got: {code}");
    }

    #[test]
    fn compute_layout_attribute() {
        use usd_hgi::HgiShaderFunctionComputeDesc;
        let desc = HgiShaderFunctionDesc {
            shader_stage: HgiShaderStage::COMPUTE,
            shader_code: "void main() {}".to_string(),
            compute_descriptor: HgiShaderFunctionComputeDesc {
                local_size: [16, 8, 1],
            },
            ..Default::default()
        };
        let shader_gen = HgiVulkanShaderGenerator::new(&desc, default_caps());
        let code = shader_gen.execute();
        assert!(
            code.contains("layout(local_size_x = 16, local_size_y = 8, local_size_z = 1) in;"),
            "got: {code}"
        );
    }

    #[test]
    fn push_constant_block_emitted() {
        use usd_hgi::shader_function::add_constant_param;
        let mut desc = HgiShaderFunctionDesc {
            shader_stage: HgiShaderStage::VERTEX,
            shader_code: "void main() {}".to_string(),
            ..Default::default()
        };
        add_constant_param(&mut desc, "transform", "mat4", "");
        let shader_gen = HgiVulkanShaderGenerator::new(&desc, default_caps());
        let code = shader_gen.execute();
        assert!(
            code.contains("layout(push_constant) uniform ParamBuffer"),
            "got: {code}"
        );
        assert!(code.contains("mat4 transform;"), "got: {code}");
    }

    #[test]
    fn in_out_location_auto_increment() {
        use usd_hgi::shader_function::{add_stage_input, add_stage_output};
        let mut desc = HgiShaderFunctionDesc {
            shader_stage: HgiShaderStage::VERTEX,
            shader_code: "void main() {}".to_string(),
            ..Default::default()
        };
        add_stage_input(&mut desc, "inPos", "vec4", "");
        add_stage_input(&mut desc, "inNorm", "vec3", "");
        add_stage_output(&mut desc, "outColor", "vec4", "", "");
        let shader_gen = HgiVulkanShaderGenerator::new(&desc, default_caps());
        let code = shader_gen.execute();
        assert!(
            code.contains("layout(location = 0) in vec4 inPos;"),
            "got: {code}"
        );
        assert!(
            code.contains("layout(location = 1) in vec3 inNorm;"),
            "got: {code}"
        );
        assert!(
            code.contains("layout(location = 0) out vec4 outColor;"),
            "got: {code}"
        );
    }

    #[test]
    fn keyword_section_for_builtin_role() {
        use usd_hgi::HgiShaderFunctionParamDesc;
        let desc = HgiShaderFunctionDesc {
            shader_stage: HgiShaderStage::VERTEX,
            shader_code: "void main() {}".to_string(),
            stage_inputs: vec![HgiShaderFunctionParamDesc {
                name_in_shader: "position".to_string(),
                type_name: "vec4".to_string(),
                role: "hdPosition".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let shader_gen = HgiVulkanShaderGenerator::new(&desc, default_caps());
        let code = shader_gen.execute();
        assert!(code.contains("vec4 position = gl_Position;"), "got: {code}");
    }

    #[test]
    fn texture_binding_starts_after_buffer() {
        use usd_hgi::{HgiBindingType, HgiShaderFunctionBufferDesc, HgiShaderFunctionTextureDesc};
        let desc = HgiShaderFunctionDesc {
            shader_stage: HgiShaderStage::FRAGMENT,
            shader_code: "void main() {}".to_string(),
            buffers: vec![HgiShaderFunctionBufferDesc {
                name_in_shader: "myBuf".to_string(),
                type_name: "float".to_string(),
                bind_index: 2,
                binding: HgiBindingType::UniformValue,
                ..Default::default()
            }],
            textures: vec![HgiShaderFunctionTextureDesc {
                name_in_shader: "myTex".to_string(),
                bind_index: 0,
                ..Default::default()
            }],
            ..Default::default()
        };
        let shader_gen = HgiVulkanShaderGenerator::new(&desc, default_caps());
        let code = shader_gen.execute();
        // texture_bind_index_start = max(0, 2+1) = 3, plus desc.bind_index 0 → binding = 3
        assert!(
            code.contains("binding = 3"),
            "expected binding=3, got: {code}"
        );
    }

    #[test]
    fn descriptor_set_info_sorted() {
        use usd_hgi::{HgiBindingType, HgiShaderFunctionBufferDesc};
        let desc = HgiShaderFunctionDesc {
            shader_stage: HgiShaderStage::FRAGMENT,
            shader_code: "void main() {}".to_string(),
            buffers: vec![
                HgiShaderFunctionBufferDesc {
                    name_in_shader: "b1".to_string(),
                    type_name: "float".to_string(),
                    bind_index: 5,
                    binding: HgiBindingType::UniformValue,
                    ..Default::default()
                },
                HgiShaderFunctionBufferDesc {
                    name_in_shader: "b0".to_string(),
                    type_name: "float".to_string(),
                    bind_index: 1,
                    binding: HgiBindingType::UniformValue,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut shader_gen = HgiVulkanShaderGenerator::new(&desc, default_caps());
        let info = shader_gen.descriptor_set_info();
        assert_eq!(info.len(), 1);
        let bindings = &info[0].bindings;
        assert_eq!(bindings[0].binding, 1, "expected sorted: 1 before 5");
        assert_eq!(bindings[1].binding, 5);
    }

    #[test]
    fn gl_position_not_re_declared_as_out() {
        use usd_hgi::HgiShaderFunctionParamDesc;
        let desc = HgiShaderFunctionDesc {
            shader_stage: HgiShaderStage::VERTEX,
            shader_code: "void main() {}".to_string(),
            stage_outputs: vec![HgiShaderFunctionParamDesc {
                name_in_shader: "gl_Position".to_string(),
                type_name: "vec4".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let shader_gen = HgiVulkanShaderGenerator::new(&desc, default_caps());
        let code = shader_gen.execute();
        assert!(
            !code.contains("out vec4 gl_Position"),
            "gl_Position must not be re-declared: {code}"
        );
    }
}
