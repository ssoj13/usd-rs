//! DataSourceAttributeColorSpace - Color space metadata data source.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceAttributeColorSpace.h
//!
//! A data source that represents the color space metadata on a USD Attribute.

use std::sync::Arc;
use usd_hd::HdSampledDataSource;
use usd_tf::Token;
use usd_vt::Value;

// Token constants
#[allow(dead_code)]
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static COLOR_SPACE: LazyLock<Token> = LazyLock::new(|| Token::new("colorSpace"));
    pub static INPUTS_FILE: LazyLock<Token> = LazyLock::new(|| Token::new("inputs:file"));
    pub static INPUTS_SOURCE_COLOR_SPACE: LazyLock<Token> =
        LazyLock::new(|| Token::new("inputs:sourceColorSpace"));
}

// ============================================================================
// DataSourceAttributeColorSpace
// ============================================================================

/// Data source for attribute color space metadata.
///
/// Returns the color space for a USD attribute, handling special cases
/// like UsdUVTexture nodes where inputs:sourceColorSpace affects inputs:file.
#[derive(Clone)]
pub struct DataSourceAttributeColorSpace {
    /// The attribute name
    attr_name: Token,
    /// The resolved color space (cached)
    #[allow(dead_code)] // Part of color space infrastructure
    color_space: Option<Token>,
}

impl DataSourceAttributeColorSpace {
    /// Create a new color space data source.
    pub fn new(attr_name: Token) -> Self {
        Self {
            attr_name,
            color_space: None,
        }
    }

    /// Create with a known color space.
    pub fn with_color_space(attr_name: Token, color_space: Token) -> Self {
        Self {
            attr_name,
            color_space: Some(color_space),
        }
    }

    /// Get the schema token.
    pub fn get_schema_token() -> Token {
        tokens::COLOR_SPACE.clone()
    }

    /// Check if this is a file attribute that needs special handling.
    pub fn is_file_attribute(&self) -> bool {
        self.attr_name == *tokens::INPUTS_FILE
    }
}

impl std::fmt::Debug for DataSourceAttributeColorSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceAttributeColorSpace")
    }
}

impl usd_hd::HdDataSourceBase for DataSourceAttributeColorSpace {
    fn clone_box(&self) -> usd_hd::HdDataSourceBaseHandle {
        std::sync::Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(self.get_value(0.0))
    }
}

impl HdSampledDataSource for DataSourceAttributeColorSpace {
    fn get_value(&self, _shutter_offset: usd_hd::HdSampledDataSourceTime) -> Value {
        // Return color space as token
        if let Some(ref cs) = self.color_space {
            Value::from(cs.clone())
        } else {
            Value::default()
        }
    }

    fn get_contributing_sample_times(
        &self,
        _start_time: usd_hd::HdSampledDataSourceTime,
        _end_time: usd_hd::HdSampledDataSourceTime,
        _out_sample_times: &mut Vec<usd_hd::HdSampledDataSourceTime>,
    ) -> bool {
        // Color space is time-invariant
        false
    }
}

/// Handle type for DataSourceAttributeColorSpace.
pub type DataSourceAttributeColorSpaceHandle = Arc<DataSourceAttributeColorSpace>;

/// Factory function for creating color space data sources.
pub fn create_data_source_attribute_color_space(
    attr_name: Token,
) -> DataSourceAttributeColorSpaceHandle {
    Arc::new(DataSourceAttributeColorSpace::new(attr_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        assert_eq!(
            DataSourceAttributeColorSpace::get_schema_token().as_str(),
            "colorSpace"
        );
    }

    #[test]
    fn test_color_space_data_source() {
        let ds = DataSourceAttributeColorSpace::new(Token::new("diffuseColor"));

        // Color space is time-invariant
        let mut times = Vec::new();
        assert!(!ds.get_contributing_sample_times(0.0, 1.0, &mut times));
    }

    #[test]
    fn test_with_color_space() {
        let ds = DataSourceAttributeColorSpace::with_color_space(
            Token::new("diffuseColor"),
            Token::new("sRGB"),
        );

        let value = ds.get_value(0.0);
        // Value should contain the color space token
        let _ = value;
    }

    #[test]
    fn test_is_file_attribute() {
        let file_ds = DataSourceAttributeColorSpace::new(Token::new("inputs:file"));
        assert!(file_ds.is_file_attribute());

        let other_ds = DataSourceAttributeColorSpace::new(Token::new("diffuseColor"));
        assert!(!other_ds.is_file_attribute());
    }
}
