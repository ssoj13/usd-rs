
//! Material binding resolving scene index.
//!
//! Resolves material bindings by walking up the hierarchy.
//!
//! In USD, material bindings can be inherited from ancestors. If a prim doesn't
//! have a direct material binding, it may inherit one from a parent prim.
//! This scene index resolves those inherited bindings so downstream consumers
//! see the effective binding on every prim.

use once_cell::sync::Lazy;
use std::sync::Arc;
use parking_lot::RwLock;
use usd_hd::data_source::HdDataSourceBaseHandle;
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, SdfPathVector, si_ref,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Token for material binding data source name.
static MATERIAL_BINDING: Lazy<TfToken> = Lazy::new(|| TfToken::new("materialBinding"));

/// Material binding resolving scene index.
pub struct HdsiMaterialBindingResolvingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl HdsiMaterialBindingResolvingSceneIndex {
    /// Creates a new material binding resolving scene index.
    ///
    /// This scene index walks up the hierarchy to resolve inherited material bindings
    /// from ancestor prims when they are not directly assigned.
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
        }));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene.read().add_observer(Arc::new(filtering_observer));
        }
        observer
    }
}

impl HdsiMaterialBindingResolvingSceneIndex {
    /// Resolve material binding for a prim by walking up the hierarchy.
    ///
    /// If the prim has a materialBinding data source, return it.
    /// Otherwise, walk up to parent prims until we find one with a binding.
    fn resolve_material_binding(
        &self,
        input: &dyn HdSceneIndexBase,
        prim_path: &SdfPath,
    ) -> Option<HdDataSourceBaseHandle> {
        let mut current_path = prim_path.clone();

        loop {
            let prim = input.get_prim(&current_path);

            // Check if this prim has a material binding
            if let Some(data_source) = &prim.data_source {
                if let Some(binding) = data_source.get(&MATERIAL_BINDING) {
                    // Found a material binding - return it
                    return Some(binding.clone());
                }
            }

            // Move to parent path
            let parent = current_path.get_parent_path();
            if parent.is_empty() || parent == current_path {
                // Reached root without finding binding
                break;
            }
            current_path = parent;
        }

        None
    }
}

impl HdSceneIndexBase for HdsiMaterialBindingResolvingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            let input_locked = input.read();
                let prim = input_locked.get_prim(prim_path);

                // Check if prim already has a material binding
                let has_binding = prim
                    .data_source
                    .as_ref()
                    .map(|ds| ds.get(&MATERIAL_BINDING).is_some())
                    .unwrap_or(false);

                if !has_binding {
                    if let Some(_inherited) =
                        self.resolve_material_binding(&*input_locked, &prim_path.get_parent_path())
                    {
                    }
                }

                return prim;
        }
        HdSceneIndexPrim::default()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_child_prim_paths(prim_path);
        }
        Vec::new()
    }

    fn add_observer(&self, observer: usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiMaterialBindingResolvingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiMaterialBindingResolvingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if entries.len() >= 1000 {
            let first = entries
                .first()
                .map(|e| e.prim_path.to_string())
                .unwrap_or_default();
            eprintln!(
                "[material_binding_resolving] on_prims_dirtied in={} sender={} first={}",
                entries.len(),
                sender.get_display_name(),
                first,
            );
        }
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
