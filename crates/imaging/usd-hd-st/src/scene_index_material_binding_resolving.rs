
//! HdSt_MaterialBindingResolvingSceneIndex - material binding resolution for Storm.
//!
//! Plugin scene index that wraps the generic material binding resolving scene
//! index (from hdsi) to resolve material bindings for Storm rendering.
//! Selects the "preview" purpose binding, falling back to "allPurpose".
//!
//! # Algorithm
//!
//! For each prim that has `materialBindings` in its data source, the scene
//! index resolves the binding by selecting from the available bindings based
//! on purpose priority:
//! 1. Check for "preview" purpose binding
//! 2. Fall back to "" (allPurpose) binding
//!
//! The resolved binding path is exposed as `materialBinding` (singular) on
//! the prim's data source, overlaid over the existing data source.
//!
//! Port of C++ `HdSt_MaterialBindingResolvingSceneIndexPlugin` which
//! wraps `HdsiMaterialBindingResolvingSceneIndex`.

use std::sync::Arc;
use parking_lot::RwLock;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdOverlayContainerDataSource, HdRetainedContainerDataSource,
    HdRetainedSampledDataSource,
};
use usd_hd::scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, FilteringObserverTarget, HdSceneIndexBase,
    HdSceneIndexHandle, HdSceneIndexObserverHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, RemovedPrimEntry, RenamedPrimEntry, SdfPathVector,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn make_sampled(value: Value) -> HdDataSourceBaseHandle {
    HdRetainedSampledDataSource::new(value).clone_box()
}

// ---------------------------------------------------------------------------
// Material binding priority config
// ---------------------------------------------------------------------------

/// Material binding priorities for Storm resolution.
///
/// Storm resolves material bindings in this priority order:
/// 1. "preview" purpose binding
/// 2. "allPurpose" (empty purpose) binding
#[derive(Clone, Debug)]
pub struct MaterialBindingPriority {
    /// Ordered purposes to check (first match wins).
    pub purposes: Vec<Token>,
    /// Fallback purpose if none match.
    pub fallback: Token,
}

impl Default for MaterialBindingPriority {
    fn default() -> Self {
        Self {
            purposes: vec![Token::new("preview"), Token::new("allPurpose")],
            fallback: Token::new("allPurpose"),
        }
    }
}

// ---------------------------------------------------------------------------
// Resolved material binding data source
// ---------------------------------------------------------------------------

/// Container data source that provides the resolved `materialBinding.path` child.
///
/// Wraps an already-resolved material path SdfPath and exposes it as a
/// `path` data source child that downstream Storm code expects.
#[derive(Clone, Debug)]
struct ResolvedMaterialBindingDataSource {
    /// Resolved material path as string.
    material_path_str: String,
}

impl ResolvedMaterialBindingDataSource {
    fn new(material_path: &SdfPath) -> Arc<Self> {
        Arc::new(Self {
            material_path_str: material_path.as_str().to_string(),
        })
    }
}

usd_hd::impl_container_datasource_base!(ResolvedMaterialBindingDataSource);

impl HdContainerDataSource for ResolvedMaterialBindingDataSource {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("path")]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == "path" {
            Some(make_sampled(Value::from(self.material_path_str.clone())))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Resolution logic
// ---------------------------------------------------------------------------

/// Resolve a material binding from a prim data source.
///
/// Looks in `materialBindings` for a binding matching one of the `purposes`
/// (in priority order). Returns the resolved `SdfPath`, or `None` if no binding
/// is found.
///
/// C++ equivalent: `HdsiMaterialBindingResolvingSceneIndex` which selects
/// the highest-priority purpose's binding path.
fn resolve_material_binding(
    data_source: &HdContainerDataSourceHandle,
    priority: &MaterialBindingPriority,
) -> Option<SdfPath> {
    // Look for materialBindings container (has per-purpose children)
    let bindings_ds = data_source.get(&Token::new("materialBindings"))?;
    let bindings_container = bindings_ds
        .as_any()
        .downcast_ref::<HdRetainedContainerDataSource>()?;

    // Try each purpose in priority order
    for purpose in &priority.purposes {
        if let Some(binding_ds) = bindings_container.get(purpose) {
            // Each purpose child should have a "path" child with the material SdfPath
            if let Some(container) = binding_ds
                .as_any()
                .downcast_ref::<HdRetainedContainerDataSource>()
            {
                if let Some(path_ds) = container.get(&Token::new("path")) {
                    // Read via sample_at_zero shortcut
                    if let Some(val) = path_ds.sample_at_zero() {
                        if let Some(path_str) = val.get::<String>() {
                            if !path_str.is_empty() {
                                if let Some(path) = SdfPath::from_string(path_str) {
                                    return Some(path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

/// Build an overlay data source that adds a resolved `materialBinding` child.
fn build_resolved_binding_overlay(
    base_ds: HdContainerDataSourceHandle,
    material_path: &SdfPath,
) -> HdContainerDataSourceHandle {
    let resolved_ds = ResolvedMaterialBindingDataSource::new(material_path);

    // Wrap as materialBinding container
    let binding_container = HdRetainedContainerDataSource::new_1(
        Token::new("materialBinding"),
        resolved_ds.clone_box(),
    );

    HdOverlayContainerDataSource::new_2(binding_container, base_ds)
}

// ---------------------------------------------------------------------------
// Scene index
// ---------------------------------------------------------------------------

/// Material binding resolving scene index for Storm.
///
/// Resolves the material binding on each prim by selecting from the
/// available bindings based on purpose priority. Storm defaults to
/// the "preview" purpose, falling back to "allPurpose".
///
/// Port of C++ `HdSt_MaterialBindingResolvingSceneIndexPlugin` which
/// wraps `HdsiMaterialBindingResolvingSceneIndex`.
pub struct HdStMaterialBindingResolvingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Binding resolution priority.
    priority: MaterialBindingPriority,
}

impl HdStMaterialBindingResolvingSceneIndex {
    /// Create with default Storm priorities (preview > allPurpose).
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Self::with_priority(input_scene, MaterialBindingPriority::default())
    }

    /// Create with custom binding priority.
    pub fn with_priority(
        input_scene: Option<HdSceneIndexHandle>,
        priority: MaterialBindingPriority,
    ) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            priority,
        }))
    }

    /// Get the binding resolution priority.
    pub fn get_priority(&self) -> &MaterialBindingPriority {
        &self.priority
    }
}

impl HdSceneIndexBase for HdStMaterialBindingResolvingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            { let input_lock = input.read();
                let prim = input_lock.get_prim(prim_path);

                if let Some(base_ds) = prim.data_source.clone() {
                    // Attempt to resolve materialBindings -> materialBinding
                    if let Some(resolved_path) = resolve_material_binding(&base_ds, &self.priority)
                    {
                        return HdSceneIndexPrim {
                            prim_type: prim.prim_type,
                            data_source: Some(build_resolved_binding_overlay(
                                base_ds,
                                &resolved_path,
                            )),
                        };
                    }
                }
                return prim;
            }
        }
        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            { let input_lock = input.read();
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
        "HdSt_MaterialBindingResolvingSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStMaterialBindingResolvingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let binding_loc = HdDataSourceLocator::from_token(Token::new("materialBindings"));
        let resolved_loc = HdDataSourceLocator::from_token(Token::new("materialBinding"));
        let mut augmented = Vec::with_capacity(entries.len());

        for entry in entries {
            // If materialBindings changed, also dirty the resolved materialBinding
            if entry.dirty_locators.intersects_locator(&binding_loc) {
                let mut locs = entry.dirty_locators.clone();
                locs.insert(resolved_loc.clone());
                augmented.push(DirtiedPrimEntry::new(entry.prim_path.clone(), locs));
            } else {
                augmented.push(entry.clone());
            }
        }

        self.base.forward_prims_dirtied(self, &augmented);
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
        let si = HdStMaterialBindingResolvingSceneIndex::new(None);
        let lock = si.read();
        assert_eq!(
            lock.get_display_name(),
            "HdSt_MaterialBindingResolvingSceneIndex"
        );
    }

    #[test]
    fn test_default_priority() {
        let si = HdStMaterialBindingResolvingSceneIndex::new(None);
        let lock = si.read();
        let priority = lock.get_priority();
        assert_eq!(priority.purposes.len(), 2);
        assert_eq!(priority.purposes[0].as_str(), "preview");
        assert_eq!(priority.purposes[1].as_str(), "allPurpose");
        assert_eq!(priority.fallback.as_str(), "allPurpose");
    }

    #[test]
    fn test_resolved_binding_data_source() {
        let path = SdfPath::from_string("/Materials/Mat").unwrap();
        let ds = ResolvedMaterialBindingDataSource::new(&path);
        let names = ds.get_names();
        assert!(names.contains(&Token::new("path")));

        let path_ds = ds.get(&Token::new("path"));
        assert!(path_ds.is_some());
    }

    #[test]
    fn test_custom_priority() {
        let priority = MaterialBindingPriority {
            purposes: vec![Token::new("full"), Token::new("preview")],
            fallback: Token::new("preview"),
        };
        let si = HdStMaterialBindingResolvingSceneIndex::with_priority(None, priority);
        let lock = si.read();
        assert_eq!(lock.get_priority().purposes[0].as_str(), "full");
    }
}
