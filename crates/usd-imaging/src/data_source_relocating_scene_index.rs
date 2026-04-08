#![allow(dead_code)]

//! Data source relocating scene index.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceRelocatingSceneIndex.h/cpp
//!
//! Filters scene index by relocating data from one locator to another.
//! Used by native instance aggregation to move skelBinding:animationSource
//! to primvars:skel:animationSource for deferred skinning.

use crate::usd_prim_info_schema::UsdPrimInfoSchema;
use parking_lot::RwLock;
use std::sync::Arc;
use usd_hd::data_source::{
    HdBlockDataSource, HdDataSourceBaseHandle, HdDataSourceLocator, HdDataSourceLocatorSet,
    HdOverlayContainerDataSource, HdRetainedContainerDataSource, HdSampledDataSource,
    hd_container_get,
};
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdContainerDataSourceHandle, HdSceneIndexBase, HdSceneIndexHandle,
    HdSceneIndexObserverHandle, HdSceneIndexPrim, HdSingleInputFilteringSceneIndexBase,
    SdfPathVector, si_ref, wire_filter_to_input,
};
use usd_sdf::Path as SdfPath;

/// Data source relocating scene index.
///
/// Relocates data from src_locator to dst_locator for prims matching
/// for_native_instance (native instances vs non-instances).
pub struct UsdImagingDataSourceRelocatingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    src_locator: HdDataSourceLocator,
    dst_locator: HdDataSourceLocator,
    dirty_dst_locators: HdDataSourceLocatorSet,
    for_native_instance: bool,
}

impl UsdImagingDataSourceRelocatingSceneIndex {
    /// Creates a new relocating scene index.
    ///
    /// # Arguments
    ///
    /// * `input_scene` - Input scene index
    /// * `src_locator` - Source locator to move from
    /// * `dst_locator` - Destination locator to move to
    /// * `for_native_instance` - If true, relocate only for native instances; if false, only for non-instances
    pub fn new(
        input_scene: HdSceneIndexHandle,
        src_locator: HdDataSourceLocator,
        dst_locator: HdDataSourceLocator,
        for_native_instance: bool,
    ) -> Arc<RwLock<Self>> {
        let mut dirty_dst = HdDataSourceLocatorSet::new();
        dirty_dst.insert(dst_locator.clone());

        let result = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            src_locator,
            dst_locator,
            dirty_dst_locators: dirty_dst,
            for_native_instance,
        }));
        wire_filter_to_input(&result, &input_scene);
        result
    }

    fn is_native_instance(prim: &HdSceneIndexPrim) -> bool {
        let Some(ref ds) = prim.data_source else {
            return false;
        };
        let ni_path_loc = UsdPrimInfoSchema::get_ni_prototype_path_locator();
        let Some(path_ds) = hd_container_get(ds.clone(), &ni_path_loc) else {
            return false;
        };
        // Try to sample path - path_ds may be HdSampledDataSource
        if let Some(sampled) = path_ds
            .as_any()
            .downcast_ref::<Arc<dyn HdSampledDataSource>>()
        {
            let v = sampled.get_value(0.0);
            if let Some(path) = v.get::<SdfPath>() {
                return !path.is_empty();
            }
        }
        false
    }

    fn build_nested_container(
        locator: &HdDataSourceLocator,
        value: HdDataSourceBaseHandle,
    ) -> HdContainerDataSourceHandle {
        let els = locator.elements();
        if els.is_empty() {
            if let Some(cont) = usd_hd::data_source::cast_to_container(&value) {
                return cont;
            }
            return HdRetainedContainerDataSource::new_empty();
        }
        let mut current: HdDataSourceBaseHandle = value;
        for name in els.iter().rev() {
            let mut children = std::collections::HashMap::new();
            children.insert(name.clone(), current);
            let container = HdRetainedContainerDataSource::new(children);
            current = container as HdDataSourceBaseHandle;
        }
        usd_hd::data_source::cast_to_container(&current).unwrap_or_else(|| {
            HdRetainedContainerDataSource::new_empty() as HdContainerDataSourceHandle
        })
    }

    fn relocate_data_source(
        prim: &mut HdSceneIndexPrim,
        src_locator: &HdDataSourceLocator,
        dst_locator: &HdDataSourceLocator,
    ) {
        let Some(ref ds) = prim.data_source else {
            return;
        };
        let Some(src) = hd_container_get(ds.clone(), src_locator) else {
            return;
        };

        let block_patch = Self::build_nested_container(
            src_locator,
            HdBlockDataSource::new() as HdDataSourceBaseHandle,
        );
        let dst_patch = Self::build_nested_container(dst_locator, src);

        let overlay = HdOverlayContainerDataSource::new_3(dst_patch, block_patch, ds.clone());
        prim.data_source = Some(overlay);
    }
}

// Implement HdFilteringSceneIndexBase via HdSceneIndexBase
impl HdSceneIndexBase for UsdImagingDataSourceRelocatingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let input = match self.base.get_input_scene() {
            Some(scene) => scene,
            None => return HdSceneIndexPrim::default(),
        };
        let mut prim = si_ref(&input).get_prim(prim_path);
        if prim.data_source.is_some()
            && (self.for_native_instance == Self::is_native_instance(&prim))
        {
            Self::relocate_data_source(&mut prim, &self.src_locator, &self.dst_locator);
        }
        prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_child_prim_paths(prim_path);
        }
        vec![]
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }
}

impl FilteringObserverTarget for UsdImagingDataSourceRelocatingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let mut new_entries = Vec::with_capacity(entries.len());
        for entry in entries {
            if entry.dirty_locators.intersects_locator(&self.src_locator) {
                new_entries.push(DirtiedPrimEntry::new(
                    entry.prim_path.clone(),
                    self.dirty_dst_locators.clone(),
                ));
            } else {
                new_entries.push(entry.clone());
            }
        }
        self.base.forward_prims_dirtied(self, &new_entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
