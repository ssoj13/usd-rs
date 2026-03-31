
//! Unloaded draw mode scene index - forces bounds draw mode on unloaded prims.
//!
//! Matches C++ UsdImagingUnloadedDrawModeSceneIndex exactly: a stateless
//! pass-through filter that checks each prim's UsdPrimInfo.isLoaded flag
//! and overlays geomModel { applyDrawMode=true, drawMode="bounds" } when unloaded.

use parking_lot::RwLock;
use std::sync::{Arc, LazyLock};
use usd_hd::data_source::cast_to_container;
use usd_hd::scene_index::base::TfTokenVector;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, SdfPathVector, wire_filter_to_input, si_ref,
};
use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdOverlayContainerDataSource,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

// Tokens matching C++ UsdImagingUsdPrimInfoSchema / UsdImagingGeomModelSchema
mod tokens {
    use super::*;

    pub static BOUNDS: LazyLock<TfToken> = LazyLock::new(|| TfToken::new("bounds"));
    pub static GEOM_MODEL: LazyLock<TfToken> = LazyLock::new(|| TfToken::new("geomModel"));
    pub static APPLY_DRAW_MODE: LazyLock<TfToken> = LazyLock::new(|| TfToken::new("applyDrawMode"));
    pub static DRAW_MODE: LazyLock<TfToken> = LazyLock::new(|| TfToken::new("drawMode"));
    pub static USD_PRIM_INFO: LazyLock<TfToken> = LazyLock::new(|| TfToken::new("__usdPrimInfo"));
    pub static IS_LOADED: LazyLock<TfToken> = LazyLock::new(|| TfToken::new("isLoaded"));
}

/// Check if a prim is loaded by inspecting __usdPrimInfo.isLoaded.
/// Returns true (loaded) if the field is absent — matches C++ _IsPrimLoaded.
fn is_prim_loaded(prim_source: &Option<HdContainerDataSourceHandle>) -> bool {
    let Some(prim_source) = prim_source else {
        return true;
    };
    let Some(usd_prim_info_base) = prim_source.get(&tokens::USD_PRIM_INFO) else {
        return true;
    };
    let Some(usd_prim_info) = cast_to_container(&usd_prim_info_base) else {
        return true;
    };
    let Some(is_loaded_base) = usd_prim_info.get(&tokens::IS_LOADED) else {
        return true;
    };
    let Some(sampled) = is_loaded_base.as_sampled() else {
        return true;
    };
    sampled
        .get_value(0.0)
        .get::<bool>()
        .copied()
        .unwrap_or(true)
}

/// Cached data source that forces draw mode to "bounds".
/// Matches C++ _DataSourceForcingBoundsDrawMode().
fn data_source_forcing_bounds_draw_mode() -> HdContainerDataSourceHandle {
    static DS: LazyLock<HdContainerDataSourceHandle> = LazyLock::new(|| {
        let geom_model = HdRetainedContainerDataSource::from_entries(&[
            (
                tokens::APPLY_DRAW_MODE.clone(),
                HdRetainedTypedSampledDataSource::new(true) as HdDataSourceBaseHandle,
            ),
            (
                tokens::DRAW_MODE.clone(),
                HdRetainedTypedSampledDataSource::new(tokens::BOUNDS.clone())
                    as HdDataSourceBaseHandle,
            ),
        ]);
        HdRetainedContainerDataSource::new_1(
            tokens::GEOM_MODEL.clone(),
            geom_model as HdDataSourceBaseHandle,
        ) as HdContainerDataSourceHandle
    });
    DS.clone()
}

/// Stateless scene index that overlays bounds draw mode on unloaded prims.
///
/// Matches C++ UsdImagingUnloadedDrawModeSceneIndex: no mutable state,
/// purely inspects prim data source for __usdPrimInfo.isLoaded.
pub struct HdUnloadedDrawModeSceneIndex {
    filtering_base: HdSingleInputFilteringSceneIndexBase,
}

impl HdUnloadedDrawModeSceneIndex {
    /// Create a new unloaded draw mode scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        let result = Arc::new(RwLock::new(Self {
            filtering_base: HdSingleInputFilteringSceneIndexBase::new(input_scene.clone()),
        }));
        if let Some(ref input) = input_scene {
            wire_filter_to_input(&result, input);
        }
        result
    }
}

impl HdSceneIndexBase for HdUnloadedDrawModeSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let prim = if let Some(input) = self.filtering_base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::empty()
        };

        // If prim is not loaded, overlay bounds draw mode (matches C++)
        if !is_prim_loaded(&prim.data_source) {
            let forced = data_source_forcing_bounds_draw_mode();
            let data_source = match prim.data_source {
                Some(original) => Some(HdOverlayContainerDataSource::new_2(forced, original)
                    as HdContainerDataSourceHandle),
                None => Some(forced),
            };
            HdSceneIndexPrim {
                prim_type: prim.prim_type,
                data_source,
            }
        } else {
            prim
        }
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.filtering_base.get_input_scene() {
            return si_ref(&input).get_child_prim_paths(prim_path);
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.filtering_base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.filtering_base.base().remove_observer(observer);
    }

    fn set_display_name(&mut self, name: String) {
        self.filtering_base.base_mut().set_display_name(name);
    }

    fn add_tag(&mut self, tag: TfToken) {
        self.filtering_base.base_mut().add_tag(tag);
    }

    fn remove_tag(&mut self, tag: &TfToken) {
        self.filtering_base.base_mut().remove_tag(tag);
    }

    fn has_tag(&self, tag: &TfToken) -> bool {
        self.filtering_base.base().has_tag(tag)
    }

    fn get_tags(&self) -> TfTokenVector {
        self.filtering_base.base().get_tags()
    }

    fn get_display_name(&self) -> String {
        let name = self.filtering_base.base().get_display_name();
        if name.is_empty() {
            "HdUnloadedDrawModeSceneIndex".to_string()
        } else {
            name.to_string()
        }
    }
}

impl FilteringObserverTarget for HdUnloadedDrawModeSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.filtering_base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.filtering_base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        // C++ comment: "Loading/unloading prims forces a resync (prims removed
        // and added). So nothing to do here."
        self.filtering_base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.filtering_base.forward_prims_renamed(self, entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unloaded_draw_mode_creation() {
        let scene = HdUnloadedDrawModeSceneIndex::new(None);
        let _scene_lock = scene.read();
        // Stateless — just verify construction succeeds
    }

    #[test]
    fn test_get_prim_without_input() {
        let scene = HdUnloadedDrawModeSceneIndex::new(None);
        let scene_lock = scene.read();
        let prim = scene_lock.get_prim(&SdfPath::absolute_root());
        assert!(!prim.is_defined());
    }

    #[test]
    fn test_is_prim_loaded_no_source() {
        // No data source => loaded (matches C++ default true)
        assert!(is_prim_loaded(&None));
    }

    #[test]
    fn test_bounds_draw_mode_ds_cached() {
        // Verify the static data source is built and cached
        let ds1 = data_source_forcing_bounds_draw_mode();
        let ds2 = data_source_forcing_bounds_draw_mode();
        // Both should point to the same Arc (cached via LazyLock)
        assert!(Arc::ptr_eq(&ds1, &ds2));
    }

    #[test]
    fn test_bounds_draw_mode_ds_structure() {
        let ds = data_source_forcing_bounds_draw_mode();
        // Should have "geomModel" key
        let names = ds.get_names();
        assert!(names.contains(&tokens::GEOM_MODEL));
    }
}
