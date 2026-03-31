
//! Asset resolution system schema for Hydra scene indices.
//!
//! Provides HdarSystemSchema which wraps asset resolver context data
//! in the scene index hierarchy.

use once_cell::sync::Lazy;
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdResolverContextDataSourceHandle,
};
use usd_hd::scene_index::HdSceneIndexHandle;
use usd_hd::schema::{HdSchema, HdSystemSchema};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Tokens for hdar system schema.
pub struct HdarSystemSchemaTokens;

impl HdarSystemSchemaTokens {}

/// Token for "assetResolution" field
pub static ASSET_RESOLUTION: Lazy<Token> = Lazy::new(|| Token::new("assetResolution"));

/// Token for "resolverContext" field
pub static RESOLVER_CONTEXT: Lazy<Token> = Lazy::new(|| Token::new("resolverContext"));

/// Asset resolution system schema for scene indices.
///
/// The HdarSystemSchema provides access to asset resolver context data
/// stored in the scene index hierarchy under "system.assetResolution".
///
/// This schema extends HdSystemSchema to provide typed access to the
/// resolver context used for asset resolution.
///
/// # Schema Location
///
/// - Default locator: `system.assetResolution`
/// - Contains: `resolverContext` field with ArResolverContext data
///
/// # Usage
///
/// ```rust
/// use usd_hdar::HdarSystemSchema;
/// use usd_hd::scene_index::HdSceneIndexHandle;
/// use usd_sdf::Path;
///
/// // Get asset resolution context from scene hierarchy
/// // let scene_index: HdSceneIndexHandle = ...;
/// // let path = Path::abs_root();
/// // let (container, found_path) = HdarSystemSchema::get_from_path(&scene_index, &path);
/// // if let Some(container) = container {
/// //     let schema = HdarSystemSchema::new(container);
/// //     if let Some(ctx) = schema.get_resolver_context() {
/// //         // Use resolver context
/// //     }
/// // }
/// ```
#[derive(Debug, Clone)]
pub struct HdarSystemSchema {
    base: HdSchema,
}

impl HdarSystemSchema {
    /// Creates a new hdar system schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            base: HdSchema::new(container),
        }
    }

    /// Creates an empty hdar system schema.
    pub fn empty() -> Self {
        Self {
            base: HdSchema::empty(),
        }
    }

    /// Returns the underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.base.get_container()
    }

    /// Returns true if this schema is defined (has a non-null container).
    pub fn is_defined(&self) -> bool {
        self.base.is_defined()
    }

    /// Retrieves an hdar system schema from a parent container.
    ///
    /// Looks for a container data source at the "assetResolution" token
    /// in the parent container and constructs an HdarSystemSchema instance.
    ///
    /// Because the requested container may not exist, the result should be
    /// checked with is_defined() before use.
    ///
    /// # Arguments
    ///
    /// * `from_parent` - Parent container to extract schema from
    ///
    /// # Returns
    ///
    /// HdarSystemSchema wrapping the assetResolution container, or empty if not found
    pub fn get_from_parent(from_parent: &HdContainerDataSourceHandle) -> Self {
        use usd_hd::data_source::cast_to_container;

        if let Some(asset_res_ds) = from_parent.get(&ASSET_RESOLUTION) {
            // Try to cast base data source to container
            if let Some(container) = cast_to_container(&asset_res_ds) {
                return Self::new(container);
            }
        }
        Self::empty()
    }

    /// Evaluates the asset resolution system data source for a path.
    ///
    /// Walks up the scene hierarchy from from_path looking for a system
    /// container with an "assetResolution" field.
    ///
    /// If found, returns the container and the path where it was found.
    ///
    /// This operation is linear in the length of from_path.
    ///
    /// # Arguments
    ///
    /// * `input_scene` - The scene index to query
    /// * `from_path` - The path to start searching from
    ///
    /// # Returns
    ///
    /// Tuple of (container, path_where_found) or (None, None) if not found
    ///
    /// # Examples
    ///
    /// ```rust
    /// use usd_hdar::HdarSystemSchema;
    /// use usd_sdf::Path;
    ///
    /// // let scene: HdSceneIndexHandle = ...;
    /// // let path = Path::from_str("/World/Characters/Hero").unwrap();
    /// // let (container, found_at) = HdarSystemSchema::get_from_path(&scene, &path);
    /// // if let Some(container) = container {
    /// //     println!("Found asset resolution data at: {:?}", found_at);
    /// // }
    /// ```
    pub fn get_from_path(
        input_scene: &HdSceneIndexHandle,
        from_path: &SdfPath,
    ) -> (Option<HdContainerDataSourceHandle>, Option<SdfPath>) {
        // Use HdSystemSchema to get the system.assetResolution data
        let (ds, found_path) =
            HdSystemSchema::get_from_path(input_scene, from_path, &ASSET_RESOLUTION);

        if let Some(ds) = ds {
            // Try to cast to container
            use usd_hd::data_source::cast_to_container;
            if let Some(container) = cast_to_container(&ds) {
                return (Some(container), found_path);
            } else {
                // Log error: system.assetResolution is not a container
                eprintln!(
                    "system.assetResolution at {:?} is not a container",
                    found_path
                );
            }
        }

        (None, None)
    }

    /// Returns the resolver context data source.
    ///
    /// Retrieves the "resolverContext" field from this schema's container.
    ///
    /// # Returns
    ///
    /// Handle to resolver context data source, or None if not present or wrong type.
    ///
    /// Uses as_any() and downcast_ref to convert from HdDataSourceBaseHandle to
    /// HdRetainedTypedSampledDataSource<ResolverContext>. Matches C++ _GetTypedDataSource.
    pub fn get_resolver_context(&self) -> Option<HdResolverContextDataSourceHandle> {
        use usd_ar::ResolverContext;
        use usd_hd::data_source::{HdRetainedTypedSampledDataSource, HdTypedSampledDataSource};

        let container = self.base.get_container()?;
        let ctx_ds = container.get(&RESOLVER_CONTEXT)?;

        // Use as_any() from HdDataSourceBase to downcast to the concrete type.
        // The C++ Cast does equivalent: dynamic_cast to the typed interface.
        let any_ref = ctx_ds.as_ref().as_any();
        if let Some(typed) =
            any_ref.downcast_ref::<HdRetainedTypedSampledDataSource<ResolverContext>>()
        {
            // Reconstruct Arc from the reference: we need Arc. The ctx_ds is Arc<dyn HdDataSourceBase>
            // pointing to the same allocation. Clone ctx_ds and try to get typed handle.
            // Since we verified the type, we can use Arc::from_raw with the same pointer - but that's
            // unsafe. Simpler: return a new Arc from the cloned value if it's HdRetainedTypedSampledDataSource.
            let value = typed.get_typed_value(0.0);
            Some(HdRetainedTypedSampledDataSource::new(value) as HdResolverContextDataSourceHandle)
        } else {
            None
        }
    }

    /// Returns the schema token ("assetResolution"). Cached static ref matching C++ GetSchemaToken().
    pub fn get_schema_token() -> &'static Token {
        &ASSET_RESOLUTION
    }

    /// Returns the default locator "system.assetResolution". Cached static matching C++ GetDefaultLocator().
    pub fn get_default_locator() -> &'static HdDataSourceLocator {
        static LOCATOR: Lazy<HdDataSourceLocator> = Lazy::new(|| {
            HdDataSourceLocator::new(&[usd_hd::schema::SYSTEM.clone(), ASSET_RESOLUTION.clone()])
        });
        &LOCATOR
    }

    /// Builds a retained container with the given resolver context.
    ///
    /// Creates a container suitable for storing in the scene index
    /// under the "assetResolution" key.
    ///
    /// # Arguments
    ///
    /// * `resolver_context` - Resolver context data source to include
    ///
    /// # Returns
    ///
    /// Container with "resolverContext" field, or empty container if None
    pub fn build_retained(
        resolver_context: Option<HdResolverContextDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use usd_hd::data_source::HdRetainedContainerDataSource;

        if let Some(ctx) = resolver_context {
            // Clone the data source into a base handle
            let ctx_base = ctx.clone_box();
            HdRetainedContainerDataSource::from_entries(&[(RESOLVER_CONTEXT.clone(), ctx_base)])
        } else {
            HdRetainedContainerDataSource::new_empty()
        }
    }
}

impl Default for HdarSystemSchema {
    fn default() -> Self {
        Self::empty()
    }
}

/// Builder for HdarSystemSchema containers.
///
/// Provides a fluent API for constructing asset resolution containers.
///
/// # Examples
///
/// ```rust
/// use usd_hdar::HdarSystemSchemaBuilder;
///
/// // let resolver_ctx_ds = ...;
/// // let container = HdarSystemSchemaBuilder::new()
/// //     .set_resolver_context(resolver_ctx_ds)
/// //     .build();
/// ```
pub struct HdarSystemSchemaBuilder {
    resolver_context: Option<HdResolverContextDataSourceHandle>,
}

impl HdarSystemSchemaBuilder {
    /// Creates a new empty builder.
    pub fn new() -> Self {
        Self {
            resolver_context: None,
        }
    }

    /// Sets the resolver context data source.
    ///
    /// # Arguments
    ///
    /// * `resolver_context` - Resolver context data source
    ///
    /// # Returns
    ///
    /// Self for method chaining
    pub fn set_resolver_context(
        mut self,
        resolver_context: HdResolverContextDataSourceHandle,
    ) -> Self {
        self.resolver_context = Some(resolver_context);
        self
    }

    /// Builds the container data source.
    ///
    /// # Returns
    ///
    /// Container with configured fields
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdarSystemSchema::build_retained(self.resolver_context)
    }
}

impl Default for HdarSystemSchemaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::data_source::HdRetainedContainerDataSource;

    #[test]
    fn test_empty_schema() {
        let schema = HdarSystemSchema::empty();
        assert!(!schema.is_defined());
    }

    #[test]
    fn test_schema_with_container() {
        let container = HdRetainedContainerDataSource::new_empty();
        let schema = HdarSystemSchema::new(container);
        assert!(schema.is_defined());
    }

    #[test]
    fn test_get_schema_token() {
        let token = HdarSystemSchema::get_schema_token();
        assert_eq!(token.as_str(), "assetResolution");
    }

    #[test]
    fn test_get_default_locator() {
        let locator = HdarSystemSchema::get_default_locator();
        let elements = locator.elements();
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].as_str(), "system");
        assert_eq!(elements[1].as_str(), "assetResolution");
    }

    #[test]
    fn test_tokens() {
        assert_eq!(ASSET_RESOLUTION.as_str(), "assetResolution");
        assert_eq!(RESOLVER_CONTEXT.as_str(), "resolverContext");
    }

    #[test]
    fn test_build_retained_empty() {
        let container = HdarSystemSchema::build_retained(None);
        let schema = HdarSystemSchema::new(container);
        assert!(schema.is_defined());
        assert!(schema.get_resolver_context().is_none());
    }

    #[test]
    fn test_builder() {
        let builder = HdarSystemSchemaBuilder::new();
        let container = builder.build();
        let schema = HdarSystemSchema::new(container);
        assert!(schema.is_defined());
    }

    #[test]
    fn test_builder_default() {
        let container = HdarSystemSchemaBuilder::default().build();
        let schema = HdarSystemSchema::new(container);
        assert!(schema.is_defined());
    }
}
