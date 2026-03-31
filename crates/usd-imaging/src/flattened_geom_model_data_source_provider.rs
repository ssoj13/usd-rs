#![allow(dead_code)]
//! Flattened geom model data source provider.
//!
//! Port of pxr/usdImaging/usdImaging/flattenedGeomModelDataSourceProvider.h
//!
//! Provides flattening for geom model schema, particularly for draw mode inheritance.
//! Draw modes can be set to "inherited" to inherit from parent prims.

use std::sync::{Arc, LazyLock};
use usd_hd::data_source::HdRetainedTypedSampledDataSource;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdFlattenedDataSourceProvider,
    HdFlattenedDataSourceProviderContext, HdTypedSampledDataSource,
};
use usd_tf::Token;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// Draw mode token
    pub static DRAW_MODE: LazyLock<Token> = LazyLock::new(|| Token::new("drawMode"));

    /// Inherited token (value for inherited draw mode)
    pub static INHERITED: LazyLock<Token> = LazyLock::new(|| Token::new("inherited"));
}

/// Check if token vector contains draw mode.
fn contains_draw_mode(names: &[Token]) -> bool {
    names.iter().any(|t| *t == *tokens::DRAW_MODE)
}

/// Data source that aggregates model data from prim and parent.
///
/// This handles draw mode inheritance: if prim's draw mode is "inherited" or empty,
/// use parent's draw mode instead.
#[derive(Clone)]
struct ModelDataSource {
    /// Model data source from current prim
    prim_model: HdContainerDataSourceHandle,
    /// Model data source from parent prim (already flattened)
    parent_model: HdContainerDataSourceHandle,
}

impl std::fmt::Debug for ModelDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelDataSource").finish()
    }
}

impl ModelDataSource {
    /// Create new model data source that aggregates prim and parent data.
    fn new(
        prim_model: HdContainerDataSourceHandle,
        parent_model: HdContainerDataSourceHandle,
    ) -> Self {
        Self {
            prim_model,
            parent_model,
        }
    }

    /// Create data source if needed, or use existing ones.
    ///
    /// Avoids allocation if only one source is present (common case).
    fn use_or_create_new(
        prim_model: Option<HdContainerDataSourceHandle>,
        parent_model: Option<HdContainerDataSourceHandle>,
    ) -> Option<HdContainerDataSourceHandle> {
        match (prim_model, parent_model) {
            (None, None) => None,
            (Some(prim), None) => Some(prim),
            (None, Some(parent)) => Some(parent),
            (Some(prim), Some(parent)) => {
                Some(Arc::new(Self::new(prim, parent)) as HdContainerDataSourceHandle)
            }
        }
    }
}

impl HdDataSourceBase for ModelDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for ModelDataSource {
    fn get_names(&self) -> Vec<Token> {
        let mut result = self.prim_model.get_names();

        // If prim doesn't have draw mode but parent does, add it to names
        if !contains_draw_mode(&result) && contains_draw_mode(&self.parent_model.get_names()) {
            result.push(tokens::DRAW_MODE.clone());
        }

        result
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // For non-draw-mode properties, always use prim's value
        if *name != *tokens::DRAW_MODE {
            return self.prim_model.get(name);
        }

        // For draw mode, check if prim has a non-"inherited" / non-empty value.
        // C++: UsdImagingGeomModelSchema(_primModel).GetDrawMode() -> typed value check.
        if let Some(prim_draw_mode_ds) = self.prim_model.get(name) {
            // Downcast to HdRetainedTypedSampledDataSource<Token> and check the value.
            let is_inherited_or_empty = prim_draw_mode_ds
                .as_any()
                .downcast_ref::<HdRetainedTypedSampledDataSource<usd_tf::Token>>()
                .map(|typed| {
                    let val = typed.get_typed_value(0.0);
                    val.is_empty() || val == *tokens::INHERITED
                })
                .unwrap_or(false); // Unknown type: treat as valid (non-inherited)

            if !is_inherited_or_empty {
                return Some(prim_draw_mode_ds);
            }
        }

        // Otherwise, inherit from parent
        self.parent_model.get(name)
    }
}

/// Provider for flattened geom model data sources.
///
/// Handles inheritance of draw mode from parent prims.
pub struct FlattenedGeomModelDataSourceProvider {}

impl FlattenedGeomModelDataSourceProvider {
    /// Create new geom model provider.
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for FlattenedGeomModelDataSourceProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl HdFlattenedDataSourceProvider for FlattenedGeomModelDataSourceProvider {
    fn get_flattened_data_source(
        &self,
        ctx: &HdFlattenedDataSourceProviderContext<'_>,
    ) -> Option<HdContainerDataSourceHandle> {
        ModelDataSource::use_or_create_new(
            ctx.get_input_data_source(),
            ctx.get_flattened_data_source_from_parent_prim(),
        )
    }

    fn compute_dirty_locators_for_descendants(&self, locators: &mut HdDataSourceLocatorSet) {
        static DRAW_MODE_LOCATOR: LazyLock<HdDataSourceLocator> =
            LazyLock::new(|| HdDataSourceLocator::from_token(tokens::DRAW_MODE.clone()));

        static DRAW_MODE_LOCATOR_SET: LazyLock<HdDataSourceLocatorSet> =
            LazyLock::new(|| HdDataSourceLocatorSet::from_iter([DRAW_MODE_LOCATOR.clone()]));

        // Only draw mode is inherited by descendants
        if locators.contains(&DRAW_MODE_LOCATOR) {
            *locators = DRAW_MODE_LOCATOR_SET.clone();
        } else {
            *locators = HdDataSourceLocatorSet::empty();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::HdRetainedContainerDataSource;
    use usd_hd::scene_index::{HdRetainedSceneIndex, HdSceneIndexPrim};
    use usd_sdf::Path as SdfPath;

    #[test]
    fn test_provider_creation() {
        let provider = FlattenedGeomModelDataSourceProvider::new();
        let scene = HdRetainedSceneIndex::new();
        let guard = scene.read();
        let empty_prim = HdSceneIndexPrim::empty();
        let prim_path = SdfPath::absolute_root();
        let name = Token::new("model");
        let ctx = HdFlattenedDataSourceProviderContext {
            flattening_scene_index: &*guard,
            flattening_scene_index_weak: std::sync::Weak::new(),
            prim_path: &prim_path,
            name: &name,
            input_prim: &empty_prim,
        };
        let result = provider.get_flattened_data_source(&ctx);
        assert!(result.is_none());
    }

    #[test]
    fn test_use_or_create_new_single_source() {
        let prim_ds = HdRetainedContainerDataSource::new(std::collections::HashMap::new());

        // With only prim source, should return it directly
        let result = ModelDataSource::use_or_create_new(Some(prim_ds.clone()), None);
        assert!(result.is_some());
    }

    #[test]
    fn test_compute_dirty_locators_draw_mode() {
        let provider = FlattenedGeomModelDataSourceProvider::new();

        let mut locators = HdDataSourceLocatorSet::from_iter([
            HdDataSourceLocator::from_token(tokens::DRAW_MODE.clone()),
            HdDataSourceLocator::from_token(Token::new("other")),
        ]);

        provider.compute_dirty_locators_for_descendants(&mut locators);

        // Should only keep draw mode locator
        assert!(locators.contains(&HdDataSourceLocator::from_token(tokens::DRAW_MODE.clone())));
    }

    #[test]
    fn test_compute_dirty_locators_no_draw_mode() {
        let provider = FlattenedGeomModelDataSourceProvider::new();

        let mut locators = HdDataSourceLocatorSet::from_iter([HdDataSourceLocator::from_token(
            Token::new("other"),
        )]);

        provider.compute_dirty_locators_for_descendants(&mut locators);

        // Should be empty (no inherited locators)
        assert!(locators.is_empty());
    }

    #[test]
    fn test_model_data_source_names() {
        // Simple test - just verify we can create model data source
        let mut prim_children = std::collections::HashMap::new();
        prim_children.insert(
            Token::new("foo"),
            HdRetainedContainerDataSource::new(std::collections::HashMap::new())
                as HdDataSourceBaseHandle,
        );
        let prim_ds = HdRetainedContainerDataSource::new(prim_children);

        let mut parent_children = std::collections::HashMap::new();
        parent_children.insert(
            tokens::DRAW_MODE.clone(),
            HdRetainedContainerDataSource::new(std::collections::HashMap::new())
                as HdDataSourceBaseHandle,
        );
        let parent_ds = HdRetainedContainerDataSource::new(parent_children);

        let model_ds = ModelDataSource::new(prim_ds, parent_ds);
        let names = model_ds.get_names();

        // Should have at least one name
        assert!(!names.is_empty());
    }
}
