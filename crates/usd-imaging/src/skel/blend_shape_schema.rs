//! BlendShapeSchema - Hydra schema for blend shape data.
//!
//! Port of pxr/usdImaging/usdSkelImaging/blendShapeSchema.h
//!
//! Corresponds to UsdSkelBlendShape. Contains offsets, normalOffsets,
//! pointIndices, and inbetweenShapes.

use super::data_source_utils::{
    get_typed_value_from_container_vec_i32, get_typed_value_from_container_vec_vec3f,
};
use usd_gf::vec3::Vec3f;
use usd_hd::data_source::cast_to_container;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocator};
use usd_tf::Token;

// Token constants (USD_SKEL_IMAGING_BLEND_SHAPE_SCHEMA_TOKENS)
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static SKEL_BLEND_SHAPE: LazyLock<Token> = LazyLock::new(|| Token::new("skelBlendShape"));
    pub static OFFSETS: LazyLock<Token> = LazyLock::new(|| Token::new("offsets"));
    pub static NORMAL_OFFSETS: LazyLock<Token> = LazyLock::new(|| Token::new("normalOffsets"));
    pub static POINT_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("pointIndices"));
    pub static INBETWEEN_SHAPES: LazyLock<Token> = LazyLock::new(|| Token::new("inbetweenShapes"));
}

// ============================================================================
// BlendShapeSchema
// ============================================================================

/// Schema for blend shape data in Hydra.
///
/// Corresponds to UsdSkelBlendShape. Contains offsets, normal offsets,
/// point indices, and inbetween shapes.
#[derive(Debug, Clone)]
pub struct BlendShapeSchema {
    #[allow(dead_code)]
    container: Option<HdContainerDataSourceHandle>,
}

impl BlendShapeSchema {
    /// Create schema from container.
    pub fn new(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self { container }
    }

    /// Check if this schema is defined.
    pub fn is_defined(&self) -> bool {
        self.container.is_some()
    }

    /// Get the schema token.
    pub fn get_schema_token() -> Token {
        tokens::SKEL_BLEND_SHAPE.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::SKEL_BLEND_SHAPE.clone())
    }

    /// Get the offsets locator.
    pub fn get_offsets_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKEL_BLEND_SHAPE.clone(),
            tokens::OFFSETS.clone(),
        )
    }

    /// Get the normal offsets locator.
    pub fn get_normal_offsets_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKEL_BLEND_SHAPE.clone(),
            tokens::NORMAL_OFFSETS.clone(),
        )
    }

    /// Get the point indices locator.
    pub fn get_point_indices_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKEL_BLEND_SHAPE.clone(),
            tokens::POINT_INDICES.clone(),
        )
    }

    /// Get the inbetween shapes locator.
    pub fn get_inbetween_shapes_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::SKEL_BLEND_SHAPE.clone(),
            tokens::INBETWEEN_SHAPES.clone(),
        )
    }

    /// Get schema from parent container (looks for "skelBlendShape" child).
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&tokens::SKEL_BLEND_SHAPE) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(Some(container));
            }
        }
        Self::new(None)
    }

    /// Get point offsets (Vec3f array) from schema.
    pub fn get_offsets(&self) -> Vec<Vec3f> {
        self.container
            .as_ref()
            .and_then(|c| get_typed_value_from_container_vec_vec3f(c, &tokens::OFFSETS))
            .unwrap_or_default()
    }

    /// Get point indices (int array) from schema. Empty = dense (implied 0..n).
    pub fn get_point_indices(&self) -> Vec<i32> {
        self.container
            .as_ref()
            .and_then(|c| get_typed_value_from_container_vec_i32(c, &tokens::POINT_INDICES))
            .unwrap_or_default()
    }

    /// Get inbetween shapes container. Each child name is an inbetween name,
    /// each value is a container with weight, offsets, normalOffsets.
    pub fn get_inbetween_shapes_container(&self) -> Option<HdContainerDataSourceHandle> {
        self.container
            .as_ref()
            .and_then(|c| c.get(&tokens::INBETWEEN_SHAPES))
            .and_then(|child| cast_to_container(&child))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        assert_eq!(
            BlendShapeSchema::get_schema_token().as_str(),
            "skelBlendShape"
        );
    }

    #[test]
    fn test_default_locator() {
        let locator = BlendShapeSchema::get_default_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_inbetween_shapes_locator() {
        let locator = BlendShapeSchema::get_inbetween_shapes_locator();
        assert_eq!(locator.len(), 2);
    }
}
