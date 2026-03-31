//! Instanced-by schema for Hydra.
//!
//! Marks a prim as being instanced by one or more instancer prims.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_sdf::Path;
use usd_tf::Token;

// Schema tokens

/// Schema token for instanced-by
pub static INSTANCED_BY: Lazy<Token> = Lazy::new(|| Token::new("instancedBy"));
/// Schema token for paths
pub static PATHS: Lazy<Token> = Lazy::new(|| Token::new("paths"));
/// Schema token for prototype roots
pub static PROTOTYPE_ROOTS: Lazy<Token> = Lazy::new(|| Token::new("prototypeRoots"));

// Typed data sources

/// Data source for path arrays
pub type HdPathArrayDataSource = dyn HdTypedSampledDataSource<Vec<Path>>;
/// Shared handle to path array data source
pub type HdPathArrayDataSourceHandle = Arc<HdPathArrayDataSource>;

/// Schema marking a prim as instanced by another prim.
///
/// Many renderers need to know not what prototypes an instancer has, but
/// rather what instancers a prototype has; this is encoded in "instancedBy".
///
/// A prim is "instancedBy" /Instancer if /Instancer has a prototype path that's
/// a parent of the prim. A complicating exception is if /A instances /A/B,
/// which instances /A/B/C, we don't consider /A to be instancing /A/B/C
/// directly; this is to support nested explicit instancing of things like
/// leaves/trees/forests.
///
/// This value is computed based on the instancer topology of instancer prims
/// in the scene.
///
/// Note: if multiple instancers reference a prototype, it's possible for
/// instancedBy to contain multiple entries. Some renderers may be able to read
/// this directly, but some may need to duplicate prims with an op so that each
/// prim has a single instancer, depending on how the renderer exposes instancing.
///
/// Provides access to:
/// - `paths` - Array of instancer paths that instance this prim
/// - `prototypeRoots` - Array of prototype root paths corresponding to each instancer
///
/// # Location
///
/// Default locator: `instancedBy`
#[derive(Debug, Clone)]
pub struct HdInstancedBySchema {
    schema: HdSchema,
}

impl HdInstancedBySchema {
    /// Create schema from container data source
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Extract instanced-by schema from parent container
    ///
    /// Returns empty schema if not found
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&INSTANCED_BY) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Check if schema is defined (has valid container)
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get underlying container data source
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Get array of instancer paths
    pub fn get_paths(&self) -> Option<HdPathArrayDataSourceHandle> {
        self.schema.get_typed(&PATHS)
    }

    /// Get array of prototype root paths
    pub fn get_prototype_roots(&self) -> Option<HdPathArrayDataSourceHandle> {
        self.schema.get_typed(&PROTOTYPE_ROOTS)
    }

    /// Get schema token for instanced-by
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &INSTANCED_BY
    }

    /// Get default data source locator for instanced-by
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[INSTANCED_BY.clone()])
    }

    /// Get data source locator for paths field
    pub fn get_paths_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[INSTANCED_BY.clone(), PATHS.clone()])
    }

    /// Build retained container with instanced-by data
    ///
    /// # Arguments
    ///
    /// * `paths` - Array of instancer paths
    /// * `prototype_roots` - Array of prototype root paths
    pub fn build_retained(
        paths: Option<HdPathArrayDataSourceHandle>,
        prototype_roots: Option<HdPathArrayDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(v) = paths {
            entries.push((PATHS.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = prototype_roots {
            entries.push((PROTOTYPE_ROOTS.clone(), v as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdInstancedBySchema
///
/// Provides fluent API for constructing instanced-by schemas.
#[allow(dead_code)]
#[derive(Default)]
pub struct HdInstancedBySchemaBuilder {
    /// Instancer paths array
    paths: Option<HdPathArrayDataSourceHandle>,
    /// Prototype roots array
    prototype_roots: Option<HdPathArrayDataSourceHandle>,
}

#[allow(dead_code)]
impl HdInstancedBySchemaBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set instancer paths array
    pub fn set_paths(mut self, v: HdPathArrayDataSourceHandle) -> Self {
        self.paths = Some(v);
        self
    }

    /// Set prototype roots array
    pub fn set_prototype_roots(mut self, v: HdPathArrayDataSourceHandle) -> Self {
        self.prototype_roots = Some(v);
        self
    }

    /// Build container data source with configured values
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdInstancedBySchema::build_retained(self.paths, self.prototype_roots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        let token = HdInstancedBySchema::get_schema_token();
        assert_eq!(token.as_str(), "instancedBy");
    }

    #[test]
    fn test_tokens() {
        assert_eq!(PATHS.as_str(), "paths");
        assert_eq!(PROTOTYPE_ROOTS.as_str(), "prototypeRoots");
    }

    #[test]
    fn test_default_locator() {
        let locator = HdInstancedBySchema::get_default_locator();
        assert!(!locator.is_empty());
    }

    #[test]
    fn test_paths_locator() {
        let locator = HdInstancedBySchema::get_paths_locator();
        assert!(!locator.is_empty());
    }

    #[test]
    fn test_empty_schema() {
        let schema = HdInstancedBySchema {
            schema: HdSchema::empty(),
        };
        assert!(!schema.is_defined());
        assert!(schema.get_container().is_none());
    }

    #[test]
    fn test_build_retained() {
        let container = HdInstancedBySchema::build_retained(None, None);
        assert!(container.get_names().is_empty());
    }

    #[test]
    fn test_builder() {
        let builder = HdInstancedBySchemaBuilder::new();
        let container = builder.build();
        assert!(container.get_names().is_empty());
    }
}
