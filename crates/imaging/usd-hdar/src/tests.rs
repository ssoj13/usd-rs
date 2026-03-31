
//! Integration tests for hdar module.

#[cfg(test)]
mod integration_tests {
    use crate::{ASSET_RESOLUTION, HdarSystemSchema, HdarSystemSchemaBuilder, RESOLVER_CONTEXT};
    use std::sync::Arc;
    use usd_ar::ResolverContext;
    use usd_hd::data_source::{
        HdContainerDataSourceHandle, HdResolverContextDataSourceHandle,
        HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource, HdTypedSampledDataSource,
    };
    use usd_hd::scene_index::{HdRetainedSceneIndex, HdSceneIndexHandle, RetainedAddedPrimEntry};
    use usd_hd::schema::{HdSystemSchema, SYSTEM};
    use usd_sdf::Path as SdfPath;

    #[test]
    fn test_hdar_schema_basic() {
        // Create empty schema
        let schema = HdarSystemSchema::empty();
        assert!(!schema.is_defined());

        // Create schema with container
        let container = HdRetainedContainerDataSource::new_empty();
        let schema = HdarSystemSchema::new(container);
        assert!(schema.is_defined());
    }

    #[test]
    fn test_hdar_tokens() {
        assert_eq!(ASSET_RESOLUTION.as_str(), "assetResolution");
        assert_eq!(RESOLVER_CONTEXT.as_str(), "resolverContext");
    }

    #[test]
    fn test_schema_token_and_locator() {
        let token = HdarSystemSchema::get_schema_token();
        assert_eq!(token.as_str(), "assetResolution");

        let locator = HdarSystemSchema::get_default_locator();
        let elements = locator.elements();
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].as_str(), "system");
        assert_eq!(elements[1].as_str(), "assetResolution");
    }

    #[test]
    fn test_build_retained() {
        // Build with no resolver context
        let container = HdarSystemSchema::build_retained(None);
        let schema = HdarSystemSchema::new(container);
        assert!(schema.is_defined());
        assert!(schema.get_resolver_context().is_none());
    }

    #[test]
    fn test_builder_pattern() {
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

    #[test]
    fn test_get_from_parent_empty() {
        let parent = HdRetainedContainerDataSource::new_empty();
        let parent_dyn: HdContainerDataSourceHandle = parent;
        let schema = HdarSystemSchema::get_from_parent(&parent_dyn);
        assert!(!schema.is_defined());
    }

    #[test]
    fn test_get_from_parent_with_data() {
        // Create asset resolution container
        let ar_container = HdRetainedContainerDataSource::new_empty();
        // Convert to HdContainerDataSourceHandle
        let ar_container_handle: HdContainerDataSourceHandle = ar_container;

        // Create parent with assetResolution field
        let parent = HdRetainedContainerDataSource::new_1(
            ASSET_RESOLUTION.clone(),
            ar_container_handle.clone_box(),
        );

        let parent_dyn: HdContainerDataSourceHandle = parent;
        let schema = HdarSystemSchema::get_from_parent(&parent_dyn);
        assert!(schema.is_defined());
    }

    #[test]
    fn test_hd_system_schema_basic() {
        let system_container = HdRetainedContainerDataSource::new_empty();
        let schema = HdSystemSchema::new(system_container);
        assert!(schema.is_defined());
    }

    #[test]
    fn test_hd_system_schema_tokens() {
        assert_eq!(SYSTEM.as_str(), "system");
    }

    #[test]
    fn test_hd_system_schema_get_from_parent() {
        // Create system container
        let system_container = HdRetainedContainerDataSource::new_empty();
        // Convert to HdContainerDataSourceHandle
        let system_container_handle: HdContainerDataSourceHandle = system_container;

        // Create prim container with system field
        let prim_ds = HdRetainedContainerDataSource::new_1(
            SYSTEM.clone(),
            system_container_handle.clone_box(),
        );

        let prim_dyn: HdContainerDataSourceHandle = prim_ds;
        let schema = HdSystemSchema::get_from_parent(&prim_dyn);
        assert!(schema.is_defined());
    }

    #[test]
    fn test_hd_system_schema_locator() {
        let locator = HdSystemSchema::get_default_locator();
        let elements = locator.elements();
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].as_str(), "system");
    }

    // Integration test: Full hierarchy with asset resolution context
    #[test]
    fn test_full_asset_resolution_hierarchy() {
        // Create a retained scene index
        let scene_index = HdRetainedSceneIndex::new();

        // Create paths
        let _root_path = SdfPath::absolute_root();
        let world_path = SdfPath::from_string("/World").unwrap();
        let char_path = SdfPath::from_string("/World/Character").unwrap();

        // Create resolver context
        let resolver_ctx = ResolverContext::new();
        let ctx_ds = HdRetainedTypedSampledDataSource::new(resolver_ctx);
        // Convert to trait object
        let ctx_ds_handle: HdResolverContextDataSourceHandle =
            ctx_ds as Arc<dyn HdTypedSampledDataSource<ResolverContext>>;

        // Build hdar system container with resolver context
        let hdar_container = HdarSystemSchemaBuilder::new()
            .set_resolver_context(ctx_ds_handle.clone())
            .build();

        // Wrap in system container
        let hdar_dyn: HdContainerDataSourceHandle = hdar_container;
        let system_container =
            HdRetainedContainerDataSource::new_1(ASSET_RESOLUTION.clone(), hdar_dyn.clone_box());

        // Wrap in prim container
        let system_dyn: HdContainerDataSourceHandle = system_container;
        let prim_ds = HdRetainedContainerDataSource::new_1(SYSTEM.clone(), system_dyn.clone_box());

        // Add to scene index at /World
        {
            let mut scene = scene_index.write();
            scene.add_prims(&[RetainedAddedPrimEntry::new(
                world_path.clone(),
                "".into(),
                Some(prim_ds.clone()),
            )]);

            // Add child prim without system data
            let child_ds = HdRetainedContainerDataSource::new_empty();
            scene.add_prims(&[RetainedAddedPrimEntry::new(
                char_path.clone(),
                "".into(),
                Some(child_ds),
            )]);
        }

        // Query from child - should find parent's context
        let scene_handle: HdSceneIndexHandle = scene_index.clone();
        let (found_container, found_path) =
            HdarSystemSchema::get_from_path(&scene_handle, &char_path);

        assert!(found_container.is_some(), "Should find container");
        assert_eq!(found_path, Some(world_path), "Should find at /World parent");

        // Verify schema and resolver context extraction
        if let Some(container) = found_container {
            let schema = HdarSystemSchema::new(container);
            assert!(schema.is_defined());
            // get_resolver_context should work with Arc::downcast fix
            let resolved_ctx = schema.get_resolver_context();
            assert!(
                resolved_ctx.is_some(),
                "get_resolver_context should return Some when built with ResolverContext DS"
            );
        }
    }

    #[test]
    fn test_get_from_path_not_found() {
        let scene_index = HdRetainedSceneIndex::new();
        let path = SdfPath::from_string("/NotExists").unwrap();

        let scene_handle: HdSceneIndexHandle = scene_index;
        let (container, found_path) = HdarSystemSchema::get_from_path(&scene_handle, &path);
        assert!(container.is_none());
        assert!(found_path.is_none());
    }

    #[test]
    fn test_locator_elements() {
        let locator = HdarSystemSchema::get_default_locator();
        let elements = locator.elements();

        assert_eq!(elements.len(), 2);
        assert_eq!(&elements[0], &*SYSTEM);
        assert_eq!(&elements[1], &*ASSET_RESOLUTION);
    }

    #[test]
    fn test_schema_is_empty_by_default() {
        let schema = HdarSystemSchema::default();
        assert!(!schema.is_defined());
        assert!(schema.get_container().is_none());
    }

    #[test]
    fn test_hd_system_schema_compose() {
        let scene_index = HdRetainedSceneIndex::new();
        let path = SdfPath::absolute_root();

        // Compose should return None for empty scene
        let scene_handle: HdSceneIndexHandle = scene_index;
        let (composed, found) = HdSystemSchema::compose(&scene_handle, &path);
        assert!(composed.is_none());
        assert!(found.is_none());
    }
}
