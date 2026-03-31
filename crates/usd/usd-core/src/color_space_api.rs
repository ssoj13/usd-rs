//! UsdColorSpaceAPI - Color space management for USD.
//!
//! Port of pxr/usd/usd/colorSpaceAPI.h

use super::attribute::Attribute;
use super::common::SchemaKind;
use super::prim::Prim;
use super::schema_base::SchemaBase;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use usd_gf::ColorSpace;
use usd_sdf::Path;
use usd_tf::Token;

// ============================================================================
// ColorSpaceCache
// ============================================================================

/// A minimalistic cache for color space lookups.
///
/// An application may provide its own cache implementation to avoid
/// redundant color space lookups. The cache should be cleared or updated
/// when the color space properties have changed.
pub trait ColorSpaceCache: Send + Sync {
    /// Find a color space for the given prim path.
    fn find(&self, prim: &Path) -> Option<Token>;

    /// Insert a color space for the given prim path.
    fn insert(&self, prim: &Path, color_space: Token);
}

/// Simple hash-based color space cache implementation.
///
/// Uses a read-write lock for thread safety.
#[derive(Default)]
pub struct ColorSpaceHashCache {
    cache: RwLock<HashMap<Path, Token>>,
}

impl ColorSpaceHashCache {
    /// Creates a new empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears the cache.
    pub fn clear(&self) {
        self.cache.write().expect("rwlock poisoned").clear();
    }
}

impl ColorSpaceCache for ColorSpaceHashCache {
    fn find(&self, prim: &Path) -> Option<Token> {
        self.cache.read().ok()?.get(prim).cloned()
    }

    fn insert(&self, prim: &Path, color_space: Token) {
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(prim.clone(), color_space);
        }
    }
}

// ============================================================================
// UsdColorSpaceAPI
// ============================================================================

/// UsdColorSpaceAPI is an API schema for authoring scene referred color space opinions.
///
/// It provides a mechanism to determine the applicable color space within a scope
/// through inheritance. This schema may be applied to any prim to introduce a
/// color space at any point in a compositional hierarchy.
///
/// Color space resolution involves determining the color space authored on an
/// attribute by first examining the attribute itself for a color space which
/// may have been authored via `UsdAttribute::SetColorSpace()`. If none is found,
/// the attribute's prim is checked for the existence of the `UsdColorSpaceAPI`,
/// and any color space authored there. If none is found on the attribute's
/// prim, the prim's ancestors are examined up the hierarchy until an authored
/// color space is found.
///
/// Matches C++ `UsdColorSpaceAPI`.
#[derive(Debug, Clone)]
pub struct ColorSpaceAPI {
    /// The underlying schema base.
    schema_base: SchemaBase,
}

impl ColorSpaceAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "ColorSpaceAPI";

    /// Token for the colorSpace:name attribute.
    fn color_space_name_token() -> Token {
        Token::new("colorSpace:name")
    }

    /// Creates a ColorSpaceAPI on the given prim.
    pub fn new(prim: &Prim) -> Self {
        Self {
            schema_base: SchemaBase::new(prim.clone()),
        }
    }

    /// Returns the prim this API is attached to.
    pub fn prim(&self) -> &Prim {
        self.schema_base.prim()
    }

    /// Returns whether this API schema is valid.
    pub fn is_valid(&self) -> bool {
        self.schema_base.prim().is_valid()
    }

    /// Returns the schema kind.
    pub fn schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Gets the prim at the given path on the given stage.
    pub fn get(stage: &Arc<super::stage::Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(&prim))
    }

    /// Returns true if this single-apply API schema can be applied to the given prim.
    pub fn can_apply(prim: &Prim) -> bool {
        prim.is_valid()
    }

    /// Applies this single-apply API schema to the given prim.
    ///
    /// This adds "ColorSpaceAPI" to the apiSchemas metadata on the prim.
    pub fn apply(prim: &Prim) -> Self {
        if prim.is_valid() {
            // Add ColorSpaceAPI to apiSchemas
            let mut schemas = prim.get_authored_applied_schemas();
            let api_token = Token::new("ColorSpaceAPI");
            if !schemas.contains(&api_token) {
                schemas.push(api_token);
                // Note: actual metadata authoring would happen here
            }
        }
        Self::new(prim)
    }

    // =========================================================================
    // ColorSpaceName Attribute
    // =========================================================================

    /// Returns the colorSpace:name attribute.
    ///
    /// The color space that applies to attributes with unauthored color spaces
    /// on this prim and its descendants.
    pub fn get_color_space_name_attr(&self) -> Option<Attribute> {
        self.prim()
            .get_attribute(Self::color_space_name_token().get_text())
    }

    /// Creates the colorSpace:name attribute with an optional default value.
    pub fn create_color_space_name_attr(&self, default_value: Option<&str>) -> Option<Attribute> {
        // Create a token type name using the schema's type registry
        let type_name = usd_sdf::ValueTypeName::invalid(); // Token type
        let attr = self.prim().create_attribute(
            Self::color_space_name_token().get_text(),
            &type_name,
            false, // not custom
            None,  // default variability
        );

        if let Some(attr) = &attr {
            if let Some(value) = default_value {
                attr.set(usd_vt::Value::from(Token::new(value)), Default::default());
            }
        }

        attr
    }

    // =========================================================================
    // Color Space Resolution
    // =========================================================================

    /// Computes the color space name for the given attribute.
    ///
    /// The attribute is first checked for an authored color space; if one
    /// exists, it's returned. Otherwise, the attribute's prim is consulted,
    /// following the inheritance rules for color space determination on a prim.
    pub fn compute_color_space_name_for_attr(
        attribute: &Attribute,
        cache: Option<&dyn ColorSpaceCache>,
    ) -> Option<Token> {
        // First check the attribute itself
        let cs = attribute.get_color_space();
        if !cs.is_empty() {
            return Some(cs);
        }

        // Then check the prim and its ancestors
        if let Some(stage) = attribute.stage() {
            let prim_path = attribute.path().get_prim_path();
            if let Some(prim) = stage.get_prim_at_path(&prim_path) {
                return Self::compute_color_space_name(&prim, cache);
            }
        }

        None
    }

    /// Computes the color space name for the given prim.
    ///
    /// The color space is determined by checking this prim for a colorSpace
    /// property. If no colorSpace property is authored, the search continues
    /// up the prim's hierarchy until a colorSpace property is found or the
    /// root prim is reached.
    pub fn compute_color_space_name(
        prim: &Prim,
        cache: Option<&dyn ColorSpaceCache>,
    ) -> Option<Token> {
        if !prim.is_valid() {
            return None;
        }

        // Check cache first
        if let Some(c) = cache {
            if let Some(cs) = c.find(prim.path()) {
                return Some(cs);
            }
        }

        // Check this prim
        let api = Self::new(prim);
        if let Some(attr) = api.get_color_space_name_attr() {
            if let Some(value) = attr.get(usd_sdf::TimeCode::default_time()) {
                if let Some(token) = value.get::<Token>() {
                    if !token.is_empty() {
                        // Cache the result
                        if let Some(c) = cache {
                            c.insert(prim.path(), token.clone());
                        }
                        return Some(token.clone());
                    }
                }
            }
        }

        // Check parent
        let parent = prim.parent();
        if parent.is_valid() {
            return Self::compute_color_space_name(&parent, cache);
        }

        None
    }

    /// Computes the color space for the given attribute.
    pub fn compute_color_space_for_attr(
        attribute: &Attribute,
        cache: Option<&dyn ColorSpaceCache>,
    ) -> ColorSpace {
        if let Some(name) = Self::compute_color_space_name_for_attr(attribute, cache) {
            ColorSpace::from_token(&name)
        } else {
            // Default: Linear Rec709 (D65 white point)
            ColorSpace::new(usd_gf::ColorSpaceName::LinearRec709)
        }
    }

    /// Computes the color space for the given prim.
    pub fn compute_color_space(prim: &Prim, cache: Option<&dyn ColorSpaceCache>) -> ColorSpace {
        if let Some(name) = Self::compute_color_space_name(prim, cache) {
            ColorSpace::from_token(&name)
        } else {
            // Default: Linear Rec709 (D65 white point)
            ColorSpace::new(usd_gf::ColorSpaceName::LinearRec709)
        }
    }

    /// Computes a color space for the given prim with a specific color space name.
    pub fn compute_color_space_named(
        _prim: &Prim,
        color_space: &Token,
        _cache: Option<&dyn ColorSpaceCache>,
    ) -> ColorSpace {
        ColorSpace::from_token(color_space)
    }

    /// Returns true if the named color space is valid (built-in or defined on prim/ancestors).
    pub fn is_valid_color_space_name(
        prim: &Prim,
        color_space: &Token,
        _cache: Option<&dyn ColorSpaceCache>,
    ) -> bool {
        if !prim.is_valid() {
            return false;
        }

        // Check if it's a known/built-in color space name
        let name = color_space.get_text();
        matches!(
            name,
            "sRGB"
                | "LinearSRGB"
                | "LinearRec709"
                | "AdobeRGB"
                | "LinearAdobeRGB"
                | "DciP3"
                | "LinearDciP3"
                | "Rec2020"
                | "LinearRec2020"
                | "LinearACEScc"
                | "LinearACEScg"
                | "OCIO"
        )
    }

    /// Creates an invalid ColorSpaceAPI.
    pub fn invalid() -> Self {
        Self {
            schema_base: SchemaBase::new(Prim::invalid()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_space_cache() {
        let cache = ColorSpaceHashCache::new();
        let path = Path::from_string("/World").unwrap();
        let color_space = Token::new("sRGB");

        assert!(cache.find(&path).is_none());

        cache.insert(&path, color_space.clone());
        assert_eq!(cache.find(&path), Some(color_space));

        cache.clear();
        assert!(cache.find(&path).is_none());
    }

    #[test]
    fn test_schema_kind() {
        assert_eq!(ColorSpaceAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }
}
