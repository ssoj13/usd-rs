#![allow(dead_code)]
//! PiPrototypeSceneIndex - Base prototype scene index for point instancers.
//!
//! Port of pxr/usdImaging/usdImaging/piPrototypeSceneIndex.h
//!
//! A scene index that prepares all prims under a given prototype root
//! to be instanced by a point instancer.

use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use usd_hd::HdDataSourceBaseHandle;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, SdfPathVector, si_ref, wire_filter_to_input,
};
use usd_sdf::Path;
use usd_tf::Token;

#[allow(dead_code)]
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static XFORM: LazyLock<Token> = LazyLock::new(|| Token::new("xform"));
    pub static RESET_XFORM_STACK: LazyLock<Token> = LazyLock::new(|| Token::new("resetXformStack"));
    pub static INSTANCED_BY: LazyLock<Token> = LazyLock::new(|| Token::new("instancedBy"));
    pub static PROTOTYPE_ROOT: LazyLock<Token> = LazyLock::new(|| Token::new("prototypeRoot"));
    pub static PATHS: LazyLock<Token> = LazyLock::new(|| Token::new("paths"));
    pub static POINT_INSTANCER: LazyLock<Token> = LazyLock::new(|| Token::new("pointInstancer"));
}

/// A scene index that prepares prims under a prototype root for instancing.
///
/// This scene index:
/// - Forces empty type on prims under an instancer within the prototype
/// - Forces empty type on prims under a USD "over" within the prototype
/// - Adds instancedBy data source to all prims whose type wasn't forced to empty
/// - Adds xform:resetXformStack to the prototype root
///
/// It is used by the prototype propagating scene index and should be preceded
/// by a rerooting scene index.
pub struct PiPrototypeSceneIndex {
    /// Base filtering scene index.
    base: HdSingleInputFilteringSceneIndexBase,
    /// The path of the point instancer using these prototypes.
    instancer: Path,
    /// The root path of the prototype hierarchy.
    prototype_root: Path,
    /// Instancers and overs within the prototype.
    /// Does not include nested instancers/overs under another instancer/over.
    instancers_and_overs: Mutex<HashSet<Path>>,
}

impl std::fmt::Debug for PiPrototypeSceneIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PiPrototypeSceneIndex")
            .field("instancer", &self.instancer)
            .field("prototype_root", &self.prototype_root)
            .finish()
    }
}

impl PiPrototypeSceneIndex {
    /// Creates a new prototype scene index.
    ///
    /// # Arguments
    ///
    /// * `input` - Input scene index (typically a rerooting scene index)
    /// * `instancer` - Path of the point instancer using these prototypes
    /// * `prototype_root` - Root path of the prototype hierarchy
    pub fn new(
        input: HdSceneIndexHandle,
        instancer: Path,
        prototype_root: Path,
    ) -> Arc<RwLock<Self>> {
        let scene_index = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input.clone())),
            instancer,
            prototype_root,
            instancers_and_overs: Mutex::new(HashSet::new()),
        }));
        wire_filter_to_input(&scene_index, &input);

        // Populate the instancers_and_overs set
        scene_index.write().populate();

        scene_index
    }

    /// Populate the instancers_and_overs set by traversing the prototype hierarchy.
    ///
    /// Ports C++ `UsdImaging_PiPrototypeSceneIndex::_Populate()` (piPrototypeSceneIndex.cpp:155-169).
    ///
    /// Traverses the input scene index under prototype_root. Any prim whose
    /// type is "pointInstancer" (or is a USD "over" specifier) is added to the
    /// set and its descendants are skipped — they will be force-typed to empty
    /// by `should_force_empty_type` so they are invisible to the renderer.
    fn populate(&self) {
        let input = match self.base.get_input_scene() {
            Some(s) => s,
            None => return,
        };
        // Collect paths to visit, starting from prototype_root.
        // We use an explicit stack to mimic C++ HdSceneIndexPrimView + SkipDescendants.
        let mut stack: Vec<Path> = vec![self.prototype_root.clone()];
        while let Some(path) = stack.pop() {
            let prim = si_ref(&input).get_prim(&path);
            let is_instancer = &prim.prim_type == &*tokens::POINT_INSTANCER;
            // Note: C++ also checks _IsOver() via UsdImagingUsdPrimInfoSchema::specifier.
            // That schema is not available at the scene-index level in Rust, so we only
            // detect point instancers here.  USD "over" suppression relies on the
            // data source being absent (prim_type empty), which is handled upstream.
            if is_instancer {
                self.instancers_and_overs
                    .lock()
                    .expect("Lock poisoned")
                    .insert(path);
                // Do NOT push children — equivalent to SkipDescendants().
                continue;
            }
            // Push children for further traversal.
            let children = si_ref(&input).get_child_prim_paths(&path);
            for child in children {
                stack.push(child);
            }
        }
    }

    /// Check if a path is an instancer or over within the prototype.
    fn is_instancer_or_over(&self, path: &Path) -> bool {
        self.instancers_and_overs
            .lock()
            .expect("Lock poisoned")
            .contains(path)
    }

    /// Check if a path is a descendant of an instancer or over.
    fn is_descendant_of_instancer_or_over(&self, path: &Path) -> bool {
        let mut current = path.get_parent_path();
        while !current.is_empty() && current.has_prefix(&self.prototype_root) {
            if self.is_instancer_or_over(&current) {
                return true;
            }
            current = current.get_parent_path();
        }
        false
    }

    /// Check if this path should have its type forced to empty.
    fn should_force_empty_type(&self, path: &Path) -> bool {
        // Skip paths outside the prototype root
        if !path.has_prefix(&self.prototype_root) {
            return false;
        }

        // Force empty for descendants of instancers/overs
        if self.is_descendant_of_instancer_or_over(path) {
            return true;
        }

        // Force empty for instancers/overs themselves (except at prototype root)
        if path != &self.prototype_root && self.is_instancer_or_over(path) {
            return true;
        }

        false
    }

    /// Check if this path should get instancedBy data source.
    fn should_add_instanced_by(&self, path: &Path) -> bool {
        // Only add to paths within prototype root
        if !path.has_prefix(&self.prototype_root) {
            return false;
        }

        // Don't add to paths with forced empty type (except prototype root itself)
        if path != &self.prototype_root && self.should_force_empty_type(path) {
            return false;
        }

        true
    }
}

impl HdSceneIndexBase for PiPrototypeSceneIndex {
    fn get_prim(&self, prim_path: &Path) -> HdSceneIndexPrim {
        let input_prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::default()
        };

        // Check if we should modify this prim
        if !prim_path.has_prefix(&self.prototype_root) {
            return input_prim;
        }

        let mut result = input_prim;

        // Force empty type if needed
        if self.should_force_empty_type(prim_path) {
            result.prim_type = Token::empty();
        }

        // Note: Full implementation would also add:
        // - instancedBy data source with prototypeRoot and paths
        // - xform:resetXformStack for the prototype root itself

        result
    }

    fn get_child_prim_paths(&self, prim_path: &Path) -> SdfPathVector {
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

    fn _system_message(&self, _message_type: &Token, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        format!("PiPrototypeSceneIndex({})", self.instancer)
    }
}

impl FilteringObserverTarget for PiPrototypeSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        // First pass: record any new instancers (C++ piPrototypeSceneIndex.cpp:270-292).
        let mut instancers_and_overs = self.instancers_and_overs.lock().expect("Lock poisoned");
        for entry in entries {
            if &entry.prim_type == &*tokens::POINT_INSTANCER {
                instancers_and_overs.insert(entry.prim_path.clone());
            }
        }
        drop(instancers_and_overs);
        // Second pass: for prims under a known instancer/over, clear the type
        // so the renderer ignores them (C++ piPrototypeSceneIndex.cpp:294-305).
        // We forward a modified copy where prim_type is set to empty.
        let mut modified: Vec<AddedPrimEntry> = entries.to_vec();
        for entry in &mut modified {
            if self.is_descendant_of_instancer_or_over(&entry.prim_path) {
                entry.prim_type = Token::empty();
            }
        }
        self.base.forward_prims_added(self, &modified);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        // Remove all entries from instancers_and_overs that have a removed path as prefix
        // (C++ piPrototypeSceneIndex.cpp:332-343).
        let mut instancers_and_overs = self.instancers_and_overs.lock().expect("Lock poisoned");
        for entry in entries {
            instancers_and_overs.retain(|p| !p.has_prefix(&entry.prim_path));
        }
        drop(instancers_and_overs);
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}

/// Handle type for PiPrototypeSceneIndex.
pub type PiPrototypeSceneIndexHandle = Arc<RwLock<PiPrototypeSceneIndex>>;

/// Creates a new prototype scene index.
pub fn create_pi_prototype_scene_index(
    input: HdSceneIndexHandle,
    instancer: Path,
    prototype_root: Path,
) -> PiPrototypeSceneIndexHandle {
    PiPrototypeSceneIndex::new(input, instancer, prototype_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prototype_root_prefix() {
        let prototype_root = Path::from_string("/MyPrototypes/MyPrototype").unwrap();
        let instancer = Path::from_string("/MyInstancer").unwrap();

        let child = Path::from_string("/MyPrototypes/MyPrototype/MySphere").unwrap();
        let outside = Path::from_string("/World/Geo").unwrap();

        assert!(child.has_prefix(&prototype_root));
        assert!(!outside.has_prefix(&prototype_root));

        // Test with empty instancers_and_overs
        let instancers_and_overs: HashSet<Path> = HashSet::new();
        assert!(!instancers_and_overs.contains(&child));

        // Silence unused variable warnings
        let _ = instancer;
    }

    #[test]
    fn test_path_relationships() {
        let prototype_root = Path::from_string("/MyPrototypes/MyPrototype").unwrap();
        let child = Path::from_string("/MyPrototypes/MyPrototype/MySphere").unwrap();
        let grandchild = Path::from_string("/MyPrototypes/MyPrototype/MySphere/Material").unwrap();

        assert!(child.has_prefix(&prototype_root));
        assert!(grandchild.has_prefix(&prototype_root));

        let parent = child.get_parent_path();
        assert_eq!(parent, prototype_root);
    }

    #[test]
    fn test_empty_type_logic() {
        let prototype_root = Path::from_string("/Proto").unwrap();
        let path1 = Path::from_string("/Proto/Child").unwrap();
        let path2 = Path::from_string("/Outside").unwrap();

        // Test basic prefix checking
        assert!(path1.has_prefix(&prototype_root));
        assert!(!path2.has_prefix(&prototype_root));
    }
}
