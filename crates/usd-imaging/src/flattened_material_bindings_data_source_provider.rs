#![allow(dead_code)]
//! Flattened material bindings data source provider.
//!
//! Port of pxr/usdImaging/usdImaging/flattenedMaterialBindingsDataSourceProvider.h
//!
//! Provides flattening for material bindings schema, aggregating bindings from
//! ancestors with prim-local bindings. Material bindings are inherited down the
//! hierarchy, with child bindings taking precedence.

use std::collections::HashSet;
use std::sync::Arc;
use usd_hd::data_source::cast_to_vector;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdFlattenedDataSourceProvider,
    HdFlattenedDataSourceProviderContext, HdRetainedSmallVectorDataSource,
    HdVectorDataSourceHandle,
};
use usd_tf::Token;

/// Data source that aggregates material bindings from parent with prim's local bindings.
///
/// Material bindings are organized by purpose (e.g., "allPurpose", "preview", "full").
/// For each purpose, we concatenate parent bindings with prim bindings.
/// The binding resolving scene index walks through bindings in this order.
#[derive(Clone)]
struct MaterialBindingsDataSource {
    /// Material bindings from current prim
    prim_bindings: HdContainerDataSourceHandle,
    /// Material bindings from parent prim (already flattened)
    parent_bindings: HdContainerDataSourceHandle,
}

impl std::fmt::Debug for MaterialBindingsDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialBindingsDataSource").finish()
    }
}

impl MaterialBindingsDataSource {
    /// Create new material bindings data source.
    fn new(
        prim_bindings: HdContainerDataSourceHandle,
        parent_bindings: HdContainerDataSourceHandle,
    ) -> Self {
        Self {
            prim_bindings,
            parent_bindings,
        }
    }

    /// Concatenate two vector data sources.
    ///
    /// Returns a vector data source with valid elements from both sources.
    fn concat(
        a: HdVectorDataSourceHandle,
        b: HdVectorDataSourceHandle,
    ) -> HdVectorDataSourceHandle {
        let num_a = a.get_num_elements();
        let num_b = b.get_num_elements();

        let mut result = Vec::new();
        result.reserve(num_a + num_b);

        // Add elements from first source
        for i in 0..num_a {
            if let Some(ds) = a.get_element(i) {
                result.push(ds);
            }
        }

        // Add elements from second source
        for i in 0..num_b {
            if let Some(ds) = b.get_element(i) {
                result.push(ds);
            }
        }

        HdRetainedSmallVectorDataSource::new(&result)
    }

    /// Create data source if needed, or use existing ones.
    ///
    /// Avoids allocation if only one source is present (common case).
    fn use_or_create_new(
        prim_bindings: Option<HdContainerDataSourceHandle>,
        parent_bindings: Option<HdContainerDataSourceHandle>,
    ) -> Option<HdContainerDataSourceHandle> {
        match (prim_bindings, parent_bindings) {
            (None, None) => None,
            (Some(prim), None) => Some(prim),
            (None, Some(parent)) => Some(parent),
            (Some(prim), Some(parent)) => {
                Some(Arc::new(Self::new(prim, parent)) as HdContainerDataSourceHandle)
            }
        }
    }
}

impl HdDataSourceBase for MaterialBindingsDataSource {
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

impl HdContainerDataSource for MaterialBindingsDataSource {
    fn get_names(&self) -> Vec<Token> {
        // Aggregate all unique purposes from both prim and parent
        let mut all_purposes = HashSet::new();

        for name in self.prim_bindings.get_names() {
            all_purposes.insert(name);
        }

        for name in self.parent_bindings.get_names() {
            all_purposes.insert(name);
        }

        all_purposes.into_iter().collect()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // The 'name' is the binding purpose (e.g., "allPurpose", "preview")

        // Get bindings for this purpose from parent (vector of bindings)
        let parent_bindings = self.parent_bindings.get(name);

        // Get bindings for this purpose from prim (vector of bindings)
        let prim_bindings = self.prim_bindings.get(name);

        match (parent_bindings, prim_bindings) {
            (None, None) => None,
            (Some(parent), None) => Some(parent),
            (None, Some(prim)) => Some(prim),
            (Some(parent), Some(prim)) => {
                // Both have bindings - concatenate parent first, then prim.
                // C++: _Concat(parentBindingsSchema.GetVector(), primBindingsSchema.GetVector())
                // Parent is prepended so the binding resolving scene index can short-circuit
                // membership evaluation by walking front-to-back.
                match (cast_to_vector(&parent), cast_to_vector(&prim)) {
                    (Some(parent_vec), Some(prim_vec)) => {
                        Some(Self::concat(parent_vec, prim_vec) as HdDataSourceBaseHandle)
                    }
                    // If one side cannot be downcast to a vector, prefer prim's opinion
                    (None, _) => Some(prim),
                    (_, None) => Some(parent),
                }
            }
        }
    }
}

/// Provider for flattened material bindings data sources.
///
/// Aggregates material bindings from parent prims with prim-local bindings.
pub struct FlattenedMaterialBindingsDataSourceProvider {}

impl FlattenedMaterialBindingsDataSourceProvider {
    /// Create new material bindings provider.
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for FlattenedMaterialBindingsDataSourceProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl HdFlattenedDataSourceProvider for FlattenedMaterialBindingsDataSourceProvider {
    fn get_flattened_data_source(
        &self,
        ctx: &HdFlattenedDataSourceProviderContext<'_>,
    ) -> Option<HdContainerDataSourceHandle> {
        MaterialBindingsDataSource::use_or_create_new(
            ctx.get_input_data_source(),
            ctx.get_flattened_data_source_from_parent_prim(),
        )
    }

    fn compute_dirty_locators_for_descendants(&self, locators: &mut HdDataSourceLocatorSet) {
        // Any locator of the form BindingPurpose:Foo:... will be turned into BindingPurpose.
        // The data source for aggregated bindings needs to be recomputed.

        let mut modify_input_locators = false;

        // Check if any locator has more than one element
        for locator in locators.iter() {
            if locator.len() != 1 {
                modify_input_locators = true;
                break;
            }
        }

        if !modify_input_locators {
            return; // Use input locators as-is
        }

        // Simplify locators to first element only
        let mut result = HdDataSourceLocatorSet::empty();
        for locator in locators.iter() {
            if let Some(first_token) = locator.get_element(0) {
                result.insert(HdDataSourceLocator::from_token(first_token.clone()));
            }
        }

        *locators = result;
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
        let provider = FlattenedMaterialBindingsDataSourceProvider::new();
        let scene = HdRetainedSceneIndex::new();
        let guard = scene.read();
        let empty_prim = HdSceneIndexPrim::empty();
        let prim_path = SdfPath::absolute_root();
        let name = Token::new("materialBindings");
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
        let result = MaterialBindingsDataSource::use_or_create_new(Some(prim_ds.clone()), None);
        assert!(result.is_some());
    }

    #[test]
    fn test_compute_dirty_locators_single_element() {
        let provider = FlattenedMaterialBindingsDataSourceProvider::new();

        let mut locators = HdDataSourceLocatorSet::from_iter([HdDataSourceLocator::from_token(
            Token::new("allPurpose"),
        )]);

        let original_count = locators.iter().count();
        provider.compute_dirty_locators_for_descendants(&mut locators);

        // Should remain unchanged (single element)
        assert_eq!(locators.iter().count(), original_count);
    }

    #[test]
    fn test_compute_dirty_locators_multi_element() {
        let provider = FlattenedMaterialBindingsDataSourceProvider::new();

        let mut locators = HdDataSourceLocatorSet::from_iter([HdDataSourceLocator::from_tokens_2(
            Token::new("allPurpose"),
            Token::new("binding"),
        )]);

        provider.compute_dirty_locators_for_descendants(&mut locators);

        // Should be simplified to first element only
        assert_eq!(locators.iter().count(), 1);
        let simplified = locators.iter().next().unwrap();
        assert_eq!(simplified.len(), 1);
    }

    #[test]
    fn test_material_bindings_names() {
        let mut prim_children = std::collections::HashMap::new();
        prim_children.insert(
            Token::new("allPurpose"),
            HdRetainedContainerDataSource::new(std::collections::HashMap::new())
                as HdDataSourceBaseHandle,
        );
        let prim_ds = HdRetainedContainerDataSource::new(prim_children);

        let mut parent_children = std::collections::HashMap::new();
        parent_children.insert(
            Token::new("preview"),
            HdRetainedContainerDataSource::new(std::collections::HashMap::new())
                as HdDataSourceBaseHandle,
        );
        let parent_ds = HdRetainedContainerDataSource::new(parent_children);

        let bindings_ds = MaterialBindingsDataSource::new(prim_ds, parent_ds);
        let names = bindings_ds.get_names();

        // Should have both purposes
        assert_eq!(names.len(), 2);
        assert!(names.iter().any(|n| n == "allPurpose"));
        assert!(names.iter().any(|n| n == "preview"));
    }

    #[test]
    fn test_concat_vectors() {
        let empty_ds = HdRetainedContainerDataSource::new(std::collections::HashMap::new())
            as HdDataSourceBaseHandle;

        let vec1 = HdRetainedSmallVectorDataSource::new(&[empty_ds.clone()]);

        let vec2 = HdRetainedSmallVectorDataSource::new(&[empty_ds.clone(), empty_ds.clone()]);

        let result = MaterialBindingsDataSource::concat(vec1, vec2);

        // Should have 3 elements total
        assert_eq!(result.get_num_elements(), 3);
    }
}
