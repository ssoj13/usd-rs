//! Dome light camera visibility scene index.
//!
//! Port of pxr/imaging/hdsi/domeLightCameraVisibilitySceneIndex.
//!
//! Overrides the cameraVisibility at light:cameraVisibility for dome light prims.

use parking_lot::RwLock;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use usd_hd::data_source::{
    HdDataSourceBaseHandle, HdDataSourceLocator, HdDataSourceLocatorSet,
    HdOverlayContainerDataSource, HdRetainedContainerDataSource,
};
use usd_hd::scene_index::base::SceneIndexDelegate;
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

const DOME_LIGHT: &str = "domeLight";
const LIGHT_SCHEMA: &str = "light";
const CAMERA_VISIBILITY: &str = "cameraVisibility";

fn light_camera_visibility_locator() -> HdDataSourceLocator {
    HdDataSourceLocator::new(&[TfToken::new(LIGHT_SCHEMA), TfToken::new(CAMERA_VISIBILITY)])
}

/// Mutable bool data source for camera visibility.
struct CameraVisibilityDataSource {
    value: RwLock<bool>,
}

impl CameraVisibilityDataSource {
    fn new(visible: bool) -> Arc<Self> {
        Arc::new(Self {
            value: RwLock::new(visible),
        })
    }

    fn get(&self) -> bool {
        *self.value.read()
    }

    fn set(&self, v: bool) {
        *self.value.write() = v;
    }
}

impl std::fmt::Debug for CameraVisibilityDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraVisibilityDataSource")
            .field("value", &self.get())
            .finish()
    }
}

impl usd_hd::data_source::HdDataSourceBase for CameraVisibilityDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            value: RwLock::new(self.get()),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(usd_vt::Value::from(self.get()))
    }
}

impl usd_hd::data_source::HdSampledDataSource for CameraVisibilityDataSource {
    fn get_value(&self, _shutter_offset: f32) -> usd_vt::Value {
        usd_vt::Value::from(self.get())
    }

    fn get_contributing_sample_times(&self, _start: f32, _end: f32, _out: &mut Vec<f32>) -> bool {
        false
    }
}

impl usd_hd::data_source::HdTypedSampledDataSource<bool> for CameraVisibilityDataSource {
    fn get_typed_value(&self, _shutter_offset: f32) -> bool {
        self.get()
    }
}

/// Scene index that overrides cameraVisibility for dome lights.
pub struct HdsiDomeLightCameraVisibilitySceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    camera_visibility_data_source: Arc<CameraVisibilityDataSource>,
    dome_light_paths: Mutex<BTreeSet<SdfPath>>,
}

impl HdsiDomeLightCameraVisibilitySceneIndex {
    /// Creates a new dome light camera visibility scene index.
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            camera_visibility_data_source: CameraVisibilityDataSource::new(true),
            dome_light_paths: Mutex::new(BTreeSet::new()),
        }));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene
                .read()
                .add_observer(Arc::new(filtering_observer));
        }
        observer
    }

    /// Sets the camera visibility for all dome lights and notifies observers.
    pub fn set_dome_light_camera_visibility(this: &Arc<RwLock<Self>>, visibility: bool) {
        let (observed, paths) = {
            let guard = this.read();
            let ds = &guard.camera_visibility_data_source;
            if ds.get() == visibility {
                return;
            }
            ds.set(visibility);
            let observed = guard.base.base().is_observed();
            let paths: Vec<SdfPath> = guard
                .dome_light_paths
                .lock()
                .expect("Lock poisoned")
                .iter()
                .cloned()
                .collect();
            (observed, paths)
        };

        if !observed || paths.is_empty() {
            return;
        }

        let mut set = HdDataSourceLocatorSet::new();
        set.insert(light_camera_visibility_locator());
        let entries: Vec<DirtiedPrimEntry> = paths
            .into_iter()
            .map(|p| DirtiedPrimEntry::new(p, set.clone()))
            .collect();

        let guard = this.read();
        let delegate = SceneIndexDelegate(Arc::clone(this));
        let sender = &delegate as &dyn HdSceneIndexBase;
        guard.base.base().send_prims_dirtied(sender, &entries);
    }
}

impl HdSceneIndexBase for HdsiDomeLightCameraVisibilitySceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let mut prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::default()
        };

        if prim.prim_type == DOME_LIGHT {
            let light_container = HdRetainedContainerDataSource::new_1(
                TfToken::new(CAMERA_VISIBILITY),
                self.camera_visibility_data_source.clone() as HdDataSourceBaseHandle,
            );
            let overlay_container = HdRetainedContainerDataSource::new_1(
                TfToken::new(LIGHT_SCHEMA),
                light_container as HdDataSourceBaseHandle,
            );
            prim.data_source =
                HdOverlayContainerDataSource::overlayed(Some(overlay_container), prim.data_source);
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
        "Dome Light Camera Visibility Scene Index".to_string()
    }
}

impl FilteringObserverTarget for HdsiDomeLightCameraVisibilitySceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let mut dome_light_paths = self.dome_light_paths.lock().expect("Lock poisoned");
        for entry in entries {
            if entry.prim_type == DOME_LIGHT {
                dome_light_paths.insert(entry.prim_path.clone());
            } else {
                dome_light_paths.remove(&entry.prim_path);
            }
        }
        drop(dome_light_paths);
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut dome_light_paths = self.dome_light_paths.lock().expect("Lock poisoned");
        for entry in entries {
            let mut to_remove = Vec::new();
            for path in dome_light_paths.iter() {
                if path.has_prefix(&entry.prim_path) {
                    to_remove.push(path.clone());
                }
            }
            for p in to_remove {
                dome_light_paths.remove(&p);
            }
        }
        drop(dome_light_paths);
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
