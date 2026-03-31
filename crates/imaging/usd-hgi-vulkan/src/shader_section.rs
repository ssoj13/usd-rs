//! Vulkan shader sections for GLSL code generation
//!
//! Port of pxr/imaging/hgiVulkan/shaderSection.h/.cpp
//!
//! Each section type knows how to write its own GLSL declaration into the
//! output string.  The shader generator collects sections and calls the
//! appropriate visit_* methods to build the final GLSL source.

use usd_hgi::{
    HgiBindingType, HgiFormat, HgiInterpolationType, HgiSamplingType, HgiShaderFunctionParamDesc,
    HgiShaderSectionAttribute, HgiShaderTextureType, HgiStorageType,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Write layout qualifier attributes as `layout(a = 0, b = 1) ` (with trailing space).
/// Does nothing if the attribute list is empty.
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

/// Returns the GLSL sampler/image type prefix that corresponds to the integer
/// format family of `format`.  Mirrors `_GetTextureTypePrefix` in C++.
fn texture_type_prefix(format: HgiFormat) -> &'static str {
    // UInt16 family → unsigned sampler/image prefix "u"
    if matches!(
        format,
        HgiFormat::UInt16 | HgiFormat::UInt16Vec2 | HgiFormat::UInt16Vec3 | HgiFormat::UInt16Vec4
    ) {
        return "u";
    }
    // Int32 family → signed sampler/image prefix "i"
    if matches!(
        format,
        HgiFormat::Int32 | HgiFormat::Int32Vec2 | HgiFormat::Int32Vec3 | HgiFormat::Int32Vec4
    ) {
        return "i";
    }
    ""
}

// ---------------------------------------------------------------------------
// Base section
// ---------------------------------------------------------------------------

/// Base Vulkan shader section.
///
/// Holds common fields shared by all section subtypes and provides the default
/// `write_declaration` / `write_parameter` implementations that match the C++
/// `HgiVulkanShaderSection` base class.
pub struct HgiVulkanShaderSection {
    pub identifier: String,
    pub attributes: Vec<HgiShaderSectionAttribute>,
    pub storage_qualifier: String,
    pub default_value: String,
    pub array_size: String,
    pub block_instance_identifier: String,
}

impl HgiVulkanShaderSection {
    pub fn new(
        identifier: impl Into<String>,
        attributes: Vec<HgiShaderSectionAttribute>,
        storage_qualifier: impl Into<String>,
        default_value: impl Into<String>,
        array_size: impl Into<String>,
        block_instance_identifier: impl Into<String>,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            attributes,
            storage_qualifier: storage_qualifier.into(),
            default_value: default_value.into(),
            array_size: array_size.into(),
            block_instance_identifier: block_instance_identifier.into(),
        }
    }

    /// Create a minimal section with just an identifier (all other fields empty).
    pub fn with_name(identifier: impl Into<String>) -> Self {
        Self::new(identifier, Vec::new(), "", "", "", "")
    }

    // --- write helpers ---

    pub fn write_identifier(&self, out: &mut String) {
        out.push_str(&self.identifier);
    }

    /// Write `[size]` when `array_size` is non-empty.
    pub fn write_array_size(&self, out: &mut String) {
        if !self.array_size.is_empty() {
            out.push('[');
            out.push_str(&self.array_size);
            out.push(']');
        }
    }

    pub fn write_block_instance_identifier(&self, out: &mut String) {
        if !self.block_instance_identifier.is_empty() {
            out.push_str(&self.block_instance_identifier);
        }
    }

    /// Default `write_type` — no-op; subclasses override by writing their own
    /// type string directly before calling declaration helpers.
    pub fn write_type(&self, _out: &mut String) {}

    /// Mirrors `HgiVulkanShaderSection::WriteDeclaration`:
    /// `layout(...) <storage> <type> <identifier>[size];\n`
    pub fn write_declaration(&self, out: &mut String) {
        write_layout_attributes(&self.attributes, out);
        if !self.storage_qualifier.is_empty() {
            out.push_str(&self.storage_qualifier);
            out.push(' ');
        }
        self.write_type(out);
        out.push(' ');
        self.write_identifier(out);
        self.write_array_size(out);
        out.push_str(";\n");
    }

    /// Mirrors `HgiVulkanShaderSection::WriteParameter`:
    /// `<type> <identifier>;`
    pub fn write_parameter(&self, out: &mut String) {
        self.write_type(out);
        out.push(' ');
        self.write_identifier(out);
        out.push(';');
    }

    pub fn has_block_instance_identifier(&self) -> bool {
        !self.block_instance_identifier.is_empty()
    }
}

// ---------------------------------------------------------------------------
// HgiVulkanMacroShaderSection
// ---------------------------------------------------------------------------

/// Emits a raw macro declaration into the global scope under `visit_global_macros`.
///
/// Corresponds to `HgiVulkanMacroShaderSection` in C++.
pub struct HgiVulkanMacroShaderSection {
    base: HgiVulkanShaderSection,
    /// Optional comment that accompanies the macro (currently unused in codegen).
    pub macro_comment: String,
}

impl HgiVulkanMacroShaderSection {
    pub fn new(macro_declaration: impl Into<String>, macro_comment: impl Into<String>) -> Self {
        Self {
            base: HgiVulkanShaderSection::with_name(macro_declaration),
            macro_comment: macro_comment.into(),
        }
    }

    /// Writes the macro identifier (the raw declaration string) to `out`.
    /// Returns `true` to signal the generator that output was produced.
    pub fn visit_global_macros(&self, out: &mut String) -> bool {
        self.base.write_identifier(out);
        true
    }
}

// ---------------------------------------------------------------------------
// HgiVulkanMemberShaderSection
// ---------------------------------------------------------------------------

/// Declares a member variable with optional interpolation / sampling / storage
/// qualifiers.  Emits into `visit_global_member_declarations`.
///
/// Corresponds to `HgiVulkanMemberShaderSection` in C++.
pub struct HgiVulkanMemberShaderSection {
    base: HgiVulkanShaderSection,
    pub type_name: String,
    pub interpolation: HgiInterpolationType,
    pub sampling: HgiSamplingType,
    pub storage: HgiStorageType,
}

impl HgiVulkanMemberShaderSection {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identifier: impl Into<String>,
        type_name: impl Into<String>,
        interpolation: HgiInterpolationType,
        sampling: HgiSamplingType,
        storage: HgiStorageType,
        attributes: Vec<HgiShaderSectionAttribute>,
        storage_qualifier: impl Into<String>,
        default_value: impl Into<String>,
        array_size: impl Into<String>,
        block_instance_identifier: impl Into<String>,
    ) -> Self {
        Self {
            base: HgiVulkanShaderSection::new(
                identifier,
                attributes,
                storage_qualifier,
                default_value,
                array_size,
                block_instance_identifier,
            ),
            type_name: type_name.into(),
            interpolation,
            sampling,
            storage,
        }
    }

    pub fn identifier(&self) -> &str {
        &self.base.identifier
    }

    pub fn write_identifier(&self, out: &mut String) {
        self.base.write_identifier(out);
    }

    /// `flat ` / `noperspective ` / nothing
    pub fn write_interpolation(&self, out: &mut String) {
        match self.interpolation {
            HgiInterpolationType::Default => {}
            HgiInterpolationType::Flat => out.push_str("flat "),
            HgiInterpolationType::NoPerspective => out.push_str("noperspective "),
        }
    }

    /// `centroid ` / `sample ` / nothing
    pub fn write_sampling(&self, out: &mut String) {
        match self.sampling {
            HgiSamplingType::Default => {}
            HgiSamplingType::Centroid => out.push_str("centroid "),
            HgiSamplingType::Sample => out.push_str("sample "),
        }
    }

    /// `patch ` / nothing
    pub fn write_storage(&self, out: &mut String) {
        match self.storage {
            HgiStorageType::Default => {}
            HgiStorageType::Patch => out.push_str("patch "),
        }
    }

    pub fn write_type(&self, out: &mut String) {
        out.push_str(&self.type_name);
    }

    /// Writes `layout(...) <interp> <sampling> <storage> <type> <id>[size];\n`
    /// Skips the declaration when the member belongs to an interface block
    /// (i.e. has a `block_instance_identifier`), matching C++ behaviour.
    pub fn visit_global_member_declarations(&self, out: &mut String) -> bool {
        if self.base.has_block_instance_identifier() {
            return true;
        }
        self.write_interpolation(out);
        self.write_sampling(out);
        self.write_storage(out);
        // Delegate to base write_declaration, but we must override write_type.
        // Re-implement inline since base.write_type is a no-op.
        write_layout_attributes(&self.base.attributes, out);
        if !self.base.storage_qualifier.is_empty() {
            out.push_str(&self.base.storage_qualifier);
            out.push(' ');
        }
        self.write_type(out);
        out.push(' ');
        self.base.write_identifier(out);
        self.base.write_array_size(out);
        out.push_str(";\n");
        true
    }
}

// ---------------------------------------------------------------------------
// HgiVulkanBlockShaderSection
// ---------------------------------------------------------------------------

/// Emits a `layout(push_constant) uniform <Name> { ... };\n` block.
///
/// Corresponds to `HgiVulkanBlockShaderSection` in C++.
pub struct HgiVulkanBlockShaderSection {
    base: HgiVulkanShaderSection,
    parameters: Vec<HgiShaderFunctionParamDesc>,
}

impl HgiVulkanBlockShaderSection {
    pub fn new(identifier: impl Into<String>, parameters: Vec<HgiShaderFunctionParamDesc>) -> Self {
        Self {
            base: HgiVulkanShaderSection::with_name(identifier),
            parameters,
        }
    }

    /// Writes:
    /// ```glsl
    /// layout(push_constant) uniform <Name>
    /// {
    ///     type member;
    ///     ...
    /// };
    /// ```
    pub fn visit_global_member_declarations(&self, out: &mut String) -> bool {
        out.push_str("layout(push_constant) uniform ");
        self.base.write_identifier(out);
        out.push('\n');
        out.push_str("{\n");
        for param in &self.parameters {
            out.push_str("    ");
            out.push_str(&param.type_name);
            out.push(' ');
            out.push_str(&param.name_in_shader);
            out.push_str(";\n");
        }
        out.push_str("\n};\n");
        true
    }
}

// ---------------------------------------------------------------------------
// HgiVulkanTextureShaderSection
// ---------------------------------------------------------------------------

/// Declares a texture sampler / image and generates the accessor functions
/// (`HgiGet_*`, `HgiSet_*`, `HgiGetSize_*`, `HgiTextureLod_*`, `HgiTexelFetch_*`).
///
/// Corresponds to `HgiVulkanTextureShaderSection` in C++.
pub struct HgiVulkanTextureShaderSection {
    base: HgiVulkanShaderSection,
    pub dimensions: u32,
    pub format: HgiFormat,
    pub texture_type: HgiShaderTextureType,
    /// When > 0 the binding is a texture array of this size.
    pub array_size: u32,
    pub writable: bool,
}

impl HgiVulkanTextureShaderSection {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identifier: impl Into<String>,
        _layout_index: u32,
        dimensions: u32,
        format: HgiFormat,
        texture_type: HgiShaderTextureType,
        array_size: u32,
        writable: bool,
        attributes: Vec<HgiShaderSectionAttribute>,
        default_value: impl Into<String>,
    ) -> Self {
        let array_size_str = if array_size > 0 {
            array_size.to_string()
        } else {
            String::new()
        };
        Self {
            base: HgiVulkanShaderSection::new(
                identifier,
                attributes,
                "uniform", // matches C++ static _storageQualifier = "uniform"
                default_value,
                array_size_str,
                "",
            ),
            dimensions,
            format,
            texture_type,
            array_size,
            writable,
        }
    }

    /// Writes the GLSL sampler/image type, e.g. `sampler2D`, `uimage3DArray`, …
    pub fn write_type(&self, out: &mut String) {
        self.write_sampler_type(out);
    }

    /// Writes the sampler or image type string.
    fn write_sampler_type(&self, out: &mut String) {
        if self.writable {
            match self.texture_type {
                HgiShaderTextureType::ArrayTexture => {
                    out.push_str("image");
                    out.push_str(&self.dimensions.to_string());
                    out.push_str("DArray");
                }
                HgiShaderTextureType::CubemapTexture => out.push_str("imageCube"),
                _ => {
                    out.push_str("image");
                    out.push_str(&self.dimensions.to_string());
                    out.push('D');
                }
            }
        } else {
            let prefix = texture_type_prefix(self.format);
            match self.texture_type {
                HgiShaderTextureType::ShadowTexture => {
                    out.push_str(prefix);
                    out.push_str("sampler");
                    out.push_str(&self.dimensions.to_string());
                    out.push_str("DShadow");
                }
                HgiShaderTextureType::ArrayTexture => {
                    out.push_str(prefix);
                    out.push_str("sampler");
                    out.push_str(&self.dimensions.to_string());
                    out.push_str("DArray");
                }
                HgiShaderTextureType::CubemapTexture => {
                    out.push_str(prefix);
                    out.push_str("samplerCube");
                }
                _ => {
                    out.push_str(prefix);
                    out.push_str("sampler");
                    out.push_str(&self.dimensions.to_string());
                    out.push('D');
                }
            }
        }
    }

    /// Writes the data type returned by sampling, e.g. `vec4`, `ivec4`, `float`.
    fn write_sampled_data_type(&self, out: &mut String) {
        if self.texture_type == HgiShaderTextureType::ShadowTexture {
            out.push_str("float");
        } else {
            out.push_str(texture_type_prefix(self.format));
            out.push_str("vec4");
        }
    }

    /// Emits `layout(...) uniform <sampler_type> <name>[size];\n`
    pub fn visit_global_member_declarations(&self, out: &mut String) -> bool {
        write_layout_attributes(&self.base.attributes, out);
        if !self.base.storage_qualifier.is_empty() {
            out.push_str(&self.base.storage_qualifier);
            out.push(' ');
        }
        self.write_type(out);
        out.push(' ');
        self.base.write_identifier(out);
        self.base.write_array_size(out);
        out.push_str(";\n");
        true
    }

    /// Emits the GLSL accessor helper functions:
    /// - `HgiGetSampler_<name>()`
    /// - writable: `HgiSet_<name>()`, `HgiGetSize_<name>()`
    /// - readable: `HgiGet_<name>()`, `HgiGetSize_<name>()`, `HgiTextureLod_<name>()`,
    ///   optionally `HgiTexelFetch_<name>()`
    pub fn visit_global_function_definitions(&self, out: &mut String) -> bool {
        // How many dimensions the size query returns (array textures add one component).
        let size_dim = if self.texture_type == HgiShaderTextureType::ArrayTexture {
            self.dimensions + 1
        } else {
            self.dimensions
        };
        // How many components coordinates require.
        let coord_dim = if matches!(
            self.texture_type,
            HgiShaderTextureType::ShadowTexture
                | HgiShaderTextureType::ArrayTexture
                | HgiShaderTextureType::CubemapTexture
        ) {
            self.dimensions + 1
        } else {
            self.dimensions
        };

        let size_type = if size_dim == 1 {
            "int".to_string()
        } else {
            format!("ivec{}", size_dim)
        };
        let int_coord_type = if coord_dim == 1 {
            "int".to_string()
        } else {
            format!("ivec{}", coord_dim)
        };
        let float_coord_type = if coord_dim == 1 {
            "float".to_string()
        } else {
            format!("vec{}", coord_dim)
        };

        // HgiGetSampler_<name>([index])
        if self.array_size > 0 {
            out.push_str("#define HgiGetSampler_");
            self.base.write_identifier(out);
            out.push_str("(index) ");
            self.base.write_identifier(out);
            out.push_str("[index]\n");
        } else {
            out.push_str("#define HgiGetSampler_");
            self.base.write_identifier(out);
            out.push_str("() ");
            self.base.write_identifier(out);
            out.push('\n');
        }

        if self.writable {
            // HgiSet_<name>(uv, data)
            out.push_str("void HgiSet_");
            self.base.write_identifier(out);
            out.push('(');
            out.push_str(&int_coord_type);
            out.push_str(" uv, vec4 data) {\n    imageStore(");
            self.base.write_identifier(out);
            out.push_str(", uv, data);\n}\n");

            // HgiGetSize_<name>()
            out.push_str(&size_type);
            out.push_str(" HgiGetSize_");
            self.base.write_identifier(out);
            out.push_str("() {\n    return imageSize(");
            self.base.write_identifier(out);
            out.push_str(");\n}\n");
        } else {
            let array_input = if self.array_size > 0 {
                "uint index, "
            } else {
                ""
            };
            let array_index = if self.array_size > 0 { "[index]" } else { "" };

            // HgiGet_<name>(uv)
            self.write_sampled_data_type(out);
            out.push_str(" HgiGet_");
            self.base.write_identifier(out);
            out.push('(');
            out.push_str(array_input);
            out.push_str(&float_coord_type);
            out.push_str(" uv) {\n    ");
            self.write_sampled_data_type(out);
            out.push_str(" result = texture(");
            self.base.write_identifier(out);
            out.push_str(array_index);
            out.push_str(", uv);\n    return result;\n}\n");

            // HgiGetSize_<name>()
            out.push_str(&size_type);
            out.push_str(" HgiGetSize_");
            self.base.write_identifier(out);
            out.push('(');
            if self.array_size > 0 {
                out.push_str("uint index");
            }
            out.push_str(") {\n    return textureSize(");
            self.base.write_identifier(out);
            out.push_str(array_index);
            out.push_str(", 0);\n}\n");

            // HgiTextureLod_<name>(coord, lod)
            self.write_sampled_data_type(out);
            out.push_str(" HgiTextureLod_");
            self.base.write_identifier(out);
            out.push('(');
            out.push_str(array_input);
            out.push_str(&float_coord_type);
            out.push_str(" coord, float lod) {\n    return textureLod(");
            self.base.write_identifier(out);
            out.push_str(array_index);
            out.push_str(", coord, lod);\n}\n");

            // HgiTexelFetch_<name>(coord) — not valid for shadow or cubemap
            if !matches!(
                self.texture_type,
                HgiShaderTextureType::ShadowTexture | HgiShaderTextureType::CubemapTexture
            ) {
                self.write_sampled_data_type(out);
                out.push_str(" HgiTexelFetch_");
                self.base.write_identifier(out);
                out.push('(');
                out.push_str(array_input);
                out.push_str(&int_coord_type);
                out.push_str(" coord) {\n    ");
                self.write_sampled_data_type(out);
                out.push_str(" result = texelFetch(");
                self.base.write_identifier(out);
                out.push_str(array_index);
                out.push_str(", coord, 0);\n    return result;\n}\n");
            }
        }

        true
    }
}

// ---------------------------------------------------------------------------
// HgiVulkanBufferShaderSection
// ---------------------------------------------------------------------------

/// Declares an SSBO or UBO binding.
///
/// Corresponds to `HgiVulkanBufferShaderSection` in C++.
pub struct HgiVulkanBufferShaderSection {
    base: HgiVulkanShaderSection,
    pub type_name: String,
    pub binding: HgiBindingType,
    /// Array size expression (may be empty for value bindings).
    pub array_size: String,
    pub writable: bool,
}

impl HgiVulkanBufferShaderSection {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identifier: impl Into<String>,
        _layout_index: u32,
        type_name: impl Into<String>,
        binding: HgiBindingType,
        array_size: impl Into<String>,
        writable: bool,
        attributes: Vec<HgiShaderSectionAttribute>,
    ) -> Self {
        Self {
            base: HgiVulkanShaderSection::new(
                identifier, attributes,
                "buffer", // overridden in VisitGlobalMemberDeclarations anyway
                "", "", "",
            ),
            type_name: type_name.into(),
            binding,
            array_size: array_size.into(),
            writable,
        }
    }

    pub fn write_type(&self, out: &mut String) {
        out.push_str(&self.type_name);
    }

    /// Emits a UBO (`uniform ubo_<name>`) or SSBO (`[readonly] buffer ssbo_<name>`) block.
    pub fn visit_global_member_declarations(&self, out: &mut String) -> bool {
        write_layout_attributes(&self.base.attributes, out);

        let is_uniform = matches!(
            self.binding,
            HgiBindingType::UniformValue | HgiBindingType::UniformArray
        );

        if is_uniform {
            out.push_str("uniform ubo_");
        } else {
            if !self.writable {
                out.push_str("readonly ");
            }
            out.push_str("buffer ssbo_");
        }
        self.base.write_identifier(out);
        out.push_str(" { ");
        self.write_type(out);
        out.push(' ');
        self.base.write_identifier(out);

        let is_value = matches!(
            self.binding,
            HgiBindingType::Value | HgiBindingType::UniformValue
        );
        if is_value {
            out.push_str("; };\n");
        } else {
            out.push('[');
            out.push_str(&self.array_size);
            out.push_str("]; };\n");
        }

        true
    }
}

// ---------------------------------------------------------------------------
// HgiVulkanKeywordShaderSection
// ---------------------------------------------------------------------------

/// Emits a built-in keyword alias, e.g. `vec4 position = gl_Position;\n`.
///
/// Corresponds to `HgiVulkanKeywordShaderSection` in C++.
pub struct HgiVulkanKeywordShaderSection {
    base: HgiVulkanShaderSection,
    pub type_name: String,
    pub keyword: String,
}

impl HgiVulkanKeywordShaderSection {
    pub fn new(
        identifier: impl Into<String>,
        type_name: impl Into<String>,
        keyword: impl Into<String>,
    ) -> Self {
        Self {
            base: HgiVulkanShaderSection::with_name(identifier),
            type_name: type_name.into(),
            keyword: keyword.into(),
        }
    }

    pub fn write_type(&self, out: &mut String) {
        out.push_str(&self.type_name);
    }

    /// Writes `<type> <identifier> = <keyword>;\n`
    pub fn visit_global_member_declarations(&self, out: &mut String) -> bool {
        self.write_type(out);
        out.push(' ');
        self.base.write_identifier(out);
        out.push_str(" = ");
        out.push_str(&self.keyword);
        out.push_str(";\n");
        true
    }
}

// ---------------------------------------------------------------------------
// HgiVulkanInterstageBlockShaderSection
// ---------------------------------------------------------------------------

/// Declares an interface block that passes data between shader stages.
///
/// Corresponds to `HgiVulkanInterstageBlockShaderSection` in C++.
/// Members are borrowed as shared references because the generator owns them
/// and may need to inspect them independently.
pub struct HgiVulkanInterstageBlockShaderSection {
    base: HgiVulkanShaderSection,
    qualifier: String,
    /// Non-owning references into the generator's member section storage.
    members: Vec<*const HgiVulkanMemberShaderSection>,
}

// SAFETY: The generator owns all member sections for at least as long as this
// block section lives, so the raw pointers remain valid.  This type is not
// intended to be sent across threads.
unsafe impl Send for HgiVulkanInterstageBlockShaderSection {}

impl HgiVulkanInterstageBlockShaderSection {
    pub fn new(
        block_identifier: impl Into<String>,
        block_instance_identifier: impl Into<String>,
        attributes: Vec<HgiShaderSectionAttribute>,
        qualifier: impl Into<String>,
        array_size: impl Into<String>,
        members: Vec<*const HgiVulkanMemberShaderSection>,
    ) -> Self {
        let qualifier_str: String = qualifier.into();
        Self {
            base: HgiVulkanShaderSection::new(
                block_identifier,
                attributes,
                qualifier_str.clone(),
                "",
                array_size,
                block_instance_identifier,
            ),
            qualifier: qualifier_str,
            members,
        }
    }

    /// Writes:
    /// ```glsl
    /// layout(...) <qualifier> <BlockName> {
    ///   <interp> <sampling> <storage> <type> <member>;
    ///   ...
    /// } <instanceName>[size];
    /// ```
    pub fn visit_global_member_declarations(&self, out: &mut String) -> bool {
        write_layout_attributes(&self.base.attributes, out);
        out.push_str(&self.qualifier);
        out.push(' ');
        self.base.write_identifier(out);
        out.push_str(" {\n");
        for &member_ptr in &self.members {
            // SAFETY: pointer is valid for the lifetime of the generator (see above).
            let member = unsafe { &*member_ptr };
            out.push_str("  ");
            member.write_interpolation(out);
            member.write_sampling(out);
            member.write_storage(out);
            member.write_type(out);
            out.push(' ');
            member.write_identifier(out);
            out.push_str(";\n");
        }
        out.push_str("} ");
        self.base.write_block_instance_identifier(out);
        self.base.write_array_size(out);
        out.push_str(";\n");
        true
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn attr(id: &str, idx: &str) -> HgiShaderSectionAttribute {
        HgiShaderSectionAttribute {
            identifier: id.to_string(),
            index: idx.to_string(),
        }
    }

    #[test]
    fn macro_section_emits_identifier() {
        let section = HgiVulkanMacroShaderSection::new("#define MY_MACRO 42", "some comment");
        let mut out = String::new();
        assert!(section.visit_global_macros(&mut out));
        assert_eq!(out, "#define MY_MACRO 42");
    }

    #[test]
    fn member_section_writes_declaration() {
        let section = HgiVulkanMemberShaderSection::new(
            "inColor",
            "vec4",
            HgiInterpolationType::Default,
            HgiSamplingType::Default,
            HgiStorageType::Default,
            vec![attr("location", "0")],
            "in",
            "",
            "",
            "",
        );
        let mut out = String::new();
        assert!(section.visit_global_member_declarations(&mut out));
        assert!(out.contains("layout(location = 0)"), "got: {out}");
        assert!(out.contains("in vec4 inColor;"), "got: {out}");
    }

    #[test]
    fn member_section_flat_interpolation() {
        let section = HgiVulkanMemberShaderSection::new(
            "flatVal",
            "int",
            HgiInterpolationType::Flat,
            HgiSamplingType::Default,
            HgiStorageType::Default,
            vec![],
            "in",
            "",
            "",
            "",
        );
        let mut out = String::new();
        section.visit_global_member_declarations(&mut out);
        assert!(out.starts_with("flat "), "got: {out}");
    }

    #[test]
    fn block_section_push_constant() {
        let params = vec![
            HgiShaderFunctionParamDesc {
                name_in_shader: "transform".to_string(),
                type_name: "mat4".to_string(),
                ..Default::default()
            },
            HgiShaderFunctionParamDesc {
                name_in_shader: "color".to_string(),
                type_name: "vec4".to_string(),
                ..Default::default()
            },
        ];
        let section = HgiVulkanBlockShaderSection::new("PushConstants", params);
        let mut out = String::new();
        assert!(section.visit_global_member_declarations(&mut out));
        assert!(
            out.contains("layout(push_constant) uniform PushConstants"),
            "got: {out}"
        );
        assert!(out.contains("mat4 transform;"), "got: {out}");
        assert!(out.contains("vec4 color;"), "got: {out}");
    }

    #[test]
    fn texture_section_sampler2d_declaration() {
        let section = HgiVulkanTextureShaderSection::new(
            "diffuseTex",
            0,
            2,
            HgiFormat::Float32Vec4,
            HgiShaderTextureType::Texture,
            0,
            false,
            vec![attr("set", "0"), attr("binding", "1")],
            "",
        );
        let mut out = String::new();
        section.visit_global_member_declarations(&mut out);
        assert_eq!(
            out,
            "layout(set = 0, binding = 1) uniform sampler2D diffuseTex;\n"
        );
    }

    #[test]
    fn texture_section_usampler_prefix() {
        let section = HgiVulkanTextureShaderSection::new(
            "uintTex",
            0,
            2,
            HgiFormat::UInt16Vec4,
            HgiShaderTextureType::Texture,
            0,
            false,
            vec![],
            "",
        );
        let mut out = String::new();
        section.visit_global_member_declarations(&mut out);
        assert!(out.contains("usampler2D"), "got: {out}");
    }

    #[test]
    fn texture_section_writable_image() {
        let section = HgiVulkanTextureShaderSection::new(
            "outputImage",
            0,
            2,
            HgiFormat::Float32Vec4,
            HgiShaderTextureType::Texture,
            0,
            true,
            vec![attr("set", "0"), attr("binding", "0")],
            "",
        );
        let mut out = String::new();
        section.visit_global_member_declarations(&mut out);
        assert!(out.contains("image2D"), "got: {out}");

        let mut fn_out = String::new();
        section.visit_global_function_definitions(&mut fn_out);
        assert!(fn_out.contains("HgiSet_outputImage"), "got: {fn_out}");
        assert!(fn_out.contains("imageStore"), "got: {fn_out}");
    }

    #[test]
    fn texture_section_readable_functions() {
        let section = HgiVulkanTextureShaderSection::new(
            "depthTex",
            0,
            2,
            HgiFormat::Float32Vec4,
            HgiShaderTextureType::Texture,
            0,
            false,
            vec![],
            "",
        );
        let mut out = String::new();
        section.visit_global_function_definitions(&mut out);
        assert!(out.contains("HgiGet_depthTex"), "got: {out}");
        assert!(out.contains("HgiGetSize_depthTex"), "got: {out}");
        assert!(out.contains("HgiTextureLod_depthTex"), "got: {out}");
        assert!(out.contains("HgiTexelFetch_depthTex"), "got: {out}");
    }

    #[test]
    fn buffer_section_ssbo() {
        let section = HgiVulkanBufferShaderSection::new(
            "primvars",
            0,
            "float",
            HgiBindingType::Array,
            "1024",
            false,
            vec![attr("set", "0"), attr("binding", "2")],
        );
        let mut out = String::new();
        assert!(section.visit_global_member_declarations(&mut out));
        assert!(out.contains("layout(set = 0, binding = 2)"), "got: {out}");
        assert!(out.contains("readonly buffer ssbo_primvars"), "got: {out}");
        assert!(out.contains("float primvars[1024]"), "got: {out}");
    }

    #[test]
    fn buffer_section_ubo() {
        let section = HgiVulkanBufferShaderSection::new(
            "constants",
            0,
            "mat4",
            HgiBindingType::UniformValue,
            "",
            true,
            vec![attr("set", "0"), attr("binding", "0")],
        );
        let mut out = String::new();
        section.visit_global_member_declarations(&mut out);
        assert!(out.contains("uniform ubo_constants"), "got: {out}");
    }

    #[test]
    fn keyword_section() {
        let section = HgiVulkanKeywordShaderSection::new("position", "vec4", "gl_Position");
        let mut out = String::new();
        assert!(section.visit_global_member_declarations(&mut out));
        assert_eq!(out, "vec4 position = gl_Position;\n");
    }
}
