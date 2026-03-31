#![allow(dead_code)]
//! Native instance prototype scene index.
//!
//! Port of pxr/usdImaging/usdImaging/niPrototypeSceneIndex.cpp
//!
//! This scene index handles native instancing by:
//!   - For USD instances (prims with niPrototypePath): clears the prim type
//!     for renderable prims so they are not drawn directly.
//!   - For native prototype mode: overlays `instancedBy` data source on
//!     descendants of the prototype root, and at the prototype root itself
//!     additionally overlays identity xform with resetXformStack=true.
//!   - In _PrimsAdded: scans for USD instances with renderable prim types
//!     and erases those types in the forwarded notice.

use crate::prototype_scene_index_utils::is_renderable_prim_type;
use once_cell::sync::Lazy;
use std::sync::Arc;
use parking_lot::RwLock;
use usd_gf::Matrix4d;
use usd_hd::data_source::{
    HdDataSourceBaseHandle, HdOverlayContainerDataSource, HdRetainedContainerDataSource,
    HdRetainedTypedSampledDataSource, HdTypedSampledDataSource,
};
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdContainerDataSourceHandle, HdSceneIndexBase, HdSceneIndexHandle,
    HdSceneIndexPrim, HdSingleInputFilteringSceneIndexBase, SdfPathVector, wire_filter_to_input,
    si_ref,
};
use usd_hd::schema::HdInstancedBySchema;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

// -- tokens ------------------------------------------------------------------

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static INSTANCER: LazyLock<Token> = LazyLock::new(|| Token::new("UsdNiInstancer"));
    pub static PROTOTYPE: LazyLock<Token> = LazyLock::new(|| Token::new("UsdNiPrototype"));
    pub static INSTANCED_BY: LazyLock<Token> = LazyLock::new(|| Token::new("instancedBy"));
    pub static XFORM: LazyLock<Token> = LazyLock::new(|| Token::new("xform"));
    pub static MATRIX: LazyLock<Token> = LazyLock::new(|| Token::new("matrix"));
    pub static RESET_XFORM_STACK: LazyLock<Token> = LazyLock::new(|| Token::new("resetXformStack"));
}

// -- static helpers ----------------------------------------------------------

/// Build identity xform with resetXformStack=true.
fn reset_xform_to_identity() -> HdContainerDataSourceHandle {
    let matrix_ds: HdDataSourceBaseHandle =
        HdRetainedTypedSampledDataSource::new(Matrix4d::identity());
    let reset_ds: HdDataSourceBaseHandle = HdRetainedTypedSampledDataSource::new(true);

    HdRetainedContainerDataSource::new_2(
        tokens::MATRIX.clone(),
        matrix_ds,
        tokens::RESET_XFORM_STACK.clone(),
        reset_ds,
    )
}

/// Underlay source for descendants of prototype root.
/// Contains `instancedBy` pointing to the instancer/prototype paths.
static UNDERLAY_SOURCE: Lazy<HdContainerDataSourceHandle> = Lazy::new(|| {
    let instanced_by_ds = UsdImagingNiPrototypeSceneIndex::get_instanced_by_data_source();
    match instanced_by_ds {
        Some(ds) => HdRetainedContainerDataSource::new_1(
            tokens::INSTANCED_BY.clone(),
            ds as HdDataSourceBaseHandle,
        ) as HdContainerDataSourceHandle,
        None => HdRetainedContainerDataSource::new_empty() as HdContainerDataSourceHandle,
    }
});

/// Check if a prim is a USD instance (has niPrototypePath set).
fn is_usd_instance(prim_source: &HdContainerDataSourceHandle) -> bool {
    // Read __usdPrimInfo.niPrototypePath
    let usd_prim_info_token = TfToken::new("__usdPrimInfo");
    let ni_prototype_path_token = TfToken::new("niPrototypePath");

    let info_ds = match prim_source.get(&usd_prim_info_token) {
        Some(ds) => ds,
        None => return false,
    };

    let info_container = match usd_hd::data_source::cast_to_container(&info_ds) {
        Some(c) => c,
        None => return false,
    };

    let path_ds = match info_container.get(&ni_prototype_path_token) {
        Some(ds) => ds,
        None => return false,
    };

    // Check if the path is non-empty
    let any = path_ds.as_any();
    if let Some(typed) = any.downcast_ref::<HdRetainedTypedSampledDataSource<SdfPath>>() {
        return !typed.get_typed_value(0.0).is_empty();
    }

    false
}

// -- UsdImagingNiPrototypeSceneIndex -----------------------------------------

/// Native instance prototype scene index.
///
/// Manages prototype prims for native instancing:
///   - Erases prim type for renderable USD instances (so they are not drawn)
///   - For the prototype root: overlays `instancedBy` + identity xform (resetXformStack)
///   - For prototype descendants: overlays `instancedBy`
pub struct UsdImagingNiPrototypeSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Whether this scene index is for a native prototype subtree.
    for_native_prototype: bool,
    /// Overlay data source for the prototype root (instancedBy + identity xform).
    prototype_root_overlay: HdContainerDataSourceHandle,
}

impl UsdImagingNiPrototypeSceneIndex {
    /// Path of the instancer: /UsdNiInstancer
    pub fn get_instancer_path() -> SdfPath {
        SdfPath::absolute_root()
            .append_child(tokens::INSTANCER.as_str())
            .expect("UsdNiInstancer is valid child")
    }

    /// Path of the prototype: /UsdNiInstancer/UsdNiPrototype
    pub fn get_prototype_path() -> SdfPath {
        Self::get_instancer_path()
            .append_child(tokens::PROTOTYPE.as_str())
            .expect("UsdNiPrototype is valid child")
    }

    /// Build the instancedBy data source (paths=[instancer], prototypeRoots=[prototype]).
    pub fn get_instanced_by_data_source() -> Option<HdContainerDataSourceHandle> {
        let paths_ds = HdRetainedTypedSampledDataSource::new(vec![Self::get_instancer_path()]);
        let roots_ds = HdRetainedTypedSampledDataSource::new(vec![Self::get_prototype_path()]);
        Some(HdInstancedBySchema::build_retained(
            Some(Arc::clone(&paths_ds) as _),
            Some(Arc::clone(&roots_ds) as _),
        ))
    }

    /// Build the prototype root overlay: instancedBy + identity xform with resetXformStack.
    fn build_prototype_root_overlay(
        user_overlay: Option<HdContainerDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        let instanced_by_ds = Self::get_instanced_by_data_source();
        let xform_ds = reset_xform_to_identity();

        let mut entries = Vec::new();
        entries.push((
            tokens::INSTANCED_BY.clone(),
            instanced_by_ds
                .map(|ds| ds as HdDataSourceBaseHandle)
                .unwrap_or_else(|| {
                    HdRetainedContainerDataSource::new_empty() as HdDataSourceBaseHandle
                }),
        ));
        entries.push((tokens::XFORM.clone(), xform_ds as HdDataSourceBaseHandle));

        let static_overlay =
            HdRetainedContainerDataSource::from_entries(&entries) as HdContainerDataSourceHandle;

        match user_overlay {
            Some(user_ds) => HdOverlayContainerDataSource::new_2(static_overlay, user_ds)
                as HdContainerDataSourceHandle,
            None => static_overlay,
        }
    }

    /// Creates a new native instance prototype scene index.
    ///
    /// # Arguments
    /// * `input_scene` - input scene index
    /// * `for_native_prototype` - true if this manages a native prototype subtree
    /// * `prototype_root_overlay` - additional overlay for the prototype root
    pub fn new(
        input_scene: HdSceneIndexHandle,
        for_native_prototype: bool,
        prototype_root_overlay: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let overlay = Self::build_prototype_root_overlay(prototype_root_overlay);
        let result = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            for_native_prototype,
            prototype_root_overlay: overlay,
        }));
        wire_filter_to_input(&result, &input_scene);
        result
    }
}

impl HdSceneIndexBase for UsdImagingNiPrototypeSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => return HdSceneIndexPrim::default(),
        };
        let input_locked = input.read();

        let mut prim = input_locked.get_prim(prim_path);
        let ds = match prim.data_source {
            Some(ref ds) => ds.clone(),
            None => return prim,
        };

        // For USD instances with renderable type -> clear type
        if is_usd_instance(&ds) {
            if is_renderable_prim_type(&prim.prim_type) {
                prim.prim_type = TfToken::new("");
            }
            return prim;
        }

        // Only do prototype overlays if for_native_prototype
        if !self.for_native_prototype {
            return prim;
        }

        let prototype_path = Self::get_prototype_path();
        if !prim_path.has_prefix(&prototype_path) {
            return prim;
        }

        let proto_depth = prototype_path.get_path_element_count();

        if prim_path.get_path_element_count() == proto_depth {
            // Prim IS the prototype root -> overlay with instancedBy + identity xform
            prim.data_source = Some(HdOverlayContainerDataSource::new_2(
                self.prototype_root_overlay.clone(),
                ds,
            ) as HdContainerDataSourceHandle);
        } else {
            // Prim is a descendant -> underlay instancedBy
            prim.data_source = Some(
                HdOverlayContainerDataSource::new_2(ds, UNDERLAY_SOURCE.clone())
                    as HdContainerDataSourceHandle,
            );
        }

        prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_child_prim_paths(prim_path);
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "UsdImagingNiPrototypeSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for UsdImagingNiPrototypeSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        // Scan for USD instances with renderable types and erase their type.
        let mut indices_to_erase = Vec::new();

        if let Some(input) = self.base.get_input_scene() {
            let input_locked = input.read();
            for (i, entry) in entries.iter().enumerate() {
                if is_renderable_prim_type(&entry.prim_type) {
                    let prim = input_locked.get_prim(&entry.prim_path);
                    if let Some(ref ds) = prim.data_source {
                        if is_usd_instance(ds) {
                            indices_to_erase.push(i);
                        }
                    }
                }
            }
        }

        if indices_to_erase.is_empty() {
            self.base.forward_prims_added(self, entries);
        } else {
            let mut new_entries: Vec<AddedPrimEntry> = entries.to_vec();
            for &index in &indices_to_erase {
                new_entries[index].prim_type = TfToken::new("");
            }
            self.base.forward_prims_added(self, &new_entries);
        }
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instancer_path() {
        let path = UsdImagingNiPrototypeSceneIndex::get_instancer_path();
        assert_eq!(path.get_text(), "/UsdNiInstancer");
    }

    #[test]
    fn test_prototype_path() {
        let path = UsdImagingNiPrototypeSceneIndex::get_prototype_path();
        assert_eq!(path.get_text(), "/UsdNiInstancer/UsdNiPrototype");
    }

    #[test]
    fn test_instanced_by_data_source() {
        let ds = UsdImagingNiPrototypeSceneIndex::get_instanced_by_data_source();
        assert!(ds.is_some());
    }

    #[test]
    fn test_prototype_root_overlay_has_xform_and_instanced_by() {
        let overlay = UsdImagingNiPrototypeSceneIndex::build_prototype_root_overlay(None);
        let names = overlay.get_names();
        assert!(
            names.iter().any(|n| n == "instancedBy"),
            "overlay should contain instancedBy"
        );
        assert!(
            names.iter().any(|n| n == "xform"),
            "overlay should contain xform"
        );
    }

    #[test]
    fn test_prototype_path_detection() {
        let proto_path = UsdImagingNiPrototypeSceneIndex::get_prototype_path();
        let child = proto_path.append_child("Mesh").expect("valid child");
        assert!(child.has_prefix(&proto_path));
    }

    #[test]
    fn test_display_name() {
        let si = UsdImagingNiPrototypeSceneIndex {
            base: HdSingleInputFilteringSceneIndexBase::new(None),
            for_native_prototype: false,
            prototype_root_overlay: HdRetainedContainerDataSource::new_empty()
                as HdContainerDataSourceHandle,
        };
        assert_eq!(si.get_display_name(), "UsdImagingNiPrototypeSceneIndex");
    }
}
