//! InbetweenShapeSchema - Hydra schema for blend shape inbetweens.
//!
//! Port of pxr/usdImaging/usdSkelImaging/inbetweenShapeSchema.h
//!
//! Corresponds to UsdSkelInbetweenShape. Each instance corresponds to a group
//! of attributes on a UsdSkelBlendShape that share a prefix inbetweens:NAME.

use std::sync::Arc;
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdRetainedContainerDataSource,
    HdTypedSampledDataSource,
};
use usd_hd::schema::HdSchema;
use usd_tf::Token;

// Token constants (USD_SKEL_IMAGING_INBETWEEN_SHAPE_SCHEMA_TOKENS)
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static WEIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("weight"));
    pub static OFFSETS: LazyLock<Token> = LazyLock::new(|| Token::new("offsets"));
    pub static NORMAL_OFFSETS: LazyLock<Token> = LazyLock::new(|| Token::new("normalOffsets"));
}

/// Handle to f32 data source.
pub type HdFloatDataSourceHandle = Arc<dyn HdTypedSampledDataSource<f32> + Send + Sync>;

/// Handle to Vec3f array data source.
pub type HdVec3fArrayDataSourceHandle =
    Arc<dyn HdTypedSampledDataSource<Vec<usd_gf::vec3::Vec3f>> + Send + Sync>;

// ============================================================================
// InbetweenShapeSchema
// ============================================================================

/// Schema for blend shape inbetween data in Hydra.
///
/// Corresponds to UsdSkelInbetweenShape. Contains weight and point/normal offsets.
#[derive(Debug, Clone)]
pub struct InbetweenShapeSchema {
    schema: HdSchema,
}

impl InbetweenShapeSchema {
    /// Create schema from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Create schema from optional container.
    pub fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self {
            schema: container.map(HdSchema::new).unwrap_or_else(HdSchema::empty),
        }
    }

    /// Get the underlying container.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Check if this schema is defined.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get weight data source.
    pub fn get_weight(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema
            .get_typed_retained::<f32>(&tokens::WEIGHT)
            .map(|source| source as HdFloatDataSourceHandle)
    }

    /// Get offsets (point offsets) data source.
    pub fn get_offsets(&self) -> Option<HdVec3fArrayDataSourceHandle> {
        self.schema
            .get_typed_retained::<Vec<usd_gf::vec3::Vec3f>>(&tokens::OFFSETS)
            .map(|source| source as HdVec3fArrayDataSourceHandle)
    }

    /// Get normal offsets data source.
    pub fn get_normal_offsets(&self) -> Option<HdVec3fArrayDataSourceHandle> {
        self.schema
            .get_typed_retained::<Vec<usd_gf::vec3::Vec3f>>(&tokens::NORMAL_OFFSETS)
            .map(|source| source as HdVec3fArrayDataSourceHandle)
    }

    /// Get schema from parent container.
    pub fn get_from_parent(_parent: &HdContainerDataSourceHandle) -> Self {
        // InbetweenShape is typically nested under blendShapes; parent lookup
        // is context-dependent
        Self::from_container(None)
    }

    /// Build retained container.
    pub fn build_retained(
        weight: Option<HdFloatDataSourceHandle>,
        offsets: Option<HdVec3fArrayDataSourceHandle>,
        normal_offsets: Option<HdVec3fArrayDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();
        if let Some(w) = weight {
            entries.push((tokens::WEIGHT.clone(), w as HdDataSourceBaseHandle));
        }
        if let Some(o) = offsets {
            entries.push((tokens::OFFSETS.clone(), o as HdDataSourceBaseHandle));
        }
        if let Some(n) = normal_offsets {
            entries.push((tokens::NORMAL_OFFSETS.clone(), n as HdDataSourceBaseHandle));
        }
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

// ============================================================================
// InbetweenShapeSchemaBuilder
// ============================================================================

/// Builder for InbetweenShapeSchema data sources.
#[derive(Debug, Default)]
pub struct InbetweenShapeSchemaBuilder {
    weight: Option<HdFloatDataSourceHandle>,
    offsets: Option<HdVec3fArrayDataSourceHandle>,
    normal_offsets: Option<HdVec3fArrayDataSourceHandle>,
}

impl InbetweenShapeSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the weight data source.
    pub fn set_weight(mut self, v: HdFloatDataSourceHandle) -> Self {
        self.weight = Some(v);
        self
    }

    /// Sets the point offsets data source.
    pub fn set_offsets(mut self, v: HdVec3fArrayDataSourceHandle) -> Self {
        self.offsets = Some(v);
        self
    }

    /// Sets the normal offsets data source.
    pub fn set_normal_offsets(mut self, v: HdVec3fArrayDataSourceHandle) -> Self {
        self.normal_offsets = Some(v);
        self
    }

    /// Builds the container data source from the configured fields.
    pub fn build(self) -> HdContainerDataSourceHandle {
        InbetweenShapeSchema::build_retained(self.weight, self.offsets, self.normal_offsets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::data_source::HdRetainedTypedSampledDataSource;

    #[test]
    fn test_tokens() {
        assert_eq!(tokens::WEIGHT.as_str(), "weight");
        assert_eq!(tokens::OFFSETS.as_str(), "offsets");
        assert_eq!(tokens::NORMAL_OFFSETS.as_str(), "normalOffsets");
    }

    #[test]
    fn test_build_retained() {
        let weight = HdRetainedTypedSampledDataSource::new(0.5f32);
        let container = InbetweenShapeSchema::build_retained(Some(weight), None, None);
        let schema = InbetweenShapeSchema::new(container);
        assert!(schema.is_defined());
    }
}
