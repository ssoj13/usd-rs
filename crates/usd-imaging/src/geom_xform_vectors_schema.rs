//! GeomXformVectorsSchema - Hydra schema for xform vector decomposition.
//!
//! Port of pxr/usdImaging/usdImaging/geomXformVectorsSchema.h
//!
//! Exposes the result of UsdGeomXformCommonAPI::GetXformVectorsByAccumulation().
//! Decomposition of USD transformation operations including pivot offset.
//! Read-only; does not participate in subsequent computations.

use std::sync::Arc;
use usd_gf::vec3::Vec3d;
use usd_gf::vec3::Vec3f;
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource, HdTypedSampledDataSource,
    cast_to_container,
};
use usd_hd::schema::HdSchema;
use usd_tf::Token;

// Token constants (USD_IMAGING_GEOM_XFORM_VECTORS_SCHEMA_TOKENS)
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static GEOM_XFORM_VECTORS: LazyLock<Token> =
        LazyLock::new(|| Token::new("geomXformVectors"));
    pub static TRANSLATION: LazyLock<Token> = LazyLock::new(|| Token::new("translation"));
    pub static ROTATION: LazyLock<Token> = LazyLock::new(|| Token::new("rotation"));
    pub static ROTATION_ORDER: LazyLock<Token> = LazyLock::new(|| Token::new("rotationOrder"));
    pub static SCALE: LazyLock<Token> = LazyLock::new(|| Token::new("scale"));
    pub static PIVOT: LazyLock<Token> = LazyLock::new(|| Token::new("pivot"));
}

/// Handle to Vec3d data source (translation).
pub type HdVec3dDataSourceHandle = Arc<dyn HdTypedSampledDataSource<Vec3d> + Send + Sync>;

/// Handle to Vec3f data source (rotation, scale, pivot).
pub type HdVec3fDataSourceHandle = Arc<dyn HdTypedSampledDataSource<Vec3f> + Send + Sync>;

/// Handle to Token data source (rotationOrder).
pub type HdTokenDataSourceHandle = Arc<dyn HdTypedSampledDataSource<Token> + Send + Sync>;

// ============================================================================
// GeomXformVectorsSchema
// ============================================================================

/// Schema for xform vectors decomposition in Hydra.
///
/// Exposes UsdGeomXformCommonAPI::GetXformVectorsByAccumulation() result.
/// Contains translation, rotation, rotationOrder, scale, pivot.
#[derive(Debug, Clone)]
pub struct GeomXformVectorsSchema {
    schema: HdSchema,
}

impl GeomXformVectorsSchema {
    /// Create schema from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Create schema from optional container (for undefined case).
    pub fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self {
            schema: container.map(HdSchema::new).unwrap_or_else(HdSchema::empty),
        }
    }

    /// Check if this schema is defined.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get translation data source.
    pub fn get_translation(&self) -> Option<HdVec3dDataSourceHandle> {
        self.schema
            .get_typed::<HdRetainedTypedSampledDataSource<Vec3d>>(&tokens::TRANSLATION)
            .map(|arc| arc as HdVec3dDataSourceHandle)
    }

    /// Get rotation data source.
    pub fn get_rotation(&self) -> Option<HdVec3fDataSourceHandle> {
        self.schema
            .get_typed::<HdRetainedTypedSampledDataSource<Vec3f>>(&tokens::ROTATION)
            .map(|arc| arc as HdVec3fDataSourceHandle)
    }

    /// Get rotation order data source.
    pub fn get_rotation_order(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema
            .get_typed::<HdRetainedTypedSampledDataSource<Token>>(&tokens::ROTATION_ORDER)
            .map(|arc| arc as HdTokenDataSourceHandle)
    }

    /// Get scale data source.
    pub fn get_scale(&self) -> Option<HdVec3fDataSourceHandle> {
        self.schema
            .get_typed::<HdRetainedTypedSampledDataSource<Vec3f>>(&tokens::SCALE)
            .map(|arc| arc as HdVec3fDataSourceHandle)
    }

    /// Get pivot data source.
    pub fn get_pivot(&self) -> Option<HdVec3fDataSourceHandle> {
        self.schema
            .get_typed::<HdRetainedTypedSampledDataSource<Vec3f>>(&tokens::PIVOT)
            .map(|arc| arc as HdVec3fDataSourceHandle)
    }

    /// Get schema token.
    pub fn get_schema_token() -> Token {
        tokens::GEOM_XFORM_VECTORS.clone()
    }

    /// Get default locator.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::GEOM_XFORM_VECTORS.clone())
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&tokens::GEOM_XFORM_VECTORS) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self::from_container(None)
    }

    /// Build retained container with optional fields.
    pub fn build_retained(
        translation: Option<HdVec3dDataSourceHandle>,
        rotation: Option<HdVec3fDataSourceHandle>,
        rotation_order: Option<HdTokenDataSourceHandle>,
        scale: Option<HdVec3fDataSourceHandle>,
        pivot: Option<HdVec3fDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();
        if let Some(t) = translation {
            entries.push((tokens::TRANSLATION.clone(), t as HdDataSourceBaseHandle));
        }
        if let Some(r) = rotation {
            entries.push((tokens::ROTATION.clone(), r as HdDataSourceBaseHandle));
        }
        if let Some(ro) = rotation_order {
            entries.push((tokens::ROTATION_ORDER.clone(), ro as HdDataSourceBaseHandle));
        }
        if let Some(s) = scale {
            entries.push((tokens::SCALE.clone(), s as HdDataSourceBaseHandle));
        }
        if let Some(p) = pivot {
            entries.push((tokens::PIVOT.clone(), p as HdDataSourceBaseHandle));
        }
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

// ============================================================================
// GeomXformVectorsSchemaBuilder
// ============================================================================

/// Builder for GeomXformVectorsSchema data sources.
#[derive(Debug, Default)]
pub struct GeomXformVectorsSchemaBuilder {
    translation: Option<HdVec3dDataSourceHandle>,
    rotation: Option<HdVec3fDataSourceHandle>,
    rotation_order: Option<HdTokenDataSourceHandle>,
    scale: Option<HdVec3fDataSourceHandle>,
    pivot: Option<HdVec3fDataSourceHandle>,
}

impl GeomXformVectorsSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set translation.
    pub fn set_translation(mut self, translation: HdVec3dDataSourceHandle) -> Self {
        self.translation = Some(translation);
        self
    }

    /// Set rotation.
    pub fn set_rotation(mut self, rotation: HdVec3fDataSourceHandle) -> Self {
        self.rotation = Some(rotation);
        self
    }

    /// Set rotation order.
    pub fn set_rotation_order(mut self, rotation_order: HdTokenDataSourceHandle) -> Self {
        self.rotation_order = Some(rotation_order);
        self
    }

    /// Set scale.
    pub fn set_scale(mut self, scale: HdVec3fDataSourceHandle) -> Self {
        self.scale = Some(scale);
        self
    }

    /// Set pivot.
    pub fn set_pivot(mut self, pivot: HdVec3fDataSourceHandle) -> Self {
        self.pivot = Some(pivot);
        self
    }

    /// Build the container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        GeomXformVectorsSchema::build_retained(
            self.translation,
            self.rotation,
            self.rotation_order,
            self.scale,
            self.pivot,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_gf::vec3::Vec3d;
    use usd_gf::vec3::Vec3f;

    #[test]
    fn test_schema_token() {
        assert_eq!(
            GeomXformVectorsSchema::get_schema_token().as_str(),
            "geomXformVectors"
        );
    }

    #[test]
    fn test_default_locator() {
        let locator = GeomXformVectorsSchema::get_default_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_build_retained() {
        let trans = HdRetainedTypedSampledDataSource::new(Vec3d::new(1.0, 2.0, 3.0));
        let scale = HdRetainedTypedSampledDataSource::new(Vec3f::new(1.0, 1.0, 1.0));
        let container =
            GeomXformVectorsSchema::build_retained(Some(trans), None, None, Some(scale), None);
        let schema = GeomXformVectorsSchema::new(container);
        assert!(schema.is_defined());
    }

    #[test]
    fn test_builder() {
        let trans = HdRetainedTypedSampledDataSource::new(Vec3d::zero());
        let scale = HdRetainedTypedSampledDataSource::new(Vec3f::new(1.0, 1.0, 1.0));
        let _container = GeomXformVectorsSchemaBuilder::new()
            .set_translation(trans)
            .set_scale(scale)
            .build();
    }

    #[test]
    fn test_tokens() {
        assert_eq!(tokens::TRANSLATION.as_str(), "translation");
        assert_eq!(tokens::ROTATION.as_str(), "rotation");
        assert_eq!(tokens::ROTATION_ORDER.as_str(), "rotationOrder");
        assert_eq!(tokens::SCALE.as_str(), "scale");
        assert_eq!(tokens::PIVOT.as_str(), "pivot");
    }
}
