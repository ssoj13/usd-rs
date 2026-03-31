
//! Root overrides scene index - overlays xform and visibility on the root prim.
//!
//! Matches C++ UsdImagingRootOverridesSceneIndex: provides SetRootTransform /
//! SetRootVisibility that emit schema-accurate dirty locators
//! (xform.matrix, visibility.visibility).

use parking_lot::RwLock;
use std::sync::Arc;
use usd_gf::Matrix4d;
use usd_hd::data_source::{
    HdBoolDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle, HdMatrixDataSourceHandle,
    HdRetainedContainerDataSource, HdSampledDataSource, HdTypedSampledDataSource,
};
use usd_hd::scene_index::base::TfTokenVector;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, SdfPathVector, wire_filter_to_input, si_ref,
};
use usd_hd::schema::visibility as vis_schema;
use usd_hd::schema::xform as xform_schema;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet, HdOverlayContainerDataSource};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

#[derive(Clone, Copy, Debug)]
struct RootOverlayInfo {
    transform: Matrix4d,
    visibility: bool,
}

type RootOverlayInfoShared = Arc<RwLock<RootOverlayInfo>>;

#[derive(Debug)]
struct MatrixSource {
    info: RootOverlayInfoShared,
}

impl MatrixSource {
    fn new(info: RootOverlayInfoShared) -> Arc<Self> {
        Arc::new(Self { info })
    }

    fn get(&self) -> Matrix4d {
        self.info.read().transform
    }
}

impl HdDataSourceBase for MatrixSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            info: self.info.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(usd_vt::Value::from(self.get()))
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }

    fn as_matrix_data_source(&self) -> Option<HdMatrixDataSourceHandle> {
        Some(Arc::new(Self {
            info: self.info.clone(),
        }) as HdMatrixDataSourceHandle)
    }
}

impl HdSampledDataSource for MatrixSource {
    fn get_value(&self, _shutter_offset: f32) -> usd_vt::Value {
        usd_vt::Value::from(self.get())
    }

    fn get_contributing_sample_times(&self, _start: f32, _end: f32, _out: &mut Vec<f32>) -> bool {
        false
    }
}

impl HdTypedSampledDataSource<Matrix4d> for MatrixSource {
    fn get_typed_value(&self, _shutter_offset: f32) -> Matrix4d {
        self.get()
    }
}

#[derive(Debug)]
struct VisibilitySource {
    info: RootOverlayInfoShared,
}

impl VisibilitySource {
    fn new(info: RootOverlayInfoShared) -> Arc<Self> {
        Arc::new(Self { info })
    }

    fn get(&self) -> bool {
        self.info.read().visibility
    }
}

impl HdDataSourceBase for VisibilitySource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            info: self.info.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(usd_vt::Value::from(self.get()))
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for VisibilitySource {
    fn get_value(&self, _shutter_offset: f32) -> usd_vt::Value {
        usd_vt::Value::from(self.get())
    }

    fn get_contributing_sample_times(&self, _start: f32, _end: f32, _out: &mut Vec<f32>) -> bool {
        false
    }
}

impl HdTypedSampledDataSource<bool> for VisibilitySource {
    fn get_typed_value(&self, _shutter_offset: f32) -> bool {
        self.get()
    }
}

/// Scene index that overlays transform and visibility on the absolute root prim.
///
/// Mirrors C++ UsdImagingRootOverridesSceneIndex exactly:
/// - Stores a shared (transform, visibility) pair
/// - Builds a static overlay datasource with xform/matrix + visibility/visibility
/// - SetRootTransform emits dirty at locator `xform.matrix`
/// - SetRootVisibility emits dirty at locator `visibility.visibility`
pub struct HdRootOverridesSceneIndex {
    filtering_base: HdSingleInputFilteringSceneIndexBase,
    root_overlay_info: RootOverlayInfoShared,
    root_overlay_ds: HdContainerDataSourceHandle,
}

impl HdRootOverridesSceneIndex {
    /// Create a new root overrides scene index wrapping `input_scene`.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        let root_overlay_info = Arc::new(RwLock::new(RootOverlayInfo {
            transform: Matrix4d::identity(),
            visibility: true,
        }));
        let root_overlay_ds = Self::build_root_overlay_ds(root_overlay_info.clone());
        let result = Arc::new(RwLock::new(Self {
            filtering_base: HdSingleInputFilteringSceneIndexBase::new(input_scene.clone()),
            root_overlay_info,
            root_overlay_ds,
        }));
        if let Some(ref input) = input_scene {
            wire_filter_to_input(&result, input);
        }
        result
    }

    /// Set the root transform. Emits dirty at `xform.matrix` on the absolute root.
    pub fn set_root_transform(&mut self, transform: Matrix4d) {
        {
            let mut info = self.root_overlay_info.write();
            if info.transform == transform {
                return;
            }
            info.transform = transform;
        }

        let locator =
            xform_schema::HdXformSchema::get_default_locator().append(&xform_schema::MATRIX);
        let mut locators = HdDataSourceLocatorSet::empty();
        locators.insert(locator);
        let entries = vec![DirtiedPrimEntry {
            prim_path: SdfPath::absolute_root(),
            dirty_locators: locators,
        }];
        // Match `_ref`: self-originated notices must use the overlay scene index
        // as sender so downstream callbacks can query the transformed view.
        self.filtering_base
            .base()
            .send_prims_dirtied(self, &entries);
    }

    /// Get the current root transform.
    pub fn get_root_transform(&self) -> Matrix4d {
        self.root_overlay_info.read().transform
    }

    /// Set root visibility. Emits dirty at `visibility.visibility` on the absolute root.
    pub fn set_root_visibility(&mut self, visibility: bool) {
        {
            let mut info = self.root_overlay_info.write();
            if info.visibility == visibility {
                return;
            }
            info.visibility = visibility;
        }

        let locator =
            vis_schema::HdVisibilitySchema::get_default_locator().append(&vis_schema::VISIBILITY);
        let mut locators = HdDataSourceLocatorSet::empty();
        locators.insert(locator);
        let entries = vec![DirtiedPrimEntry {
            prim_path: SdfPath::absolute_root(),
            dirty_locators: locators,
        }];
        // Match `_ref`: self-originated notices must use the overlay scene index
        // as sender so downstream callbacks can query the transformed view.
        self.filtering_base
            .base()
            .send_prims_dirtied(self, &entries);
    }

    /// Get the current root visibility.
    pub fn get_root_visibility(&self) -> bool {
        self.root_overlay_info.read().visibility
    }

    fn build_root_overlay_ds(info: RootOverlayInfoShared) -> HdContainerDataSourceHandle {
        let matrix_ds = MatrixSource::new(info.clone()) as HdMatrixDataSourceHandle;
        let vis_ds = VisibilitySource::new(info) as HdBoolDataSourceHandle;

        HdRetainedContainerDataSource::new_2(
            xform_schema::XFORM.clone(),
            HdRetainedContainerDataSource::new_1(xform_schema::MATRIX.clone(), matrix_ds as _)
                as HdDataSourceBaseHandle,
            vis_schema::VISIBILITY.clone(),
            HdRetainedContainerDataSource::new_1(vis_schema::VISIBILITY.clone(), vis_ds as _)
                as HdDataSourceBaseHandle,
        ) as HdContainerDataSourceHandle
    }
}

impl HdSceneIndexBase for HdRootOverridesSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let prim = if let Some(input) = self.filtering_base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::empty()
        };

        if prim_path.is_absolute_root_path() {
            HdSceneIndexPrim {
                prim_type: prim.prim_type,
                data_source: HdOverlayContainerDataSource::overlayed(
                    Some(self.root_overlay_ds.clone()),
                    prim.data_source,
                ),
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
            "HdRootOverridesSceneIndex".to_string()
        } else {
            name.to_string()
        }
    }
}

impl FilteringObserverTarget for HdRootOverridesSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.filtering_base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.filtering_base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
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
    fn test_root_overrides_creation() {
        let scene = HdRootOverridesSceneIndex::new(None);
        let scene_lock = scene.read();
        assert_eq!(scene_lock.get_root_transform(), Matrix4d::identity());
        assert!(scene_lock.get_root_visibility());
    }

    #[test]
    fn test_set_root_transform() {
        let scene = HdRootOverridesSceneIndex::new(None);
        let mut scene_lock = scene.write();
        let m = Matrix4d::from_diagonal_values(2.0, 2.0, 2.0, 1.0);
        scene_lock.set_root_transform(m);
        assert_eq!(scene_lock.get_root_transform(), m);
    }

    #[test]
    fn test_set_root_visibility() {
        let scene = HdRootOverridesSceneIndex::new(None);
        let mut scene_lock = scene.write();
        scene_lock.set_root_visibility(false);
        assert!(!scene_lock.get_root_visibility());
    }

    #[test]
    fn test_get_prim_without_input() {
        let scene = HdRootOverridesSceneIndex::new(None);
        let scene_lock = scene.read();
        let root_prim = scene_lock.get_prim(&SdfPath::absolute_root());
        assert!(root_prim.data_source.is_some());
    }

    #[test]
    fn test_get_child_prim_paths_without_input() {
        let scene = HdRootOverridesSceneIndex::new(None);
        let scene_lock = scene.read();
        let children = scene_lock.get_child_prim_paths(&SdfPath::absolute_root());
        assert_eq!(children.len(), 0);
    }

    #[test]
    fn test_dirty_locator_xform() {
        let locator =
            xform_schema::HdXformSchema::get_default_locator().append(&xform_schema::MATRIX);
        assert_eq!(locator.len(), 2);
    }

    #[test]
    fn test_dirty_locator_visibility() {
        let locator =
            vis_schema::HdVisibilitySchema::get_default_locator().append(&vis_schema::VISIBILITY);
        assert_eq!(locator.len(), 2);
    }
}
