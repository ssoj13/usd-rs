//! GeomModelSchema - Hydra schema for geom model data.
//!
//! Port of pxr/usdImaging/usdImaging/geomModelSchema.h
//!
//! Provides data source schema for geom model API in Hydra.
//! Contains draw mode settings, card geometry, and texture paths.

use usd_hd::data_source::cast_to_container;
use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
};
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static GEOM_MODEL: LazyLock<Token> = LazyLock::new(|| Token::new("geomModel"));
    pub static DRAW_MODE: LazyLock<Token> = LazyLock::new(|| Token::new("drawMode"));
    #[allow(dead_code)] // Used in builder
    pub static APPLY_DRAW_MODE: LazyLock<Token> = LazyLock::new(|| Token::new("applyDrawMode"));
    #[allow(dead_code)] // Used in builder
    pub static DRAW_MODE_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("drawModeColor"));
    #[allow(dead_code)] // Used in builder
    pub static CARD_GEOMETRY: LazyLock<Token> = LazyLock::new(|| Token::new("cardGeometry"));
    #[allow(dead_code)] // Used in builder
    pub static CARD_TEXTURE_X_POS: LazyLock<Token> =
        LazyLock::new(|| Token::new("cardTextureXPos"));
    #[allow(dead_code)] // Used in builder
    pub static CARD_TEXTURE_Y_POS: LazyLock<Token> =
        LazyLock::new(|| Token::new("cardTextureYPos"));
    #[allow(dead_code)] // Used in builder
    pub static CARD_TEXTURE_Z_POS: LazyLock<Token> =
        LazyLock::new(|| Token::new("cardTextureZPos"));
    #[allow(dead_code)] // Used in builder
    pub static CARD_TEXTURE_X_NEG: LazyLock<Token> =
        LazyLock::new(|| Token::new("cardTextureXNeg"));
    #[allow(dead_code)] // Used in builder
    pub static CARD_TEXTURE_Y_NEG: LazyLock<Token> =
        LazyLock::new(|| Token::new("cardTextureYNeg"));
    #[allow(dead_code)] // Used in builder
    pub static CARD_TEXTURE_Z_NEG: LazyLock<Token> =
        LazyLock::new(|| Token::new("cardTextureZNeg"));

    // Draw mode values
    #[allow(dead_code)] // Public token
    pub static DEFAULT: LazyLock<Token> = LazyLock::new(|| Token::new("default"));
    #[allow(dead_code)] // Public token
    pub static ORIGIN: LazyLock<Token> = LazyLock::new(|| Token::new("origin"));
    #[allow(dead_code)] // Public token
    pub static BOUNDS: LazyLock<Token> = LazyLock::new(|| Token::new("bounds"));
    #[allow(dead_code)] // Public token
    pub static CARDS: LazyLock<Token> = LazyLock::new(|| Token::new("cards"));
    #[allow(dead_code)] // Public token
    pub static INHERITED: LazyLock<Token> = LazyLock::new(|| Token::new("inherited"));

    // Card geometry values
    #[allow(dead_code)] // Public token
    pub static CROSS: LazyLock<Token> = LazyLock::new(|| Token::new("cross"));
    #[allow(dead_code)] // Public token
    pub static BOX: LazyLock<Token> = LazyLock::new(|| Token::new("box"));
    #[allow(dead_code)] // Public token
    pub static FROM_TEXTURE: LazyLock<Token> = LazyLock::new(|| Token::new("fromTexture"));
}

// ============================================================================
// GeomModelSchema
// ============================================================================

/// Schema for geom model data in Hydra.
///
/// Contains draw mode settings (default, origin, bounds, cards),
/// card geometry configuration, and texture paths for each axis.
#[derive(Debug, Clone)]
pub struct GeomModelSchema {
    #[allow(dead_code)] // Part of schema infrastructure
    container: Option<HdContainerDataSourceHandle>,
}

impl GeomModelSchema {
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
        tokens::GEOM_MODEL.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::GEOM_MODEL.clone())
    }

    /// Get the draw mode locator.
    pub fn get_draw_mode_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::GEOM_MODEL.clone(), tokens::DRAW_MODE.clone())
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        let child = parent.get(&tokens::GEOM_MODEL)?;
        let container = cast_to_container(&child)?;
        Some(Self::new(Some(container)))
    }

    /// Build draw mode data source for common values.
    ///
    /// Caches static sampled token data sources for:
    /// default, origin, bounds, cards, inherited.
    pub fn build_draw_mode_data_source(draw_mode: &Token) -> HdDataSourceBaseHandle {
        if draw_mode == &*tokens::DEFAULT {
            static DS: std::sync::LazyLock<HdDataSourceBaseHandle> =
                std::sync::LazyLock::new(|| {
                    HdRetainedTypedSampledDataSource::new(tokens::DEFAULT.clone())
                });
            return DS.clone();
        }
        if draw_mode == &*tokens::ORIGIN {
            static DS: std::sync::LazyLock<HdDataSourceBaseHandle> =
                std::sync::LazyLock::new(|| {
                    HdRetainedTypedSampledDataSource::new(tokens::ORIGIN.clone())
                });
            return DS.clone();
        }
        if draw_mode == &*tokens::BOUNDS {
            static DS: std::sync::LazyLock<HdDataSourceBaseHandle> =
                std::sync::LazyLock::new(|| {
                    HdRetainedTypedSampledDataSource::new(tokens::BOUNDS.clone())
                });
            return DS.clone();
        }
        if draw_mode == &*tokens::CARDS {
            static DS: std::sync::LazyLock<HdDataSourceBaseHandle> =
                std::sync::LazyLock::new(|| {
                    HdRetainedTypedSampledDataSource::new(tokens::CARDS.clone())
                });
            return DS.clone();
        }
        if draw_mode == &*tokens::INHERITED {
            static DS: std::sync::LazyLock<HdDataSourceBaseHandle> =
                std::sync::LazyLock::new(|| {
                    HdRetainedTypedSampledDataSource::new(tokens::INHERITED.clone())
                });
            return DS.clone();
        }
        HdRetainedTypedSampledDataSource::new(draw_mode.clone())
    }

    /// Build card geometry data source for common values.
    ///
    /// Caches static sampled token data sources for:
    /// cross, box, fromTexture.
    pub fn build_card_geometry_data_source(card_geometry: &Token) -> HdDataSourceBaseHandle {
        if card_geometry == &*tokens::CROSS {
            static DS: std::sync::LazyLock<HdDataSourceBaseHandle> =
                std::sync::LazyLock::new(|| {
                    HdRetainedTypedSampledDataSource::new(tokens::CROSS.clone())
                });
            return DS.clone();
        }
        if card_geometry == &*tokens::BOX {
            static DS: std::sync::LazyLock<HdDataSourceBaseHandle> =
                std::sync::LazyLock::new(|| {
                    HdRetainedTypedSampledDataSource::new(tokens::BOX.clone())
                });
            return DS.clone();
        }
        if card_geometry == &*tokens::FROM_TEXTURE {
            static DS: std::sync::LazyLock<HdDataSourceBaseHandle> =
                std::sync::LazyLock::new(|| {
                    HdRetainedTypedSampledDataSource::new(tokens::FROM_TEXTURE.clone())
                });
            return DS.clone();
        }
        HdRetainedTypedSampledDataSource::new(card_geometry.clone())
    }
}

// ============================================================================
// GeomModelSchemaBuilder
// ============================================================================

/// Builder for GeomModelSchema data sources.
#[derive(Debug, Default)]
pub struct GeomModelSchemaBuilder {
    draw_mode: Option<HdDataSourceBaseHandle>,
    apply_draw_mode: Option<HdDataSourceBaseHandle>,
    draw_mode_color: Option<HdDataSourceBaseHandle>,
    card_geometry: Option<HdDataSourceBaseHandle>,
    card_texture_x_pos: Option<HdDataSourceBaseHandle>,
    card_texture_y_pos: Option<HdDataSourceBaseHandle>,
    card_texture_z_pos: Option<HdDataSourceBaseHandle>,
    card_texture_x_neg: Option<HdDataSourceBaseHandle>,
    card_texture_y_neg: Option<HdDataSourceBaseHandle>,
    card_texture_z_neg: Option<HdDataSourceBaseHandle>,
}

impl GeomModelSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the draw mode data source.
    pub fn set_draw_mode(mut self, draw_mode: HdDataSourceBaseHandle) -> Self {
        self.draw_mode = Some(draw_mode);
        self
    }

    /// Set the apply draw mode data source.
    pub fn set_apply_draw_mode(mut self, apply: HdDataSourceBaseHandle) -> Self {
        self.apply_draw_mode = Some(apply);
        self
    }

    /// Set the draw mode color data source.
    pub fn set_draw_mode_color(mut self, color: HdDataSourceBaseHandle) -> Self {
        self.draw_mode_color = Some(color);
        self
    }

    /// Set the card geometry data source.
    pub fn set_card_geometry(mut self, geometry: HdDataSourceBaseHandle) -> Self {
        self.card_geometry = Some(geometry);
        self
    }

    /// Set the card texture X positive data source.
    pub fn set_card_texture_x_pos(mut self, texture: HdDataSourceBaseHandle) -> Self {
        self.card_texture_x_pos = Some(texture);
        self
    }

    /// Set the card texture Y positive data source.
    pub fn set_card_texture_y_pos(mut self, texture: HdDataSourceBaseHandle) -> Self {
        self.card_texture_y_pos = Some(texture);
        self
    }

    /// Set the card texture Z positive data source.
    pub fn set_card_texture_z_pos(mut self, texture: HdDataSourceBaseHandle) -> Self {
        self.card_texture_z_pos = Some(texture);
        self
    }

    /// Set the card texture X negative data source.
    pub fn set_card_texture_x_neg(mut self, texture: HdDataSourceBaseHandle) -> Self {
        self.card_texture_x_neg = Some(texture);
        self
    }

    /// Set the card texture Y negative data source.
    pub fn set_card_texture_y_neg(mut self, texture: HdDataSourceBaseHandle) -> Self {
        self.card_texture_y_neg = Some(texture);
        self
    }

    /// Set the card texture Z negative data source.
    pub fn set_card_texture_z_neg(mut self, texture: HdDataSourceBaseHandle) -> Self {
        self.card_texture_z_neg = Some(texture);
        self
    }

    /// Build the container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();
        if let Some(v) = self.draw_mode {
            entries.push((tokens::DRAW_MODE.clone(), v));
        }
        if let Some(v) = self.apply_draw_mode {
            entries.push((tokens::APPLY_DRAW_MODE.clone(), v));
        }
        if let Some(v) = self.draw_mode_color {
            entries.push((tokens::DRAW_MODE_COLOR.clone(), v));
        }
        if let Some(v) = self.card_geometry {
            entries.push((tokens::CARD_GEOMETRY.clone(), v));
        }
        if let Some(v) = self.card_texture_x_pos {
            entries.push((tokens::CARD_TEXTURE_X_POS.clone(), v));
        }
        if let Some(v) = self.card_texture_y_pos {
            entries.push((tokens::CARD_TEXTURE_Y_POS.clone(), v));
        }
        if let Some(v) = self.card_texture_z_pos {
            entries.push((tokens::CARD_TEXTURE_Z_POS.clone(), v));
        }
        if let Some(v) = self.card_texture_x_neg {
            entries.push((tokens::CARD_TEXTURE_X_NEG.clone(), v));
        }
        if let Some(v) = self.card_texture_y_neg {
            entries.push((tokens::CARD_TEXTURE_Y_NEG.clone(), v));
        }
        if let Some(v) = self.card_texture_z_neg {
            entries.push((tokens::CARD_TEXTURE_Z_NEG.clone(), v));
        }
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        assert_eq!(GeomModelSchema::get_schema_token().as_str(), "geomModel");
    }

    #[test]
    fn test_draw_mode_locator() {
        let locator = GeomModelSchema::get_draw_mode_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_draw_mode_tokens() {
        assert_eq!(tokens::DEFAULT.as_str(), "default");
        assert_eq!(tokens::ORIGIN.as_str(), "origin");
        assert_eq!(tokens::BOUNDS.as_str(), "bounds");
        assert_eq!(tokens::CARDS.as_str(), "cards");
        assert_eq!(tokens::INHERITED.as_str(), "inherited");
    }

    #[test]
    fn test_card_geometry_tokens() {
        assert_eq!(tokens::CROSS.as_str(), "cross");
        assert_eq!(tokens::BOX.as_str(), "box");
        assert_eq!(tokens::FROM_TEXTURE.as_str(), "fromTexture");
    }

    #[test]
    fn test_builder() {
        let _schema = GeomModelSchemaBuilder::new().build();
    }

    #[test]
    fn test_is_defined() {
        let schema = GeomModelSchema::new(None);
        assert!(!schema.is_defined());
    }

    #[test]
    fn test_build_draw_mode_data_source() {
        let _ds = GeomModelSchema::build_draw_mode_data_source(&tokens::DEFAULT);
    }

    #[test]
    fn test_build_card_geometry_data_source() {
        let _ds = GeomModelSchema::build_card_geometry_data_source(&tokens::CROSS);
    }
}
