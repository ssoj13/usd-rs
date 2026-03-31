//! ResolvedSkeletonSchema - Hydra schema for resolved skeleton data.
//!
//! Port of pxr/usdImaging/usdSkelImaging/resolvedSkeletonSchema.h
//!
//! Resolved data for a skeleton and targeted skelAnim.
//! Populated by the skeleton resolving scene index.

use std::sync::Arc;
use usd_gf::matrix4::{Matrix4d, Matrix4f};
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdRetainedContainerDataSource, HdTypedSampledDataSource, SampledToTypedAdapter,
    cast_to_container,
};
use usd_hd::schema::HdSchema;
use usd_hd::schema::HdMatrixDataSourceHandle;
use usd_tf::Token;

// Token constants (USD_SKEL_IMAGING_RESOLVED_SKELETON_SCHEMA_TOKENS)
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static RESOLVED_SKELETON: LazyLock<Token> =
        LazyLock::new(|| Token::new("resolvedSkeleton"));
    pub static SKEL_LOCAL_TO_COMMON_SPACE: LazyLock<Token> =
        LazyLock::new(|| Token::new("skelLocalToCommonSpace"));
    pub static SKINNING_TRANSFORMS: LazyLock<Token> =
        LazyLock::new(|| Token::new("skinningTransforms"));
    pub static BLEND_SHAPES: LazyLock<Token> = LazyLock::new(|| Token::new("blendShapes"));
    pub static BLEND_SHAPE_WEIGHTS: LazyLock<Token> =
        LazyLock::new(|| Token::new("blendShapeWeights"));
    pub static BLEND_SHAPE_RANGES: LazyLock<Token> =
        LazyLock::new(|| Token::new("blendShapeRanges"));
}

/// Handle to Matrix4f array data source.
pub type HdMatrix4fArrayDataSourceHandle =
    Arc<dyn HdTypedSampledDataSource<Vec<Matrix4f>> + Send + Sync>;

/// Handle to Token array data source.
pub type HdTokenArrayDataSourceHandle = Arc<dyn HdTypedSampledDataSource<Vec<Token>> + Send + Sync>;

/// Handle to f32 array data source.
pub type HdFloatArrayDataSourceHandle = Arc<dyn HdTypedSampledDataSource<Vec<f32>> + Send + Sync>;

/// Handle to Vec2i array data source (blend shape ranges).
pub type HdVec2iArrayDataSourceHandle =
    Arc<dyn HdTypedSampledDataSource<Vec<usd_gf::vec2::Vec2i>> + Send + Sync>;

// ============================================================================
// ResolvedSkeletonSchema
// ============================================================================

/// Schema for resolved skeleton data in Hydra.
///
/// Resolved data for skeleton and skelAnim. Contains skinning transforms,
/// blend shapes, blend shape weights and ranges.
#[derive(Debug, Clone)]
pub struct ResolvedSkeletonSchema {
    schema: HdSchema,
}

impl ResolvedSkeletonSchema {
    fn get_typed_child<T>(
        &self,
        name: &Token,
    ) -> Option<Arc<dyn HdTypedSampledDataSource<T> + Send + Sync>>
    where
        T: usd_hd::data_source::HdValueExtract + std::fmt::Debug,
    {
        let child = self.schema.get_container()?.get(name)?;
        Some(SampledToTypedAdapter::<T>::new(child))
    }

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

    /// Check if this schema is defined.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get skel local to common space transform.
    pub fn get_skel_local_to_common_space(&self) -> Option<HdMatrixDataSourceHandle> {
        self.get_typed_child::<Matrix4d>(&tokens::SKEL_LOCAL_TO_COMMON_SPACE)
            .map(|arc| arc as HdMatrixDataSourceHandle)
    }

    /// Get skinning transforms.
    pub fn get_skinning_transforms(&self) -> Option<HdMatrix4fArrayDataSourceHandle> {
        self.get_typed_child::<Vec<Matrix4f>>(&tokens::SKINNING_TRANSFORMS)
            .map(|arc| arc as HdMatrix4fArrayDataSourceHandle)
    }

    /// Get blend shapes (token array).
    pub fn get_blend_shapes(&self) -> Option<HdTokenArrayDataSourceHandle> {
        self.get_typed_child::<Vec<Token>>(&tokens::BLEND_SHAPES)
            .map(|arc| arc as HdTokenArrayDataSourceHandle)
    }

    /// Get blend shape weights.
    pub fn get_blend_shape_weights(&self) -> Option<HdFloatArrayDataSourceHandle> {
        self.get_typed_child::<Vec<f32>>(&tokens::BLEND_SHAPE_WEIGHTS)
            .map(|arc| arc as HdFloatArrayDataSourceHandle)
    }

    /// Get blend shape ranges.
    pub fn get_blend_shape_ranges(&self) -> Option<HdVec2iArrayDataSourceHandle> {
        self.get_typed_child::<Vec<usd_gf::vec2::Vec2i>>(&tokens::BLEND_SHAPE_RANGES)
            .map(|arc| arc as HdVec2iArrayDataSourceHandle)
    }

    /// Get schema token.
    pub fn get_schema_token() -> Token {
        tokens::RESOLVED_SKELETON.clone()
    }

    /// Get default locator.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::RESOLVED_SKELETON.clone())
    }

    /// Get skel local to common space locator.
    pub fn get_skel_local_to_common_space_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::RESOLVED_SKELETON.clone(),
            tokens::SKEL_LOCAL_TO_COMMON_SPACE.clone(),
        )
    }

    /// Get skinning transforms locator.
    pub fn get_skinning_transforms_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::RESOLVED_SKELETON.clone(),
            tokens::SKINNING_TRANSFORMS.clone(),
        )
    }

    /// Get blend shapes locator.
    pub fn get_blend_shapes_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::RESOLVED_SKELETON.clone(),
            tokens::BLEND_SHAPES.clone(),
        )
    }

    /// Get blend shape weights locator.
    pub fn get_blend_shape_weights_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::RESOLVED_SKELETON.clone(),
            tokens::BLEND_SHAPE_WEIGHTS.clone(),
        )
    }

    /// Get blend shape ranges locator.
    pub fn get_blend_shape_ranges_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::RESOLVED_SKELETON.clone(),
            tokens::BLEND_SHAPE_RANGES.clone(),
        )
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&tokens::RESOLVED_SKELETON) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self::from_container(None)
    }

    /// Build retained container.
    pub fn build_retained(
        skel_local_to_common_space: Option<HdMatrixDataSourceHandle>,
        skinning_transforms: Option<HdMatrix4fArrayDataSourceHandle>,
        blend_shapes: Option<HdTokenArrayDataSourceHandle>,
        blend_shape_weights: Option<HdFloatArrayDataSourceHandle>,
        blend_shape_ranges: Option<HdVec2iArrayDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();
        if let Some(s) = skel_local_to_common_space {
            entries.push((
                tokens::SKEL_LOCAL_TO_COMMON_SPACE.clone(),
                s as HdDataSourceBaseHandle,
            ));
        }
        if let Some(s) = skinning_transforms {
            entries.push((
                tokens::SKINNING_TRANSFORMS.clone(),
                s as HdDataSourceBaseHandle,
            ));
        }
        if let Some(b) = blend_shapes {
            entries.push((tokens::BLEND_SHAPES.clone(), b as HdDataSourceBaseHandle));
        }
        if let Some(b) = blend_shape_weights {
            entries.push((
                tokens::BLEND_SHAPE_WEIGHTS.clone(),
                b as HdDataSourceBaseHandle,
            ));
        }
        if let Some(b) = blend_shape_ranges {
            entries.push((
                tokens::BLEND_SHAPE_RANGES.clone(),
                b as HdDataSourceBaseHandle,
            ));
        }
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

// ============================================================================
// ResolvedSkeletonSchemaBuilder
// ============================================================================

/// Builder for ResolvedSkeletonSchema data sources.
#[derive(Debug, Default)]
pub struct ResolvedSkeletonSchemaBuilder {
    skel_local_to_common_space: Option<HdMatrixDataSourceHandle>,
    skinning_transforms: Option<HdMatrix4fArrayDataSourceHandle>,
    blend_shapes: Option<HdTokenArrayDataSourceHandle>,
    blend_shape_weights: Option<HdFloatArrayDataSourceHandle>,
    blend_shape_ranges: Option<HdVec2iArrayDataSourceHandle>,
}

impl ResolvedSkeletonSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the skeleton local-to-common-space matrix data source.
    pub fn set_skel_local_to_common_space(mut self, v: HdMatrixDataSourceHandle) -> Self {
        self.skel_local_to_common_space = Some(v);
        self
    }

    /// Sets the skinning transforms array data source.
    pub fn set_skinning_transforms(mut self, v: HdMatrix4fArrayDataSourceHandle) -> Self {
        self.skinning_transforms = Some(v);
        self
    }

    /// Sets the blend shapes token array data source.
    pub fn set_blend_shapes(mut self, v: HdTokenArrayDataSourceHandle) -> Self {
        self.blend_shapes = Some(v);
        self
    }

    /// Sets the blend shape weights array data source.
    pub fn set_blend_shape_weights(mut self, v: HdFloatArrayDataSourceHandle) -> Self {
        self.blend_shape_weights = Some(v);
        self
    }

    /// Sets the blend shape ranges (start, count) array data source.
    pub fn set_blend_shape_ranges(mut self, v: HdVec2iArrayDataSourceHandle) -> Self {
        self.blend_shape_ranges = Some(v);
        self
    }

    /// Builds the container data source from the configured fields.
    pub fn build(self) -> HdContainerDataSourceHandle {
        ResolvedSkeletonSchema::build_retained(
            self.skel_local_to_common_space,
            self.skinning_transforms,
            self.blend_shapes,
            self.blend_shape_weights,
            self.blend_shape_ranges,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        assert_eq!(
            ResolvedSkeletonSchema::get_schema_token().as_str(),
            "resolvedSkeleton"
        );
    }

    #[test]
    fn test_locators() {
        let _ = ResolvedSkeletonSchema::get_skinning_transforms_locator();
        let _ = ResolvedSkeletonSchema::get_blend_shapes_locator();
    }

    #[test]
    fn test_build_empty() {
        let container = ResolvedSkeletonSchema::build_retained(None, None, None, None, None);
        let schema = ResolvedSkeletonSchema::new(container);
        assert!(schema.is_defined());
    }
}
