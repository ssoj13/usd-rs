
//! Rerooting scene index - remaps prim paths to a new root.
//!
//! This scene index transforms paths from an input scene by rerooting them
//! under a different path. For example, if root_path is "/World", then the
//! prim at "/Foo" in the input becomes "/World/Foo" in this scene index.

use parking_lot::RwLock;
use std::sync::{Arc, LazyLock};
use usd_hd::scene_index::base::TfTokenVector;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, SdfPathVector, wire_filter_to_input, si_ref,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

// Tokens for rerooting
#[allow(dead_code)]
mod tokens {
    use super::*;

    pub static ROOT_PATH: LazyLock<TfToken> = LazyLock::new(|| TfToken::new("rootPath"));
}

/// Path mapping mode for rerooting.
#[derive(Clone)]
enum RerootMapping {
    /// Map / to root_path (new(input, root_path) behavior)
    RootToPath { root_path: SdfPath },
    /// Replace src_prefix with dst_prefix (UsdImagingRerootingSceneIndex behavior)
    ReplacePrefix {
        src_prefix: SdfPath,
        dst_prefix: SdfPath,
    },
}

/// A scene index that reroots all prims under a specified path.
///
/// This is useful for:
/// - Namespace isolation (putting a scene under a specific root)
/// - Path transformation when composing multiple scenes
/// - Testing scene indices with controlled paths
///
/// # Example
///
/// ```ignore
/// // Input scene has prim at "/Cube"
/// let rerooted = HdRerootingSceneIndex::new(input_scene, SdfPath::from_string("/World"));
/// // Rerooted scene has prim at "/World/Cube"
/// let prim = rerooted.get_prim(&SdfPath::from_string("/World/Cube"));
/// ```
pub struct HdRerootingSceneIndex {
    /// Filtering base for observer management
    filtering_base: HdSingleInputFilteringSceneIndexBase,
    /// Path mapping configuration
    mapping: RerootMapping,
}

impl HdRerootingSceneIndex {
    /// Create a new rerooting scene index (maps / to root_path).
    ///
    /// # Arguments
    ///
    /// * `input_scene` - The input scene to reroot
    /// * `root_path` - The new root path (must be absolute)
    pub fn new(input_scene: Option<HdSceneIndexHandle>, root_path: SdfPath) -> Arc<RwLock<Self>> {
        let result = Arc::new(RwLock::new(Self {
            filtering_base: HdSingleInputFilteringSceneIndexBase::new(input_scene.clone()),
            mapping: RerootMapping::RootToPath { root_path },
        }));
        if let Some(ref input) = input_scene {
            wire_filter_to_input(&result, input);
        }
        result
    }

    /// Create a rerooting scene index that replaces src_prefix with dst_prefix.
    ///
    /// Drops prims not under src_prefix; those under src_prefix are
    /// remapped by replacing src_prefix with dst_prefix.
    ///
    /// Port of UsdImagingRerootingSceneIndex::New(inputScene, srcPrefix, dstPrefix).
    pub fn new_with_prefixes(
        input_scene: Option<HdSceneIndexHandle>,
        src_prefix: SdfPath,
        dst_prefix: SdfPath,
    ) -> Arc<RwLock<Self>> {
        let result = Arc::new(RwLock::new(Self {
            filtering_base: HdSingleInputFilteringSceneIndexBase::new(input_scene.clone()),
            mapping: RerootMapping::ReplacePrefix {
                src_prefix,
                dst_prefix,
            },
        }));
        if let Some(ref input) = input_scene {
            wire_filter_to_input(&result, input);
        }
        result
    }

    /// Get the root path used for rerooting (when using new()).
    pub fn get_root_path(&self) -> &SdfPath {
        match &self.mapping {
            RerootMapping::RootToPath { root_path } => root_path,
            RerootMapping::ReplacePrefix { dst_prefix, .. } => dst_prefix,
        }
    }

    /// Set a new root path (only for RootToPath mode).
    ///
    /// Note: This will invalidate all cached paths. In production, this should
    /// trigger appropriate scene change notifications.
    pub fn set_root_path(&mut self, root_path: SdfPath) {
        self.mapping = RerootMapping::RootToPath { root_path };
    }

    /// Transform a path from the output space back to the input space.
    fn map_to_input(&self, output_path: &SdfPath) -> Option<SdfPath> {
        match &self.mapping {
            RerootMapping::RootToPath { root_path } => {
                if root_path.is_absolute_root_path() {
                    return Some(output_path.clone());
                }
                if !output_path.has_prefix(root_path) {
                    return None;
                }
                let relative = output_path.make_relative(root_path)?;
                SdfPath::absolute_root().append_path(&relative)
            }
            RerootMapping::ReplacePrefix {
                src_prefix,
                dst_prefix,
            } => {
                if !output_path.has_prefix(dst_prefix) {
                    return None;
                }
                output_path.replace_prefix(dst_prefix, src_prefix)
            }
        }
    }

    /// Transform a path from the input space to the output space.
    fn map_from_input(&self, input_path: &SdfPath) -> Option<SdfPath> {
        match &self.mapping {
            RerootMapping::RootToPath { root_path } => {
                if root_path.is_absolute_root_path() {
                    return Some(input_path.clone());
                }
                if let Some(relative) = input_path.make_relative(&SdfPath::absolute_root()) {
                    return root_path.append_path(&relative);
                }
                Some(input_path.clone())
            }
            RerootMapping::ReplacePrefix {
                src_prefix,
                dst_prefix,
            } => {
                if !input_path.has_prefix(src_prefix) {
                    return None;
                }
                input_path.replace_prefix(src_prefix, dst_prefix)
            }
        }
    }
}

impl HdSceneIndexBase for HdRerootingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let input_path = match self.map_to_input(prim_path) {
            Some(path) => path,
            None => return HdSceneIndexPrim::empty(),
        };

        if let Some(input) = self.filtering_base.get_input_scene() {
            return si_ref(&input).get_prim(&input_path);
        }

        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        let input_path = match self.map_to_input(prim_path) {
            Some(path) => path,
            None => {
                // Special case: querying dst root, return path to input root's children
                let dst_root = self.get_root_path();
                if prim_path == dst_root {
                    let input_root = SdfPath::absolute_root();
                    if let Some(input) = self.filtering_base.get_input_scene() {
                        let input_children = si_ref(&input).get_child_prim_paths(&input_root);
                        return input_children
                            .into_iter()
                            .filter_map(|p| self.map_from_input(&p))
                            .collect();
                    }
                }
                return Vec::new();
            }
        };

        if let Some(input) = self.filtering_base.get_input_scene() {
            let input_children = si_ref(&input).get_child_prim_paths(&input_path);
            return input_children
                .into_iter()
                .filter_map(|p| self.map_from_input(&p))
                .collect();
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
            format!("HdRerootingSceneIndex({})", self.get_root_path().get_text())
        } else {
            name.to_string()
        }
    }
}

impl FilteringObserverTarget for HdRerootingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let remapped: Vec<_> = entries
            .iter()
            .filter_map(|e| {
                self.map_from_input(&e.prim_path)
                    .map(|prim_path| AddedPrimEntry {
                        prim_path,
                        prim_type: e.prim_type.clone(),
                        data_source: e.data_source.clone(),
                    })
            })
            .collect();
        self.filtering_base.forward_prims_added(self, &remapped);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let remapped: Vec<_> = entries
            .iter()
            .filter_map(|e| {
                self.map_from_input(&e.prim_path)
                    .map(|prim_path| RemovedPrimEntry { prim_path })
            })
            .collect();
        self.filtering_base.forward_prims_removed(self, &remapped);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let remapped: Vec<_> = entries
            .iter()
            .filter_map(|e| {
                self.map_from_input(&e.prim_path)
                    .map(|prim_path| DirtiedPrimEntry {
                        prim_path,
                        dirty_locators: e.dirty_locators.clone(),
                    })
            })
            .collect();
        self.filtering_base.forward_prims_dirtied(self, &remapped);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        let remapped: Vec<_> = entries
            .iter()
            .filter_map(|e| {
                let old_path = self.map_from_input(&e.old_prim_path)?;
                let new_path = self.map_from_input(&e.new_prim_path)?;
                Some(RenamedPrimEntry {
                    old_prim_path: old_path,
                    new_prim_path: new_path,
                })
            })
            .collect();
        self.filtering_base.forward_prims_renamed(self, &remapped);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rerooting_creation() {
        let root = SdfPath::from_string("/World").unwrap();
        let scene = HdRerootingSceneIndex::new(None, root.clone());
        let scene_lock = scene.read();

        assert_eq!(scene_lock.get_root_path(), &root);
    }

    #[test]
    fn test_path_mapping_to_input() {
        let root = SdfPath::from_string("/World").unwrap();
        let scene = HdRerootingSceneIndex::new(None, root);
        let scene_lock = scene.read();

        // Path under root should map correctly
        let rerooted = SdfPath::from_string("/World/Cube").unwrap();
        let mapped = scene_lock.map_to_input(&rerooted);
        assert!(mapped.is_some());
        assert_eq!(mapped.unwrap().get_text(), "/Cube");

        // Path not under root should return None
        let outside = SdfPath::from_string("/Other/Sphere").unwrap();
        let mapped = scene_lock.map_to_input(&outside);
        assert!(mapped.is_none());
    }

    #[test]
    fn test_path_mapping_from_input() {
        let root = SdfPath::from_string("/World").unwrap();
        let scene = HdRerootingSceneIndex::new(None, root);
        let scene_lock = scene.read();

        let input = SdfPath::from_string("/Cube").unwrap();
        let mapped = scene_lock.map_from_input(&input).unwrap();
        assert_eq!(mapped.get_text(), "/World/Cube");

        let input_nested = SdfPath::from_string("/Assets/Cube").unwrap();
        let mapped_nested = scene_lock.map_from_input(&input_nested).unwrap();
        assert_eq!(mapped_nested.get_text(), "/World/Assets/Cube");
    }

    #[test]
    fn test_root_at_absolute_root() {
        let root = SdfPath::absolute_root();
        let scene = HdRerootingSceneIndex::new(None, root);
        let scene_lock = scene.read();

        let path = SdfPath::from_string("/Cube").unwrap();
        let mapped_to = scene_lock.map_to_input(&path);
        assert_eq!(mapped_to.unwrap(), path);

        let mapped_from = scene_lock.map_from_input(&path).unwrap();
        assert_eq!(mapped_from, path);
    }

    #[test]
    fn test_new_with_prefixes() {
        let input = SdfPath::from_string("/UsdNiInstancer/UsdNiPrototype").unwrap();
        let src = SdfPath::from_string("/UsdNiInstancer").unwrap();
        let dst = SdfPath::from_string("/X/Y/__Prototype_1/UsdNiInstancer").unwrap();
        let scene = HdRerootingSceneIndex::new_with_prefixes(None, src, dst);
        let scene_lock = scene.read();

        let mapped = scene_lock.map_from_input(&input).unwrap();
        assert_eq!(
            mapped.get_text(),
            "/X/Y/__Prototype_1/UsdNiInstancer/UsdNiPrototype"
        );

        let back = scene_lock.map_to_input(&mapped).unwrap();
        assert_eq!(back, input);
    }

    #[test]
    fn test_get_prim_without_input() {
        let root = SdfPath::from_string("/World").unwrap();
        let scene = HdRerootingSceneIndex::new(None, root);
        let scene_lock = scene.read();

        let prim = scene_lock.get_prim(&SdfPath::from_string("/World/Cube").unwrap());
        assert!(!prim.is_defined());
    }
}
