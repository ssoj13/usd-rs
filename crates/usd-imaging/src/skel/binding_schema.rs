//! BindingSchema - Hydra schema for skeleton binding data.
//!
//! Port of pxr/usdImaging/usdSkelImaging/bindingSchema.h
//!
//! Provides data source schema for skeleton binding data in Hydra.

use super::data_source_utils::{
    get_typed_value_from_container_bool, get_typed_value_from_container_path,
    get_typed_value_from_container_vec_path, get_typed_value_from_container_vec_token,
};
use usd_hd::schema::HdSchema;
use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator, cast_to_container,
};
use usd_sdf::Path;
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static SKEL_BINDING: LazyLock<Token> = LazyLock::new(|| Token::new("skelBinding"));
    pub static SKELETON: LazyLock<Token> = LazyLock::new(|| Token::new("skeleton"));
    pub static ANIMATION_SOURCE: LazyLock<Token> = LazyLock::new(|| Token::new("animationSource"));
    pub static JOINT_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("jointIndices"));
    pub static JOINT_WEIGHTS: LazyLock<Token> = LazyLock::new(|| Token::new("jointWeights"));
    /// Primvar name for joint indices (skel:jointIndices)
    pub static JOINT_INDICES_PRIMVAR: LazyLock<Token> =
        LazyLock::new(|| Token::new("skel:jointIndices"));
    /// Primvar name for joint weights (skel:jointWeights)
    pub static JOINT_WEIGHTS_PRIMVAR: LazyLock<Token> =
        LazyLock::new(|| Token::new("skel:jointWeights"));
    /// Primvar name for skinning method (skel:skinningMethod)
    pub static SKINNING_METHOD_PRIMVAR: LazyLock<Token> =
        LazyLock::new(|| Token::new("skel:skinningMethod"));
    /// Primvar name for geom bind transform (skel:geomBindTransform)
    pub static GEOM_BIND_TRANSFORM_PRIMVAR: LazyLock<Token> =
        LazyLock::new(|| Token::new("skel:geomBindTransform"));
    pub static GEOM_BIND_TRANSFORM: LazyLock<Token> =
        LazyLock::new(|| Token::new("geomBindTransform"));
    pub static JOINTS: LazyLock<Token> = LazyLock::new(|| Token::new("joints"));
    pub static BLEND_SHAPES: LazyLock<Token> = LazyLock::new(|| Token::new("blendShapes"));
    pub static BLEND_SHAPE_TARGETS: LazyLock<Token> =
        LazyLock::new(|| Token::new("blendShapeTargets"));
    pub static HAS_SKEL_ROOT: LazyLock<Token> = LazyLock::new(|| Token::new("hasSkelRoot"));
}

// ============================================================================
// BindingSchema
// ============================================================================

/// Schema for skeleton binding data in Hydra.
///
/// Contains skeleton reference, animation source, joint influences,
/// and geometry bind transform.
#[derive(Debug, Clone)]
pub struct BindingSchema {
    schema: HdSchema,
}

impl BindingSchema {
    /// Create schema from container.
    pub fn new(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self {
            schema: container.map(HdSchema::new).unwrap_or_else(HdSchema::empty),
        }
    }

    /// Get schema from parent container (looks for "skelBinding" child).
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&tokens::SKEL_BINDING) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(Some(container));
            }
        }
        Self::new(None)
    }

    /// Check if this schema is defined.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get skeleton path.
    pub fn get_skeleton(&self) -> Option<Path> {
        self.schema
            .get_container()
            .and_then(|c| get_typed_value_from_container_path(c, &tokens::SKELETON))
    }

    pub fn get_animation_source(&self) -> Option<Path> {
        self.schema
            .get_container()
            .and_then(|c| get_typed_value_from_container_path(c, &tokens::ANIMATION_SOURCE))
    }

    /// Get has SkelRoot (prim is under SkelRoot).
    pub fn get_has_skel_root(&self) -> bool {
        self.schema
            .get_container()
            .and_then(|c| get_typed_value_from_container_bool(c, &tokens::HAS_SKEL_ROOT))
            .unwrap_or(false)
    }

    /// Get blend shape target paths.
    pub fn get_blend_shape_targets(&self) -> Vec<Path> {
        self.schema
            .get_container()
            .and_then(|c| get_typed_value_from_container_vec_path(c, &tokens::BLEND_SHAPE_TARGETS))
            .unwrap_or_default()
    }

    /// Get the schema token.
    pub fn get_schema_token() -> Token {
        tokens::SKEL_BINDING.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::SKEL_BINDING.clone())
    }

    /// Get the skeleton locator.
    pub fn get_skeleton_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::SKEL_BINDING.clone(), tokens::SKELETON.clone())
    }

    /// Get the animation source locator.
    pub fn get_animation_source_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKEL_BINDING.clone(),
            tokens::ANIMATION_SOURCE.clone(),
        )
    }

    /// Get the joint indices locator.
    pub fn get_joint_indices_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKEL_BINDING.clone(),
            tokens::JOINT_INDICES.clone(),
        )
    }

    /// Get the joint weights locator.
    pub fn get_joint_weights_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKEL_BINDING.clone(),
            tokens::JOINT_WEIGHTS.clone(),
        )
    }

    /// Get the geom bind transform locator.
    pub fn get_geom_bind_transform_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKEL_BINDING.clone(),
            tokens::GEOM_BIND_TRANSFORM.clone(),
        )
    }

    /// Get the joints locator.
    pub fn get_joints_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::SKEL_BINDING.clone(), tokens::JOINTS.clone())
    }

    /// Get the blend shapes locator.
    pub fn get_blend_shapes_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKEL_BINDING.clone(),
            tokens::BLEND_SHAPES.clone(),
        )
    }

    /// Get joint names (binding order) from schema.
    pub fn get_joints(&self) -> Vec<Token> {
        self.schema
            .get_container()
            .and_then(|c| get_typed_value_from_container_vec_token(c, &tokens::JOINTS))
            .unwrap_or_default()
    }

    /// Get blend shape names from schema.
    pub fn get_blend_shapes(&self) -> Vec<Token> {
        self.schema
            .get_container()
            .and_then(|c| get_typed_value_from_container_vec_token(c, &tokens::BLEND_SHAPES))
            .unwrap_or_default()
    }

    /// Token for joint indices primvar (skel:jointIndices).
    pub fn get_joint_indices_primvar_token() -> Token {
        tokens::JOINT_INDICES_PRIMVAR.clone()
    }

    /// Token for joint weights primvar (skel:jointWeights).
    pub fn get_joint_weights_primvar_token() -> Token {
        tokens::JOINT_WEIGHTS_PRIMVAR.clone()
    }

    /// Token for skinning method primvar (skel:skinningMethod).
    pub fn get_skinning_method_primvar_token() -> Token {
        tokens::SKINNING_METHOD_PRIMVAR.clone()
    }

    /// Token for geom bind transform primvar (skel:geomBindTransform).
    pub fn get_geom_bind_transform_primvar_token() -> Token {
        tokens::GEOM_BIND_TRANSFORM_PRIMVAR.clone()
    }
}

// ============================================================================
// BindingSchemaBuilder
// ============================================================================

/// Builder for BindingSchema data sources.
#[derive(Debug, Default)]
pub struct BindingSchemaBuilder {
    skeleton: Option<HdDataSourceBaseHandle>,
    animation_source: Option<HdDataSourceBaseHandle>,
    joint_indices: Option<HdDataSourceBaseHandle>,
    joint_weights: Option<HdDataSourceBaseHandle>,
}

impl BindingSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the skeleton path data source.
    pub fn set_skeleton(mut self, v: HdDataSourceBaseHandle) -> Self {
        self.skeleton = Some(v);
        self
    }

    /// Set the animation source path data source.
    pub fn set_animation_source(mut self, v: HdDataSourceBaseHandle) -> Self {
        self.animation_source = Some(v);
        self
    }

    /// Set the joint indices data source.
    pub fn set_joint_indices(mut self, v: HdDataSourceBaseHandle) -> Self {
        self.joint_indices = Some(v);
        self
    }

    /// Set the joint weights data source.
    pub fn set_joint_weights(mut self, v: HdDataSourceBaseHandle) -> Self {
        self.joint_weights = Some(v);
        self
    }

    /// Build the container data source from set fields.
    pub fn build(self) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::with_capacity(4);
        if let Some(v) = self.skeleton {
            entries.push((tokens::SKELETON.clone(), v));
        }
        if let Some(v) = self.animation_source {
            entries.push((tokens::ANIMATION_SOURCE.clone(), v));
        }
        if let Some(v) = self.joint_indices {
            entries.push((tokens::JOINT_INDICES.clone(), v));
        }
        if let Some(v) = self.joint_weights {
            entries.push((tokens::JOINT_WEIGHTS.clone(), v));
        }
        usd_hd::HdRetainedContainerDataSource::from_entries(&entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        assert_eq!(BindingSchema::get_schema_token().as_str(), "skelBinding");
    }

    #[test]
    fn test_skeleton_locator() {
        let locator = BindingSchema::get_skeleton_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_joint_indices_locator() {
        let locator = BindingSchema::get_joint_indices_locator();
        assert!(locator.first_element().is_some());
    }
}
