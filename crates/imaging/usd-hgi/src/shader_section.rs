//! Shader section for generated shader code
//!
//! Mirrors C++ HgiShaderSection from shaderSection.h.
//! Represents a section of generated shader code that knows how to
//! declare itself, define its type, and pass as a parameter.

use std::fmt::Write;

/// Attribute on a shader section (e.g. layout qualifiers)
#[derive(Debug, Clone, PartialEq)]
pub struct HgiShaderSectionAttribute {
    /// Attribute identifier (e.g. "location")
    pub identifier: String,
    /// Attribute index value (e.g. "0")
    pub index: String,
}

/// A base struct for a shader section.
///
/// In its simplest form it is a construct that knows how to declare itself,
/// define itself, and pass as a parameter. Can be subclassed to add more
/// behaviour for complex cases and to hook into the visitor tree.
pub struct HgiShaderSection {
    /// Unique name of this section instance
    identifier: String,
    /// Layout/qualifier attributes
    attributes: Vec<HgiShaderSectionAttribute>,
    /// Default value expression
    default_value: String,
    /// Array size expression (empty = not an array)
    array_size: String,
    /// Block instance identifier (for interface blocks)
    block_instance_identifier: String,
}

impl HgiShaderSection {
    /// Create a new shader section
    pub fn new(
        identifier: impl Into<String>,
        attributes: Vec<HgiShaderSectionAttribute>,
        default_value: impl Into<String>,
        array_size: impl Into<String>,
        block_instance_identifier: impl Into<String>,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            attributes,
            default_value: default_value.into(),
            array_size: array_size.into(),
            block_instance_identifier: block_instance_identifier.into(),
        }
    }

    /// Create a minimal section with just an identifier
    pub fn with_name(identifier: impl Into<String>) -> Self {
        Self::new(identifier, Vec::new(), "", "", "")
    }

    /// Returns the identifier of the section
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Returns the attributes of the section
    pub fn attributes(&self) -> &[HgiShaderSectionAttribute] {
        &self.attributes
    }

    /// Returns the array size of the section
    pub fn array_size(&self) -> &str {
        &self.array_size
    }

    /// Returns whether the section has a block instance identifier
    pub fn has_block_instance_identifier(&self) -> bool {
        !self.block_instance_identifier.is_empty()
    }

    /// Returns the default value
    pub fn default_value(&self) -> &str {
        &self.default_value
    }

    /// Write out the type. Base implementation is a no-op;
    /// subclasses fully control how the type is defined.
    pub fn write_type(&self, _out: &mut String) {
        // Base: no type written. Subclasses override.
    }

    /// Write the unique name of this section instance
    pub fn write_identifier(&self, out: &mut String) {
        out.push_str(&self.identifier);
    }

    /// Write a declaration statement for a member or in global scope
    pub fn write_declaration(&self, out: &mut String) {
        self.write_type(out);
        out.push(' ');
        self.write_identifier(out);
        self.write_array_size(out);
        if !self.default_value.is_empty() {
            let _ = write!(out, " = {}", self.default_value);
        }
    }

    /// Write the section as a parameter to a function
    pub fn write_parameter(&self, out: &mut String) {
        self.write_type(out);
        out.push(' ');
        self.write_identifier(out);
    }

    /// Write the array size
    pub fn write_array_size(&self, out: &mut String) {
        if !self.array_size.is_empty() {
            let _ = write!(out, "[{}]", self.array_size);
        }
    }

    /// Write the block instance identifier
    pub fn write_block_instance_identifier(&self, out: &mut String) {
        if !self.block_instance_identifier.is_empty() {
            out.push_str(&self.block_instance_identifier);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_section() {
        let section = HgiShaderSection::with_name("myVar");
        assert_eq!(section.identifier(), "myVar");
        assert!(section.attributes().is_empty());
        assert!(section.array_size().is_empty());
        assert!(!section.has_block_instance_identifier());
    }

    #[test]
    fn test_section_with_attributes() {
        let section = HgiShaderSection::new(
            "position",
            vec![HgiShaderSectionAttribute {
                identifier: "location".to_string(),
                index: "0".to_string(),
            }],
            "",
            "3",
            "",
        );

        assert_eq!(section.attributes().len(), 1);
        assert_eq!(section.array_size(), "3");

        let mut out = String::new();
        section.write_identifier(&mut out);
        assert_eq!(out, "position");

        let mut out2 = String::new();
        section.write_array_size(&mut out2);
        assert_eq!(out2, "[3]");
    }

    #[test]
    fn test_section_declaration() {
        let section = HgiShaderSection::new("color", Vec::new(), "vec4(1.0)", "", "");
        let mut out = String::new();
        section.write_declaration(&mut out);
        assert!(out.contains("color"));
        assert!(out.contains("= vec4(1.0)"));
    }

    #[test]
    fn test_block_instance() {
        let section = HgiShaderSection::new("", Vec::new(), "", "", "blockInst");
        assert!(section.has_block_instance_identifier());

        let mut out = String::new();
        section.write_block_instance_identifier(&mut out);
        assert_eq!(out, "blockInst");
    }
}
