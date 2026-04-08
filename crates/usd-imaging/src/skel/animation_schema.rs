//! AnimationSchema - Hydra schema for skeleton animation data.
//!
//! Port of pxr/usdImaging/usdSkelImaging/animationSchema.h
//!
//! Provides data source schema for skeleton animation data in Hydra.

use std::sync::Arc;
use usd_gf::{Quatf, Vec3f, Vec3h};
use usd_hd::data_source::{HdTypedSampledDataSource, SampledToTypedAdapter};
use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator, cast_to_container,
};
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static SKEL_ANIMATION: LazyLock<Token> = LazyLock::new(|| Token::new("skelAnimation"));
    pub static JOINTS: LazyLock<Token> = LazyLock::new(|| Token::new("joints"));
    pub static TRANSLATIONS: LazyLock<Token> = LazyLock::new(|| Token::new("translations"));
    pub static ROTATIONS: LazyLock<Token> = LazyLock::new(|| Token::new("rotations"));
    pub static SCALES: LazyLock<Token> = LazyLock::new(|| Token::new("scales"));
    pub static BLEND_SHAPES: LazyLock<Token> = LazyLock::new(|| Token::new("blendShapes"));
    pub static BLEND_SHAPE_WEIGHTS: LazyLock<Token> =
        LazyLock::new(|| Token::new("blendShapeWeights"));
}

// ============================================================================
// AnimationSchema
// ============================================================================

/// Schema for skeleton animation data in Hydra.
///
/// Contains joint transforms (translations, rotations, scales)
/// and blend shape weights.
#[derive(Debug, Clone)]
pub struct AnimationSchema {
    container: Option<HdContainerDataSourceHandle>,
}

impl AnimationSchema {
    /// Create schema from container.
    pub fn new(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self { container }
    }

    /// Check if this schema is defined.
    pub fn is_defined(&self) -> bool {
        self.container.is_some()
    }

    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.container.as_ref()
    }

    /// Get the schema token.
    pub fn get_schema_token() -> Token {
        tokens::SKEL_ANIMATION.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::SKEL_ANIMATION.clone())
    }

    /// Get the joints locator.
    pub fn get_joints_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::SKEL_ANIMATION.clone(), tokens::JOINTS.clone())
    }

    /// Get the translations locator.
    pub fn get_translations_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKEL_ANIMATION.clone(),
            tokens::TRANSLATIONS.clone(),
        )
    }

    /// Get the rotations locator.
    pub fn get_rotations_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKEL_ANIMATION.clone(),
            tokens::ROTATIONS.clone(),
        )
    }

    /// Get the scales locator.
    pub fn get_scales_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::SKEL_ANIMATION.clone(), tokens::SCALES.clone())
    }

    /// Get the blend shapes locator.
    pub fn get_blend_shapes_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKEL_ANIMATION.clone(),
            tokens::BLEND_SHAPES.clone(),
        )
    }

    /// Get the blend shape weights locator.
    pub fn get_blend_shape_weights_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKEL_ANIMATION.clone(),
            tokens::BLEND_SHAPE_WEIGHTS.clone(),
        )
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        let ds = parent.get(&tokens::SKEL_ANIMATION)?;
        let container = cast_to_container(&ds)?;
        Some(Self {
            container: Some(container),
        })
    }

    /// Get the joints data source.
    pub fn get_joints(&self) -> Option<HdDataSourceBaseHandle> {
        self.container.as_ref()?.get(&tokens::JOINTS)
    }

    /// Get the translations data source.
    pub fn get_translations(&self) -> Option<HdDataSourceBaseHandle> {
        self.container.as_ref()?.get(&tokens::TRANSLATIONS)
    }

    /// Get the rotations data source.
    pub fn get_rotations(&self) -> Option<HdDataSourceBaseHandle> {
        self.container.as_ref()?.get(&tokens::ROTATIONS)
    }

    /// Get the scales data source.
    pub fn get_scales(&self) -> Option<HdDataSourceBaseHandle> {
        self.container.as_ref()?.get(&tokens::SCALES)
    }

    /// Get the blend shapes data source.
    pub fn get_blend_shapes(&self) -> Option<HdDataSourceBaseHandle> {
        self.container.as_ref()?.get(&tokens::BLEND_SHAPES)
    }

    /// Get the blend shape weights data source.
    pub fn get_blend_shape_weights(&self) -> Option<HdDataSourceBaseHandle> {
        self.container.as_ref()?.get(&tokens::BLEND_SHAPE_WEIGHTS)
    }

    pub fn get_joints_data_source(
        &self,
    ) -> Option<Arc<dyn HdTypedSampledDataSource<Vec<Token>> + Send + Sync>> {
        let child = self.container.as_ref()?.get(&tokens::JOINTS)?;
        Some(SampledToTypedAdapter::<Vec<Token>>::new(child))
    }

    pub fn get_translations_data_source(
        &self,
    ) -> Option<Arc<dyn HdTypedSampledDataSource<Vec<Vec3f>> + Send + Sync>> {
        let child = self.container.as_ref()?.get(&tokens::TRANSLATIONS)?;
        Some(SampledToTypedAdapter::<Vec<Vec3f>>::new(child))
    }

    pub fn get_rotations_data_source(
        &self,
    ) -> Option<Arc<dyn HdTypedSampledDataSource<Vec<Quatf>> + Send + Sync>> {
        let child = self.container.as_ref()?.get(&tokens::ROTATIONS)?;
        Some(SampledToTypedAdapter::<Vec<Quatf>>::new(child))
    }

    pub fn get_scales_data_source(
        &self,
    ) -> Option<Arc<dyn HdTypedSampledDataSource<Vec<Vec3h>> + Send + Sync>> {
        let child = self.container.as_ref()?.get(&tokens::SCALES)?;
        Some(SampledToTypedAdapter::<Vec<Vec3h>>::new(child))
    }

    pub fn get_blend_shapes_data_source(
        &self,
    ) -> Option<Arc<dyn HdTypedSampledDataSource<Vec<Token>> + Send + Sync>> {
        let child = self.container.as_ref()?.get(&tokens::BLEND_SHAPES)?;
        Some(SampledToTypedAdapter::<Vec<Token>>::new(child))
    }

    pub fn get_blend_shape_weights_data_source(
        &self,
    ) -> Option<Arc<dyn HdTypedSampledDataSource<Vec<f32>> + Send + Sync>> {
        let child = self.container.as_ref()?.get(&tokens::BLEND_SHAPE_WEIGHTS)?;
        Some(SampledToTypedAdapter::<Vec<f32>>::new(child))
    }
}

// ============================================================================
// AnimationSchemaBuilder
// ============================================================================

/// Builder for AnimationSchema data sources.
#[derive(Debug, Default)]
pub struct AnimationSchemaBuilder {
    joints: Option<HdDataSourceBaseHandle>,
    translations: Option<HdDataSourceBaseHandle>,
    rotations: Option<HdDataSourceBaseHandle>,
    scales: Option<HdDataSourceBaseHandle>,
    blend_shapes: Option<HdDataSourceBaseHandle>,
    blend_shape_weights: Option<HdDataSourceBaseHandle>,
}

impl AnimationSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the joints data source.
    pub fn set_joints(mut self, v: HdDataSourceBaseHandle) -> Self {
        self.joints = Some(v);
        self
    }

    /// Set the translations data source.
    pub fn set_translations(mut self, v: HdDataSourceBaseHandle) -> Self {
        self.translations = Some(v);
        self
    }

    /// Set the rotations data source.
    pub fn set_rotations(mut self, v: HdDataSourceBaseHandle) -> Self {
        self.rotations = Some(v);
        self
    }

    /// Set the scales data source.
    pub fn set_scales(mut self, v: HdDataSourceBaseHandle) -> Self {
        self.scales = Some(v);
        self
    }

    /// Set the blend shapes data source.
    pub fn set_blend_shapes(mut self, v: HdDataSourceBaseHandle) -> Self {
        self.blend_shapes = Some(v);
        self
    }

    /// Set the blend shape weights data source.
    pub fn set_blend_shape_weights(mut self, v: HdDataSourceBaseHandle) -> Self {
        self.blend_shape_weights = Some(v);
        self
    }

    /// Build the container data source from set fields.
    pub fn build(self) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::with_capacity(6);
        if let Some(v) = self.joints {
            entries.push((tokens::JOINTS.clone(), v));
        }
        if let Some(v) = self.translations {
            entries.push((tokens::TRANSLATIONS.clone(), v));
        }
        if let Some(v) = self.rotations {
            entries.push((tokens::ROTATIONS.clone(), v));
        }
        if let Some(v) = self.scales {
            entries.push((tokens::SCALES.clone(), v));
        }
        if let Some(v) = self.blend_shapes {
            entries.push((tokens::BLEND_SHAPES.clone(), v));
        }
        if let Some(v) = self.blend_shape_weights {
            entries.push((tokens::BLEND_SHAPE_WEIGHTS.clone(), v));
        }
        usd_hd::HdRetainedContainerDataSource::from_entries(&entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        assert_eq!(
            AnimationSchema::get_schema_token().as_str(),
            "skelAnimation"
        );
    }

    #[test]
    fn test_translations_locator() {
        let locator = AnimationSchema::get_translations_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_blend_shape_weights_locator() {
        let locator = AnimationSchema::get_blend_shape_weights_locator();
        assert!(locator.first_element().is_some());
    }
}
