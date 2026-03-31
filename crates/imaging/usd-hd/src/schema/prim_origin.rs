
//! HdPrimOriginSchema - Prim origin schema.
//!
//! Tracks the original scene path of a prim (e.g. when prefixing scene index
//! alters paths). Corresponds to pxr/imaging/hd/primOriginSchema.h.

use super::base::HdSchema;
use crate::data_source::{HdContainerDataSourceHandle, HdDataSourceLocator};
use once_cell::sync::Lazy;
use usd_sdf::Path;
use usd_tf::Token;

static PRIM_ORIGIN: Lazy<Token> = Lazy::new(|| Token::new("primOrigin"));
static SCENE_PATH: Lazy<Token> = Lazy::new(|| Token::new("scenePath"));

/// Schema for prim origin (original path tracking).
#[derive(Debug, Clone)]
pub struct HdPrimOriginSchema {
    #[allow(dead_code)] // Will be used when get_origin_path is fully implemented
    schema: HdSchema,
}

impl HdPrimOriginSchema {
    /// Construct from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get from parent container at "primOrigin".
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        parent
            .get(&PRIM_ORIGIN)
            .and_then(|h| crate::data_source::cast_to_container(&h).map(Self::new))
    }

    /// Get schema token.
    pub fn get_schema_token() -> &'static Token {
        &PRIM_ORIGIN
    }

    /// Get default locator.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(PRIM_ORIGIN.clone())
    }

    /// Get scene path locator.
    pub fn get_scene_path_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[PRIM_ORIGIN.clone(), SCENE_PATH.clone()])
    }

    /// Extracts SdfPath from container for the given name.
    ///
    /// Used when the prim origin stores path data via OriginPathDataSource.
    /// Returns empty path if not found.
    pub fn get_origin_path(&self, _name: &Token) -> Path {
        // C++ uses OriginPath typed data source; we'd need HdPathDataSource
        // or similar. For now return empty path.
        Path::default()
    }
}
