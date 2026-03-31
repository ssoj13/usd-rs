//! Metal shader section types for MSL code generation.
//! Port of pxr/imaging/hgiMetal/shaderSection
//!
//! Defines various shader section types that generate Metal Shading Language
//! code fragments. Each section knows how to declare itself in different
//! scopes (global, struct, entry point, etc.).

use usd_hgi::{HgiBindingType, HgiFormat, HgiShaderSectionAttribute, HgiShaderTextureType};

/// Base trait for Metal shader sections.
/// Extends HgiShaderSection with Metal-specific visitor methods.
///
/// Mirrors C++ HgiMetalShaderSection.
pub trait HgiMetalShaderSection: Send + Sync {
    /// Section identifier
    fn identifier(&self) -> &str;

    /// Section attributes
    fn attributes(&self) -> &[HgiShaderSectionAttribute];

    /// Write out the type
    fn write_type(&self, _output: &mut String) {}

    /// Write as a parameter
    fn write_parameter(&self, output: &mut String) {
        self.write_type(output);
        output.push(' ');
        output.push_str(self.identifier());
    }

    /// Write a declaration
    fn write_declaration(&self, output: &mut String) {
        self.write_type(output);
        output.push(' ');
        output.push_str(self.identifier());
    }

    /// Write attributes with index
    fn write_attributes_with_index(&self, output: &mut String) {
        for attr in self.attributes() {
            output.push_str(" [[");
            output.push_str(&attr.identifier);
            if !attr.index.is_empty() {
                output.push('(');
                output.push_str(&attr.index);
                output.push(')');
            }
            output.push_str("]]");
        }
    }

    // Metal-specific visitor hooks
    fn visit_global_macros(&self, _output: &mut String) -> bool {
        false
    }
    fn visit_global_member_declarations(&self, _output: &mut String) -> bool {
        false
    }
    fn visit_scope_structs(&self, _output: &mut String) -> bool {
        false
    }
    fn visit_scope_member_declarations(&self, _output: &mut String) -> bool {
        false
    }
    fn visit_scope_function_definitions(&self, _output: &mut String) -> bool {
        false
    }
    fn visit_scope_constructor_declarations(&self, _output: &mut String) -> bool {
        false
    }
    fn visit_scope_constructor_initialization(&self, _output: &mut String) -> bool {
        false
    }
    fn visit_scope_constructor_instantiation(&self, _output: &mut String) -> bool {
        false
    }
    fn visit_entry_point_parameter_declarations(&self, _output: &mut String) -> bool {
        false
    }
    fn visit_entry_point_function_executions(
        &self,
        _output: &mut String,
        _scope_instance_name: &str,
    ) -> bool {
        false
    }
}

// -- Concrete section types --

/// A shader section for defining macros.
/// Mirrors C++ HgiMetalMacroShaderSection.
pub struct HgiMetalMacroShaderSection {
    identifier: String,
    attributes: Vec<HgiShaderSectionAttribute>,
    macro_comment: String,
}

impl HgiMetalMacroShaderSection {
    pub fn new(macro_declaration: &str, macro_comment: &str) -> Self {
        Self {
            identifier: macro_declaration.to_string(),
            attributes: Vec::new(),
            macro_comment: macro_comment.to_string(),
        }
    }
}

impl HgiMetalShaderSection for HgiMetalMacroShaderSection {
    fn identifier(&self) -> &str {
        &self.identifier
    }
    fn attributes(&self) -> &[HgiShaderSectionAttribute] {
        &self.attributes
    }

    fn visit_global_macros(&self, output: &mut String) -> bool {
        if !self.macro_comment.is_empty() {
            output.push_str("// ");
            output.push_str(&self.macro_comment);
            output.push('\n');
        }
        output.push_str(&self.identifier);
        output.push('\n');
        true
    }
}

/// Defines a member within the scope.
/// Mirrors C++ HgiMetalMemberShaderSection.
pub struct HgiMetalMemberShaderSection {
    identifier: String,
    type_name: String,
    qualifiers: String,
    attributes: Vec<HgiShaderSectionAttribute>,
    array_size: String,
    block_instance_identifier: String,
}

impl HgiMetalMemberShaderSection {
    pub fn new(
        identifier: &str,
        type_name: &str,
        qualifiers: &str,
        attributes: Vec<HgiShaderSectionAttribute>,
        array_size: &str,
        block_instance_identifier: &str,
    ) -> Self {
        Self {
            identifier: identifier.to_string(),
            type_name: type_name.to_string(),
            qualifiers: qualifiers.to_string(),
            attributes,
            array_size: array_size.to_string(),
            block_instance_identifier: block_instance_identifier.to_string(),
        }
    }
}

impl HgiMetalShaderSection for HgiMetalMemberShaderSection {
    fn identifier(&self) -> &str {
        &self.identifier
    }
    fn attributes(&self) -> &[HgiShaderSectionAttribute] {
        &self.attributes
    }

    fn write_type(&self, output: &mut String) {
        if !self.qualifiers.is_empty() {
            output.push_str(&self.qualifiers);
            output.push(' ');
        }
        output.push_str(&self.type_name);
    }

    fn write_parameter(&self, output: &mut String) {
        self.write_type(output);
        output.push(' ');
        output.push_str(&self.identifier);
        if !self.array_size.is_empty() {
            output.push('[');
            output.push_str(&self.array_size);
            output.push(']');
        }
    }

    fn visit_scope_member_declarations(&self, output: &mut String) -> bool {
        self.write_type(output);
        output.push(' ');
        output.push_str(&self.identifier);
        if !self.array_size.is_empty() {
            output.push('[');
            output.push_str(&self.array_size);
            output.push(']');
        }
        if !self.block_instance_identifier.is_empty() {
            output.push_str(" ");
            output.push_str(&self.block_instance_identifier);
        }
        self.write_attributes_with_index(output);
        output.push_str(";\n");
        true
    }
}

/// Creates a texture sampler shader section.
/// Mirrors C++ HgiMetalSamplerShaderSection.
pub struct HgiMetalSamplerShaderSection {
    identifier: String,
    texture_shared_identifier: String,
    parent_scope_identifier: String,
    array_of_samplers_size: u32,
    attributes: Vec<HgiShaderSectionAttribute>,
}

impl HgiMetalSamplerShaderSection {
    pub fn new(
        texture_shared_identifier: &str,
        parent_scope_identifier: &str,
        array_of_samplers_size: u32,
        attributes: Vec<HgiShaderSectionAttribute>,
    ) -> Self {
        let identifier = format!("{}_sampler", texture_shared_identifier);
        Self {
            identifier,
            texture_shared_identifier: texture_shared_identifier.to_string(),
            parent_scope_identifier: parent_scope_identifier.to_string(),
            array_of_samplers_size,
            attributes,
        }
    }
}

impl HgiMetalShaderSection for HgiMetalSamplerShaderSection {
    fn identifier(&self) -> &str {
        &self.identifier
    }
    fn attributes(&self) -> &[HgiShaderSectionAttribute] {
        &self.attributes
    }

    fn write_type(&self, output: &mut String) {
        if self.array_of_samplers_size > 0 {
            output.push_str("array<sampler, ");
            output.push_str(&self.array_of_samplers_size.to_string());
            output.push('>');
        } else {
            output.push_str("sampler");
        }
    }

    fn write_parameter(&self, output: &mut String) {
        self.write_type(output);
        output.push(' ');
        output.push_str(&self.identifier);
        self.write_attributes_with_index(output);
    }

    fn visit_scope_constructor_declarations(&self, output: &mut String) -> bool {
        self.write_type(output);
        output.push(' ');
        output.push_str(&self.identifier);
        true
    }

    fn visit_scope_constructor_initialization(&self, output: &mut String) -> bool {
        output.push_str(&self.identifier);
        output.push('(');
        output.push_str(&self.identifier);
        output.push(')');
        true
    }

    fn visit_scope_constructor_instantiation(&self, output: &mut String) -> bool {
        output.push_str(&self.parent_scope_identifier);
        output.push('.');
        output.push_str(&self.identifier);
        true
    }

    fn visit_scope_member_declarations(&self, output: &mut String) -> bool {
        self.write_type(output);
        output.push(' ');
        output.push_str(&self.identifier);
        output.push_str(";\n");
        true
    }
}

/// Declares a texture with sampler and helper sampling function.
/// Mirrors C++ HgiMetalTextureShaderSection.
pub struct HgiMetalTextureShaderSection {
    identifier: String,
    sampler_shared_identifier: String,
    parent_scope_identifier: String,
    attributes: Vec<HgiShaderSectionAttribute>,
    dimensions: u32,
    format: HgiFormat,
    texture_type: HgiShaderTextureType,
    array_of_textures_size: u32,
    writable: bool,
    base_type: String,
    return_type: String,
}

impl HgiMetalTextureShaderSection {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sampler_shared_identifier: &str,
        parent_scope_identifier: &str,
        attributes: Vec<HgiShaderSectionAttribute>,
        dimensions: u32,
        format: HgiFormat,
        texture_type: HgiShaderTextureType,
        array_of_textures_size: u32,
        writable: bool,
        _default_value: &str,
    ) -> Self {
        let base_type = if format.is_float() {
            "float".to_string()
        } else {
            "int".to_string()
        };
        let return_type = format!("{}4", base_type);

        Self {
            identifier: sampler_shared_identifier.to_string(),
            sampler_shared_identifier: sampler_shared_identifier.to_string(),
            parent_scope_identifier: parent_scope_identifier.to_string(),
            attributes,
            dimensions,
            format,
            texture_type,
            array_of_textures_size,
            writable,
            base_type,
            return_type,
        }
    }
}

impl HgiMetalShaderSection for HgiMetalTextureShaderSection {
    fn identifier(&self) -> &str {
        &self.identifier
    }
    fn attributes(&self) -> &[HgiShaderSectionAttribute] {
        &self.attributes
    }

    fn write_type(&self, output: &mut String) {
        if self.writable {
            output.push_str("texture");
        } else {
            output.push_str("texture");
        }
        output.push_str(&self.dimensions.to_string());
        output.push_str("d<");
        output.push_str(&self.base_type);
        if self.writable {
            output.push_str(", access::read_write");
        }
        output.push('>');
    }

    fn write_parameter(&self, output: &mut String) {
        self.write_type(output);
        output.push(' ');
        output.push_str(&self.identifier);
        self.write_attributes_with_index(output);
    }

    fn visit_scope_constructor_declarations(&self, output: &mut String) -> bool {
        self.write_type(output);
        output.push(' ');
        output.push_str(&self.identifier);
        true
    }

    fn visit_scope_constructor_initialization(&self, output: &mut String) -> bool {
        output.push_str(&self.identifier);
        output.push('(');
        output.push_str(&self.identifier);
        output.push(')');
        true
    }

    fn visit_scope_constructor_instantiation(&self, output: &mut String) -> bool {
        output.push_str(&self.parent_scope_identifier);
        output.push('.');
        output.push_str(&self.identifier);
        true
    }

    fn visit_scope_member_declarations(&self, output: &mut String) -> bool {
        self.write_type(output);
        output.push(' ');
        output.push_str(&self.identifier);
        output.push_str(";\n");
        true
    }

    fn visit_scope_function_definitions(&self, output: &mut String) -> bool {
        // Generate sampling helper function
        output.push_str(&self.return_type);
        output.push_str(" HgiGet_");
        output.push_str(&self.sampler_shared_identifier);
        output.push_str("() { return ");
        output.push_str(&self.return_type);
        output.push_str("(0); }\n");
        true
    }
}

/// Declares a buffer binding.
/// Mirrors C++ HgiMetalBufferShaderSection.
pub struct HgiMetalBufferShaderSection {
    identifier: String,
    type_name: String,
    binding: HgiBindingType,
    writable: bool,
    unused: bool,
    parent_scope_identifier: String,
    attributes: Vec<HgiShaderSectionAttribute>,
}

impl HgiMetalBufferShaderSection {
    pub fn new(
        identifier: &str,
        parent_scope_identifier: &str,
        type_name: &str,
        binding: HgiBindingType,
        writable: bool,
        attributes: Vec<HgiShaderSectionAttribute>,
    ) -> Self {
        Self {
            identifier: identifier.to_string(),
            type_name: type_name.to_string(),
            binding,
            writable,
            unused: false,
            parent_scope_identifier: parent_scope_identifier.to_string(),
            attributes,
        }
    }

    /// Create a dummy padded binding point.
    pub fn new_unused(identifier: &str, attributes: Vec<HgiShaderSectionAttribute>) -> Self {
        Self {
            identifier: identifier.to_string(),
            type_name: String::new(),
            binding: HgiBindingType::Value,
            writable: false,
            unused: true,
            parent_scope_identifier: String::new(),
            attributes,
        }
    }
}

impl HgiMetalShaderSection for HgiMetalBufferShaderSection {
    fn identifier(&self) -> &str {
        &self.identifier
    }
    fn attributes(&self) -> &[HgiShaderSectionAttribute] {
        &self.attributes
    }

    fn write_type(&self, output: &mut String) {
        if self.unused {
            output.push_str("void*");
            return;
        }
        match self.binding {
            HgiBindingType::Pointer => {
                if self.writable {
                    output.push_str("device ");
                } else {
                    output.push_str("const device ");
                }
                output.push_str(&self.type_name);
                output.push('*');
            }
            _ => {
                if self.writable {
                    output.push_str("device ");
                } else {
                    output.push_str("const device ");
                }
                output.push_str(&self.type_name);
                output.push('*');
            }
        }
    }

    fn write_parameter(&self, output: &mut String) {
        self.write_type(output);
        output.push(' ');
        output.push_str(&self.identifier);
        self.write_attributes_with_index(output);
    }

    fn visit_scope_member_declarations(&self, output: &mut String) -> bool {
        if self.unused {
            return false;
        }
        self.write_type(output);
        output.push(' ');
        output.push_str(&self.identifier);
        output.push_str(";\n");
        true
    }

    fn visit_scope_constructor_declarations(&self, output: &mut String) -> bool {
        if self.unused {
            return false;
        }
        self.write_type(output);
        output.push(' ');
        output.push_str(&self.identifier);
        true
    }

    fn visit_scope_constructor_initialization(&self, output: &mut String) -> bool {
        if self.unused {
            return false;
        }
        output.push_str(&self.identifier);
        output.push('(');
        output.push_str(&self.identifier);
        output.push(')');
        true
    }

    fn visit_scope_constructor_instantiation(&self, output: &mut String) -> bool {
        if self.unused {
            return false;
        }
        output.push_str(&self.parent_scope_identifier);
        output.push('.');
        output.push_str(&self.identifier);
        true
    }
}

/// Defines a struct type declaration with members.
/// Mirrors C++ HgiMetalStructTypeDeclarationShaderSection.
pub struct HgiMetalStructTypeDeclarationShaderSection {
    identifier: String,
    attributes: Vec<HgiShaderSectionAttribute>,
    member_identifiers: Vec<String>,
    template_wrapper: String,
    template_wrapper_parameters: String,
}

impl HgiMetalStructTypeDeclarationShaderSection {
    pub fn new(
        identifier: &str,
        member_identifiers: Vec<String>,
        template_wrapper: &str,
        template_wrapper_parameters: &str,
    ) -> Self {
        Self {
            identifier: identifier.to_string(),
            attributes: Vec::new(),
            member_identifiers,
            template_wrapper: template_wrapper.to_string(),
            template_wrapper_parameters: template_wrapper_parameters.to_string(),
        }
    }

    pub fn members(&self) -> &[String] {
        &self.member_identifiers
    }

    pub fn write_template_wrapper(&self, output: &mut String) {
        if !self.template_wrapper.is_empty() {
            output.push_str(&self.template_wrapper);
            if !self.template_wrapper_parameters.is_empty() {
                output.push('<');
                output.push_str(&self.template_wrapper_parameters);
                output.push('>');
            }
        }
    }
}

impl HgiMetalShaderSection for HgiMetalStructTypeDeclarationShaderSection {
    fn identifier(&self) -> &str {
        &self.identifier
    }
    fn attributes(&self) -> &[HgiShaderSectionAttribute] {
        &self.attributes
    }

    fn write_type(&self, output: &mut String) {
        output.push_str(&self.identifier);
    }

    fn write_declaration(&self, output: &mut String) {
        output.push_str("struct ");
        output.push_str(&self.identifier);
        output.push_str(" {\n");
        // Members would be written here by the generator
        output.push_str("};\n");
    }

    fn write_parameter(&self, output: &mut String) {
        output.push_str(&self.identifier);
    }
}

/// Instance of a struct type.
/// Mirrors C++ HgiMetalStructInstanceShaderSection.
pub struct HgiMetalStructInstanceShaderSection {
    identifier: String,
    attributes: Vec<HgiShaderSectionAttribute>,
    struct_type_identifier: String,
    default_value: String,
}

impl HgiMetalStructInstanceShaderSection {
    pub fn new(
        identifier: &str,
        attributes: Vec<HgiShaderSectionAttribute>,
        struct_type_identifier: &str,
        default_value: &str,
    ) -> Self {
        Self {
            identifier: identifier.to_string(),
            attributes,
            struct_type_identifier: struct_type_identifier.to_string(),
            default_value: default_value.to_string(),
        }
    }

    pub fn struct_type_identifier(&self) -> &str {
        &self.struct_type_identifier
    }
}

impl HgiMetalShaderSection for HgiMetalStructInstanceShaderSection {
    fn identifier(&self) -> &str {
        &self.identifier
    }
    fn attributes(&self) -> &[HgiShaderSectionAttribute] {
        &self.attributes
    }

    fn write_type(&self, output: &mut String) {
        output.push_str(&self.struct_type_identifier);
    }
}

/// An input struct parameter to a shader stage.
/// Mirrors C++ HgiMetalParameterInputShaderSection.
pub struct HgiMetalParameterInputShaderSection {
    identifier: String,
    attributes: Vec<HgiShaderSectionAttribute>,
    address_space: String,
    is_pointer: bool,
    struct_type_identifier: String,
}

impl HgiMetalParameterInputShaderSection {
    pub fn new(
        identifier: &str,
        attributes: Vec<HgiShaderSectionAttribute>,
        address_space: &str,
        is_pointer: bool,
        struct_type_identifier: &str,
    ) -> Self {
        Self {
            identifier: identifier.to_string(),
            attributes,
            address_space: address_space.to_string(),
            is_pointer,
            struct_type_identifier: struct_type_identifier.to_string(),
        }
    }
}

impl HgiMetalShaderSection for HgiMetalParameterInputShaderSection {
    fn identifier(&self) -> &str {
        &self.identifier
    }
    fn attributes(&self) -> &[HgiShaderSectionAttribute] {
        &self.attributes
    }

    fn write_type(&self, output: &mut String) {
        output.push_str(&self.struct_type_identifier);
    }

    fn write_parameter(&self, output: &mut String) {
        if !self.address_space.is_empty() {
            output.push_str(&self.address_space);
            output.push(' ');
        }
        output.push_str(&self.struct_type_identifier);
        if self.is_pointer {
            output.push('*');
        }
        output.push(' ');
        output.push_str(&self.identifier);
        self.write_attributes_with_index(output);
    }

    fn visit_entry_point_parameter_declarations(&self, output: &mut String) -> bool {
        self.write_parameter(output);
        true
    }

    fn visit_entry_point_function_executions(
        &self,
        output: &mut String,
        _scope_instance_name: &str,
    ) -> bool {
        if self.is_pointer {
            output.push('*');
        }
        output.push_str(&self.identifier);
        true
    }

    fn visit_global_member_declarations(&self, _output: &mut String) -> bool {
        false
    }
}

/// An argument buffer for bindless buffer bindings.
/// Mirrors C++ HgiMetalArgumentBufferInputShaderSection.
pub struct HgiMetalArgumentBufferInputShaderSection {
    identifier: String,
    attributes: Vec<HgiShaderSectionAttribute>,
    address_space: String,
    is_pointer: bool,
    struct_type_identifier: String,
}

impl HgiMetalArgumentBufferInputShaderSection {
    pub fn new(
        identifier: &str,
        attributes: Vec<HgiShaderSectionAttribute>,
        address_space: &str,
        is_pointer: bool,
        struct_type_identifier: &str,
    ) -> Self {
        Self {
            identifier: identifier.to_string(),
            attributes,
            address_space: address_space.to_string(),
            is_pointer,
            struct_type_identifier: struct_type_identifier.to_string(),
        }
    }
}

impl HgiMetalShaderSection for HgiMetalArgumentBufferInputShaderSection {
    fn identifier(&self) -> &str {
        &self.identifier
    }
    fn attributes(&self) -> &[HgiShaderSectionAttribute] {
        &self.attributes
    }

    fn write_parameter(&self, output: &mut String) {
        if !self.address_space.is_empty() {
            output.push_str(&self.address_space);
            output.push(' ');
        }
        output.push_str(&self.struct_type_identifier);
        if self.is_pointer {
            output.push('*');
        }
        output.push(' ');
        output.push_str(&self.identifier);
        self.write_attributes_with_index(output);
    }

    fn visit_entry_point_parameter_declarations(&self, output: &mut String) -> bool {
        self.write_parameter(output);
        true
    }

    fn visit_global_member_declarations(&self, _output: &mut String) -> bool {
        false
    }
}

/// Defines shader keyword inputs (e.g. thread_position_in_grid).
/// Mirrors C++ HgiMetalKeywordInputShaderSection.
pub struct HgiMetalKeywordInputShaderSection {
    identifier: String,
    type_name: String,
    attributes: Vec<HgiShaderSectionAttribute>,
}

impl HgiMetalKeywordInputShaderSection {
    pub fn new(
        identifier: &str,
        type_name: &str,
        attributes: Vec<HgiShaderSectionAttribute>,
    ) -> Self {
        Self {
            identifier: identifier.to_string(),
            type_name: type_name.to_string(),
            attributes,
        }
    }
}

impl HgiMetalShaderSection for HgiMetalKeywordInputShaderSection {
    fn identifier(&self) -> &str {
        &self.identifier
    }
    fn attributes(&self) -> &[HgiShaderSectionAttribute] {
        &self.attributes
    }

    fn write_type(&self, output: &mut String) {
        output.push_str(&self.type_name);
    }

    fn visit_scope_member_declarations(&self, output: &mut String) -> bool {
        output.push_str(&self.type_name);
        output.push(' ');
        output.push_str(&self.identifier);
        output.push_str(";\n");
        true
    }

    fn visit_entry_point_parameter_declarations(&self, output: &mut String) -> bool {
        output.push_str(&self.type_name);
        output.push(' ');
        output.push_str(&self.identifier);
        self.write_attributes_with_index(output);
        true
    }

    fn visit_entry_point_function_executions(
        &self,
        output: &mut String,
        _scope_instance_name: &str,
    ) -> bool {
        output.push_str(&self.identifier);
        true
    }
}

/// Defines shader stage outputs.
/// Mirrors C++ HgiMetalStageOutputShaderSection.
pub struct HgiMetalStageOutputShaderSection {
    identifier: String,
    attributes: Vec<HgiShaderSectionAttribute>,
    struct_type_identifier: String,
    address_space: String,
    is_pointer: bool,
}

impl HgiMetalStageOutputShaderSection {
    pub fn new(identifier: &str, struct_type_identifier: &str) -> Self {
        Self {
            identifier: identifier.to_string(),
            attributes: Vec::new(),
            struct_type_identifier: struct_type_identifier.to_string(),
            address_space: String::new(),
            is_pointer: false,
        }
    }

    pub fn new_with_address_space(
        identifier: &str,
        attributes: Vec<HgiShaderSectionAttribute>,
        address_space: &str,
        is_pointer: bool,
        struct_type_identifier: &str,
    ) -> Self {
        Self {
            identifier: identifier.to_string(),
            attributes,
            struct_type_identifier: struct_type_identifier.to_string(),
            address_space: address_space.to_string(),
            is_pointer,
        }
    }
}

impl HgiMetalShaderSection for HgiMetalStageOutputShaderSection {
    fn identifier(&self) -> &str {
        &self.identifier
    }
    fn attributes(&self) -> &[HgiShaderSectionAttribute] {
        &self.attributes
    }

    fn write_type(&self, output: &mut String) {
        output.push_str(&self.struct_type_identifier);
    }

    fn visit_entry_point_function_executions(
        &self,
        output: &mut String,
        _scope_instance_name: &str,
    ) -> bool {
        output.push_str(&self.identifier);
        true
    }

    fn visit_global_member_declarations(&self, _output: &mut String) -> bool {
        false
    }
}

/// Defines an interstage interface block.
/// Mirrors C++ HgiMetalInterstageBlockShaderSection.
pub struct HgiMetalInterstageBlockShaderSection {
    block_identifier: String,
    block_instance_identifier: String,
    struct_type_identifier: String,
    attributes: Vec<HgiShaderSectionAttribute>,
}

impl HgiMetalInterstageBlockShaderSection {
    pub fn new(
        block_identifier: &str,
        block_instance_identifier: &str,
        struct_type_identifier: &str,
    ) -> Self {
        Self {
            block_identifier: block_identifier.to_string(),
            block_instance_identifier: block_instance_identifier.to_string(),
            struct_type_identifier: struct_type_identifier.to_string(),
            attributes: Vec::new(),
        }
    }

    pub fn struct_type_identifier(&self) -> &str {
        &self.struct_type_identifier
    }
}

impl HgiMetalShaderSection for HgiMetalInterstageBlockShaderSection {
    fn identifier(&self) -> &str {
        &self.block_identifier
    }
    fn attributes(&self) -> &[HgiShaderSectionAttribute] {
        &self.attributes
    }

    fn visit_scope_structs(&self, output: &mut String) -> bool {
        output.push_str("struct ");
        output.push_str(&self.struct_type_identifier);
        output.push_str(" {};\n");
        true
    }

    fn visit_scope_member_declarations(&self, output: &mut String) -> bool {
        output.push_str(&self.struct_type_identifier);
        output.push(' ');
        output.push_str(&self.block_instance_identifier);
        output.push_str(";\n");
        true
    }
}
