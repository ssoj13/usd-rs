//! DataSourceAttributeTypeName - Type name metadata data source.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceAttributeTypeName.h
//!
//! A data source that represents the type name on a USD Attribute.

use std::sync::Arc;
use usd_hd::HdSampledDataSource;
use usd_tf::Token;
use usd_vt::Value;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static TYPE_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("typeName"));
}

// ============================================================================
// DataSourceAttributeTypeName
// ============================================================================

/// Data source for attribute type name metadata.
///
/// Returns the type name token for a USD attribute.
#[derive(Clone)]
pub struct DataSourceAttributeTypeName {
    /// The type name token
    type_name: Token,
}

impl DataSourceAttributeTypeName {
    /// Create a new type name data source.
    pub fn new(type_name: Token) -> Self {
        Self { type_name }
    }

    /// Get the schema token.
    pub fn get_schema_token() -> Token {
        tokens::TYPE_NAME.clone()
    }
}

impl std::fmt::Debug for DataSourceAttributeTypeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceAttributeTypeName")
    }
}

impl usd_hd::HdDataSourceBase for DataSourceAttributeTypeName {
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

impl HdSampledDataSource for DataSourceAttributeTypeName {
    fn get_value(&self, _shutter_offset: usd_hd::HdSampledDataSourceTime) -> Value {
        Value::from(self.type_name.clone())
    }

    fn get_contributing_sample_times(
        &self,
        _start_time: usd_hd::HdSampledDataSourceTime,
        _end_time: usd_hd::HdSampledDataSourceTime,
        _out_sample_times: &mut Vec<usd_hd::HdSampledDataSourceTime>,
    ) -> bool {
        // Type name is time-invariant
        false
    }
}

/// Handle type for DataSourceAttributeTypeName.
pub type DataSourceAttributeTypeNameHandle = Arc<DataSourceAttributeTypeName>;

/// Factory function for creating type name data sources.
pub fn create_data_source_attribute_type_name(
    type_name: Token,
) -> DataSourceAttributeTypeNameHandle {
    Arc::new(DataSourceAttributeTypeName::new(type_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        assert_eq!(
            DataSourceAttributeTypeName::get_schema_token().as_str(),
            "typeName"
        );
    }

    #[test]
    fn test_type_name_data_source() {
        let ds = DataSourceAttributeTypeName::new(Token::new("float3"));

        // Type name is time-invariant
        let mut times = Vec::new();
        assert!(!ds.get_contributing_sample_times(0.0, 1.0, &mut times));
    }

    #[test]
    fn test_get_value() {
        let ds = DataSourceAttributeTypeName::new(Token::new("double"));
        let _value = ds.get_value(0.0);
    }
}
