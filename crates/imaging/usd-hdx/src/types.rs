
//! Common types for Hydra extensions

use usd_hgi::HgiFormat;
use usd_hio::HioFormat;
use usd_tf::Token;
use usd_vt::Dictionary;

/// Shader inputs used to communicate between application and Hydra
///
/// This structure packages shader parameters, textures, attributes,
/// and metadata for consumption by rendering tasks.
///
/// Note: `PartialEq` matches C++ `operator==` which excludes `metadata`.
#[derive(Debug, Clone)]
pub struct HdxShaderInputs {
    /// Shader parameter values (uniforms)
    pub parameters: Dictionary,

    /// Texture bindings
    pub textures: Dictionary,

    /// Fallback values for textures when not available
    pub texture_fallback_values: Dictionary,

    /// Vertex attribute names
    pub attributes: Vec<Token>,

    /// Additional metadata for shader configuration
    pub metadata: Dictionary,
}

impl HdxShaderInputs {
    /// Create new empty shader inputs
    pub fn new() -> Self {
        Self {
            parameters: Dictionary::new(),
            textures: Dictionary::new(),
            texture_fallback_values: Dictionary::new(),
            attributes: Vec::new(),
            metadata: Dictionary::new(),
        }
    }

    /// Create shader inputs with specific parameter dictionary
    pub fn with_parameters(parameters: Dictionary) -> Self {
        Self {
            parameters,
            textures: Dictionary::new(),
            texture_fallback_values: Dictionary::new(),
            attributes: Vec::new(),
            metadata: Dictionary::new(),
        }
    }

    /// Check if shader inputs are empty
    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
            && self.textures.is_empty()
            && self.texture_fallback_values.is_empty()
            && self.attributes.is_empty()
            && self.metadata.is_empty()
    }
}

impl PartialEq for HdxShaderInputs {
    /// Matches C++ `operator==`: compares parameters, textures, texture_fallback_values,
    /// and attributes. `metadata` is intentionally excluded.
    fn eq(&self, other: &Self) -> bool {
        self.parameters == other.parameters
            && self.textures == other.textures
            && self.texture_fallback_values == other.texture_fallback_values
            && self.attributes == other.attributes
    }
}

impl Default for HdxShaderInputs {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert HgiFormat to HioFormat
///
/// Maps Hydra Graphics Interface formats to Hydra Image I/O formats
/// for file writing and reading operations.
pub fn get_hio_format(hgi_format: HgiFormat) -> HioFormat {
    match hgi_format {
        HgiFormat::Invalid => HioFormat::Invalid,

        HgiFormat::UNorm8 => HioFormat::UNorm8,
        HgiFormat::UNorm8Vec2 => HioFormat::UNorm8Vec2,
        HgiFormat::UNorm8Vec4 => HioFormat::UNorm8Vec4,

        HgiFormat::SNorm8 => HioFormat::SNorm8,
        HgiFormat::SNorm8Vec2 => HioFormat::SNorm8Vec2,
        HgiFormat::SNorm8Vec4 => HioFormat::SNorm8Vec4,

        HgiFormat::Float16 => HioFormat::Float16,
        HgiFormat::Float16Vec2 => HioFormat::Float16Vec2,
        HgiFormat::Float16Vec3 => HioFormat::Float16Vec3,
        HgiFormat::Float16Vec4 => HioFormat::Float16Vec4,

        HgiFormat::Float32 => HioFormat::Float32,
        HgiFormat::Float32Vec2 => HioFormat::Float32Vec2,
        HgiFormat::Float32Vec3 => HioFormat::Float32Vec3,
        HgiFormat::Float32Vec4 => HioFormat::Float32Vec4,

        HgiFormat::Int16 => HioFormat::Int16,
        HgiFormat::Int16Vec2 => HioFormat::Int16Vec2,
        HgiFormat::Int16Vec3 => HioFormat::Int16Vec3,
        HgiFormat::Int16Vec4 => HioFormat::Int16Vec4,

        HgiFormat::UInt16 => HioFormat::UInt16,
        HgiFormat::UInt16Vec2 => HioFormat::UInt16Vec2,
        HgiFormat::UInt16Vec3 => HioFormat::UInt16Vec3,
        HgiFormat::UInt16Vec4 => HioFormat::UInt16Vec4,

        HgiFormat::Int32 => HioFormat::Int32,
        HgiFormat::Int32Vec2 => HioFormat::Int32Vec2,
        HgiFormat::Int32Vec3 => HioFormat::Int32Vec3,
        HgiFormat::Int32Vec4 => HioFormat::Int32Vec4,

        HgiFormat::UNorm8Vec4srgb => HioFormat::UNorm8Vec4Srgb,

        HgiFormat::BC6FloatVec3 => HioFormat::BC6FloatVec3,
        HgiFormat::BC6UFloatVec3 => HioFormat::BC6UFloatVec3,
        HgiFormat::BC7UNorm8Vec4 => HioFormat::BC7UNorm8Vec4,
        HgiFormat::BC7UNorm8Vec4srgb => HioFormat::BC7UNorm8Vec4Srgb,
        HgiFormat::BC1UNorm8Vec4 => HioFormat::BC1UNorm8Vec4,
        HgiFormat::BC3UNorm8Vec4 => HioFormat::BC3UNorm8Vec4,

        // These HgiFormat variants don't have direct HioFormat equivalents
        HgiFormat::Float32UInt8 => HioFormat::Invalid,
        HgiFormat::PackedD16Unorm => HioFormat::Invalid,
        HgiFormat::PackedInt1010102 => HioFormat::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_inputs_new() {
        let inputs = HdxShaderInputs::new();
        assert!(inputs.is_empty());
        assert!(inputs.parameters.is_empty());
        assert!(inputs.textures.is_empty());
        assert!(inputs.attributes.is_empty());
    }

    #[test]
    fn test_shader_inputs_default() {
        let inputs = HdxShaderInputs::default();
        assert!(inputs.is_empty());
    }

    #[test]
    fn test_shader_inputs_equality() {
        let inputs1 = HdxShaderInputs::new();
        let inputs2 = HdxShaderInputs::default();
        assert_eq!(inputs1, inputs2);
    }

    #[test]
    fn test_shader_inputs_metadata_excluded_from_eq() {
        // C++ operator== does NOT compare metadata field.
        // Two inputs with different metadata but same other fields must be equal.
        let mut inputs1 = HdxShaderInputs::new();
        let mut inputs2 = HdxShaderInputs::new();

        inputs1
            .metadata
            .insert("key".to_string(), usd_vt::Value::from(1));
        inputs2
            .metadata
            .insert("key".to_string(), usd_vt::Value::from(999));

        assert_eq!(inputs1, inputs2, "metadata must be excluded from PartialEq");
    }

    #[test]
    fn test_shader_inputs_parameters_included_in_eq() {
        let mut inputs1 = HdxShaderInputs::new();
        let inputs2 = HdxShaderInputs::new();

        inputs1
            .parameters
            .insert("val".to_string(), usd_vt::Value::from(1));
        // inputs2 has no parameter

        assert_ne!(inputs1, inputs2, "parameters must be included in PartialEq");
    }

    #[test]
    fn test_shader_inputs_with_parameters() {
        let mut params = Dictionary::new();
        params.insert("test".to_string(), usd_vt::Value::from(42));

        let inputs = HdxShaderInputs::with_parameters(params);
        assert!(!inputs.is_empty());
        assert!(!inputs.parameters.is_empty());
        assert!(inputs.textures.is_empty());
    }

    #[test]
    fn test_hgi_to_hio_format_conversion() {
        assert_eq!(get_hio_format(HgiFormat::Invalid), HioFormat::Invalid);
        assert_eq!(
            get_hio_format(HgiFormat::Float32Vec4),
            HioFormat::Float32Vec4
        );
        assert_eq!(
            get_hio_format(HgiFormat::UNorm8Vec4srgb),
            HioFormat::UNorm8Vec4Srgb
        );
    }

    #[test]
    fn test_format_roundtrip() {
        // Test that common formats map correctly
        let formats = vec![
            HgiFormat::Float32,
            HgiFormat::Float32Vec2,
            HgiFormat::Float32Vec3,
            HgiFormat::Float32Vec4,
            HgiFormat::UNorm8Vec4,
        ];

        for format in formats {
            let hio_format = get_hio_format(format);
            assert_ne!(hio_format, HioFormat::Invalid);
        }
    }
}
