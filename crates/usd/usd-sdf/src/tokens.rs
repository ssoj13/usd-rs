//! SDF tokens and constants.
//!
//! This module provides static tokens and character/string constants
//! used throughout the SDF (Scene Description Foundation) module.

use std::sync::OnceLock;

use usd_tf::Token;

/// Path delimiter characters and strings.
pub mod path_chars {
    /// Absolute path indicator character.
    pub const ABSOLUTE_INDICATOR: char = '/';
    /// Absolute path indicator string.
    pub const ABSOLUTE_INDICATOR_STR: &str = "/";
    /// Relative root character (single dot).
    pub const RELATIVE_ROOT: char = '.';
    /// Relative root string.
    pub const RELATIVE_ROOT_STR: &str = ".";
    /// Child delimiter character (forward slash).
    pub const CHILD_DELIMITER: char = '/';
    /// Child delimiter string.
    pub const CHILD_DELIMITER_STR: &str = "/";
    /// Namespace delimiter character (colon).
    pub const NS_DELIMITER: char = ':';
    /// Namespace delimiter string.
    pub const NS_DELIMITER_STR: &str = ":";
    /// Relationship target start character.
    pub const RELATIONSHIP_TARGET_START: char = '[';
    /// Relationship target start string.
    pub const RELATIONSHIP_TARGET_START_STR: &str = "[";
    /// Relationship target end character.
    pub const RELATIONSHIP_TARGET_END: char = ']';
    /// Relationship target end string.
    pub const RELATIONSHIP_TARGET_END_STR: &str = "]";
    /// Property delimiter character (period).
    pub const PROPERTY_DELIMITER: char = '.';
    /// Property delimiter string.
    pub const PROPERTY_DELIMITER_STR: &str = ".";
    /// Variant selection start character.
    pub const VARIANT_START: char = '{';
    /// Variant selection start string.
    pub const VARIANT_START_STR: &str = "{";
    /// Variant selection end character.
    pub const VARIANT_END: char = '}';
    /// Variant selection end string.
    pub const VARIANT_END_STR: &str = "}";
    /// Variant selection separator character.
    pub const VARIANT_SEPARATOR: char = '=';
    /// Variant selection separator string.
    pub const VARIANT_SEPARATOR_STR: &str = "=";
}

/// Collection of SDF path tokens.
#[derive(Debug)]
pub struct SdfPathTokens {
    /// The absolute path indicator token.
    pub absolute_indicator: Token,
    /// The relative root token.
    pub relative_root: Token,
    /// The child delimiter token.
    pub child_delimiter: Token,
    /// The property delimiter token.
    pub property_delimiter: Token,
    /// The relationship target start token.
    pub relationship_target_start: Token,
    /// The relationship target end token.
    pub relationship_target_end: Token,
    /// The parent path element token ("..").
    pub parent_path_element: Token,
    /// The mapper indicator token.
    pub mapper_indicator: Token,
    /// The expression indicator token.
    pub expression_indicator: Token,
    /// The mapper arg delimiter token.
    pub mapper_arg_delimiter: Token,
    /// The namespace delimiter token.
    pub namespace_delimiter: Token,
    /// The empty token.
    pub empty: Token,
}

impl SdfPathTokens {
    /// Creates a new set of SDF path tokens.
    fn new() -> Self {
        Self {
            absolute_indicator: Token::new(path_chars::ABSOLUTE_INDICATOR_STR),
            relative_root: Token::new(path_chars::RELATIVE_ROOT_STR),
            child_delimiter: Token::new(path_chars::CHILD_DELIMITER_STR),
            property_delimiter: Token::new(path_chars::PROPERTY_DELIMITER_STR),
            relationship_target_start: Token::new(path_chars::RELATIONSHIP_TARGET_START_STR),
            relationship_target_end: Token::new(path_chars::RELATIONSHIP_TARGET_END_STR),
            parent_path_element: Token::new(".."),
            mapper_indicator: Token::new("mapper"),
            expression_indicator: Token::new("expression"),
            mapper_arg_delimiter: Token::new("."),
            namespace_delimiter: Token::new(path_chars::NS_DELIMITER_STR),
            empty: Token::empty(),
        }
    }

    /// Returns the singleton instance of SDF path tokens.
    pub fn get_instance() -> &'static SdfPathTokens {
        static INSTANCE: OnceLock<SdfPathTokens> = OnceLock::new();
        INSTANCE.get_or_init(SdfPathTokens::new)
    }
}

/// Returns the global SDF path tokens instance.
pub fn path_tokens() -> &'static SdfPathTokens {
    SdfPathTokens::get_instance()
}

/// Collection of general SDF tokens.
#[derive(Debug)]
pub struct SdfTokens {
    /// The any type token.
    pub any_type: Token,
}

impl SdfTokens {
    /// Creates a new set of general SDF tokens.
    fn new() -> Self {
        Self {
            any_type: Token::new("__AnyType__"),
        }
    }

    /// Returns the singleton instance of general SDF tokens.
    pub fn get_instance() -> &'static SdfTokens {
        static INSTANCE: OnceLock<SdfTokens> = OnceLock::new();
        INSTANCE.get_or_init(SdfTokens::new)
    }
}

/// Returns the global SDF tokens instance.
pub fn sdf_tokens() -> &'static SdfTokens {
    SdfTokens::get_instance()
}

/// Metadata display group tokens.
#[derive(Debug)]
pub struct SdfMetadataDisplayGroupTokens {
    /// Core metadata group (empty string).
    pub core: Token,
    /// Internal metadata group.
    pub internal: Token,
    /// Direct manipulation metadata group.
    pub dmanip: Token,
    /// Pipeline metadata group.
    pub pipeline: Token,
    /// Symmetry metadata group.
    pub symmetry: Token,
    /// User interface metadata group.
    pub ui: Token,
}

impl SdfMetadataDisplayGroupTokens {
    /// Creates a new set of metadata display group tokens.
    fn new() -> Self {
        Self {
            core: Token::empty(),
            internal: Token::new("Internal"),
            dmanip: Token::new("Direct Manip"),
            pipeline: Token::new("Pipeline"),
            symmetry: Token::new("Symmetry"),
            ui: Token::new("User Interface"),
        }
    }

    /// Returns the singleton instance of metadata display group tokens.
    pub fn get_instance() -> &'static SdfMetadataDisplayGroupTokens {
        static INSTANCE: OnceLock<SdfMetadataDisplayGroupTokens> = OnceLock::new();
        INSTANCE.get_or_init(SdfMetadataDisplayGroupTokens::new)
    }
}

/// Returns the global metadata display group tokens instance.
pub fn metadata_display_group_tokens() -> &'static SdfMetadataDisplayGroupTokens {
    SdfMetadataDisplayGroupTokens::get_instance()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_chars() {
        assert_eq!(path_chars::ABSOLUTE_INDICATOR, '/');
        assert_eq!(path_chars::RELATIVE_ROOT, '.');
        assert_eq!(path_chars::CHILD_DELIMITER, '/');
        assert_eq!(path_chars::NS_DELIMITER, ':');
        assert_eq!(path_chars::PROPERTY_DELIMITER, '.');
        assert_eq!(path_chars::RELATIONSHIP_TARGET_START, '[');
        assert_eq!(path_chars::RELATIONSHIP_TARGET_END, ']');
        assert_eq!(path_chars::VARIANT_START, '{');
        assert_eq!(path_chars::VARIANT_END, '}');
        assert_eq!(path_chars::VARIANT_SEPARATOR, '=');
    }

    #[test]
    fn test_path_tokens_singleton() {
        let t1 = SdfPathTokens::get_instance();
        let t2 = SdfPathTokens::get_instance();
        assert!(std::ptr::eq(t1, t2));
    }

    #[test]
    fn test_path_tokens_values() {
        let tokens = path_tokens();
        assert_eq!(tokens.absolute_indicator.as_str(), "/");
        assert_eq!(tokens.relative_root.as_str(), ".");
        assert_eq!(tokens.child_delimiter.as_str(), "/");
        assert_eq!(tokens.property_delimiter.as_str(), ".");
        assert_eq!(tokens.relationship_target_start.as_str(), "[");
        assert_eq!(tokens.relationship_target_end.as_str(), "]");
        assert_eq!(tokens.parent_path_element.as_str(), "..");
        assert_eq!(tokens.mapper_indicator.as_str(), "mapper");
        assert_eq!(tokens.expression_indicator.as_str(), "expression");
        assert_eq!(tokens.namespace_delimiter.as_str(), ":");
        assert!(tokens.empty.is_empty());
    }

    #[test]
    fn test_sdf_tokens() {
        let tokens = sdf_tokens();
        assert_eq!(tokens.any_type.as_str(), "__AnyType__");
    }

    #[test]
    fn test_metadata_display_group_tokens() {
        let tokens = metadata_display_group_tokens();
        assert!(tokens.core.is_empty());
        assert_eq!(tokens.internal.as_str(), "Internal");
        assert_eq!(tokens.dmanip.as_str(), "Direct Manip");
        assert_eq!(tokens.pipeline.as_str(), "Pipeline");
        assert_eq!(tokens.symmetry.as_str(), "Symmetry");
        assert_eq!(tokens.ui.as_str(), "User Interface");
    }
}
