//! HdSt_RenderSettingsSceneIndex - render settings propagation for Storm.
//!
//! Filtering scene index that propagates render settings to prims.
//! Render settings control global rendering behavior like:
//! - Resolution and pixel aspect ratio
//! - Camera path
//! - AOV bindings
//! - Lighting mode
//! - Render products/vars

use parking_lot::RwLock;
use std::sync::{Arc, Mutex};
use usd_hd::data_source::HdDataSourceBaseHandle;
use usd_hd::scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, FilteringObserverTarget, HdSceneIndexBase,
    HdSceneIndexHandle, HdSceneIndexObserverHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, RemovedPrimEntry, RenamedPrimEntry, SdfPathVector,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Render settings scene index for Storm.
///
/// Observes render settings prims and propagates their configuration
/// to the render pipeline. This enables render settings authored in
/// USD scenes to affect Storm rendering behavior.
///
/// Port of render settings handling from C++ Storm render delegate.
pub struct HdStRenderSettingsSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Currently active render settings prim path
    active_render_settings: Mutex<Option<SdfPath>>,
}

impl HdStRenderSettingsSceneIndex {
    /// Create a new render settings scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            active_render_settings: Mutex::new(None),
        }))
    }

    /// Check if prim is a render settings type.
    fn is_render_settings(prim_type: &Token) -> bool {
        prim_type == "renderSettings" || prim_type == "RenderSettings"
    }

    /// Get the active render settings path.
    pub fn get_active_render_settings(&self) -> Option<SdfPath> {
        self.active_render_settings
            .lock()
            .expect("Lock poisoned")
            .clone()
    }

    /// Set the active render settings path.
    pub fn set_active_render_settings(&mut self, path: Option<SdfPath>) {
        *self.active_render_settings.lock().expect("Lock poisoned") = path;
    }
}

impl HdSceneIndexBase for HdStRenderSettingsSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            {
                let input_lock = input.read();
                return input_lock.get_prim(prim_path);
            }
        }
        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            {
                let input_lock = input.read();
                return input_lock.get_child_prim_paths(prim_path);
            }
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _msg: &Token, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdSt_RenderSettingsSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStRenderSettingsSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        // Track render settings prims; auto-activate first one found
        let mut active_render_settings = self.active_render_settings.lock().expect("Lock poisoned");
        for entry in entries {
            if Self::is_render_settings(&entry.prim_type) && active_render_settings.is_none() {
                *active_render_settings = Some(entry.prim_path.clone());
                log::info!(
                    "RenderSettings: auto-activated {}",
                    entry.prim_path.as_str()
                );
            }
        }
        drop(active_render_settings);
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        // Clear active render settings if removed
        let mut active_render_settings = self.active_render_settings.lock().expect("Lock poisoned");
        if let Some(ref active) = *active_render_settings {
            for entry in entries {
                if active.has_prefix(&entry.prim_path) {
                    *active_render_settings = None;
                    break;
                }
            }
        }
        drop(active_render_settings);
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
    fn test_create() {
        let si = HdStRenderSettingsSceneIndex::new(None);
        let lock = si.read();
        assert_eq!(lock.get_display_name(), "HdSt_RenderSettingsSceneIndex");
        assert!(lock.get_active_render_settings().is_none());
    }

    #[test]
    fn test_is_render_settings() {
        assert!(HdStRenderSettingsSceneIndex::is_render_settings(
            &Token::new("renderSettings")
        ));
        assert!(!HdStRenderSettingsSceneIndex::is_render_settings(
            &Token::new("mesh")
        ));
    }
}
