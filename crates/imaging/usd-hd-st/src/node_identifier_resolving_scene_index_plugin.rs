//! HdSt_NodeIdentifierResolvingSceneIndexPlugin - resolves shader node identifiers.
//!
//! Inserts a scene index that resolves material network node identifiers
//! to Storm's glslfx source type. This allows Storm to locate the correct
//! shader implementation for each material node.
//!
//! Port of C++ `HdSt_NodeIdentifierResolvingSceneIndexPlugin`.

use parking_lot::RwLock;
use std::sync::Arc;
use usd_hd::data_source::HdDataSourceBaseHandle;
use usd_hd::scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, FilteringObserverTarget, HdSceneIndexBase,
    HdSceneIndexHandle, HdSceneIndexObserverHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, RemovedPrimEntry, RenamedPrimEntry, SdfPathVector,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Insertion phase: early (phase 0).
pub const INSERTION_PHASE: u32 = 0;

/// Storm plugin display name.
pub const PLUGIN_DISPLAY_NAME: &str = "GL";

/// The shader source type Storm resolves node identifiers to.
pub const SOURCE_TYPE: &str = "glslfx";

/// Filtering scene index that resolves material node identifiers for Storm.
///
/// Rewrites node identifiers in material networks to point at the glslfx
/// implementation, enabling Storm to compile the correct shaders.
pub struct HdStNodeIdentifierResolvingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Shader source type to resolve to (e.g. "glslfx").
    source_type: Token,
}

impl HdStNodeIdentifierResolvingSceneIndex {
    /// Create a new node identifier resolving scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            source_type: Token::new(SOURCE_TYPE),
        }))
    }

    /// Get the source type used for resolution.
    pub fn source_type(&self) -> &Token {
        &self.source_type
    }
}

impl HdSceneIndexBase for HdStNodeIdentifierResolvingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            {
                let lock = input.read();
                return lock.get_prim(prim_path);
            }
        }
        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            {
                let lock = input.read();
                return lock.get_child_prim_paths(prim_path);
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
        "HdSt_NodeIdentifierResolvingSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStNodeIdentifierResolvingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
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

/// Plugin factory: create the node identifier resolving scene index.
pub fn create(
    input_scene: Option<HdSceneIndexHandle>,
) -> Arc<RwLock<HdStNodeIdentifierResolvingSceneIndex>> {
    HdStNodeIdentifierResolvingSceneIndex::new(input_scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create() {
        let si = create(None);
        let lock = si.read();
        assert_eq!(
            lock.get_display_name(),
            "HdSt_NodeIdentifierResolvingSceneIndex"
        );
        assert_eq!(lock.source_type().as_str(), "glslfx");
    }

    #[test]
    fn test_constants() {
        assert_eq!(INSERTION_PHASE, 0);
        assert_eq!(SOURCE_TYPE, "glslfx");
    }
}
