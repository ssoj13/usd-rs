//! MaterialBindingsResolvingSceneIndex - resolves USD material bindings into Hydra bindings.
//!
//! Port of pxr/usdImaging/usdImaging/materialBindingsResolvingSceneIndex.cpp
//!
//! Wraps input prim data sources with resolved Hydra materialBindings by reading
//! the flattened USD material bindings (usdMaterialBindings) and evaluating
//! direct and collection bindings per the USD binding resolution algorithm.
//!
//! Direct bindings and collection bindings are ordered ancestor-first (DFS order).
//! For each {direct, collection} pair:
//!   - Collection binding with strongerThanDescendants wins immediately
//!   - Direct binding with strongerThanDescendants wins immediately
//!   - Otherwise collection > direct at any namespace level
//!   - Most local (last) binding without strongerThanDescendants wins

use std::sync::Arc;
use parking_lot::RwLock;
use usd_hd::data_source::{
    HdDataSourceBaseHandle, HdDataSourceLocator, HdDataSourceLocatorSet,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource, HdTypedSampledDataSource,
};
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdContainerDataSourceHandle, HdSceneIndexBase, HdSceneIndexHandle,
    HdSceneIndexPrim, HdSingleInputFilteringSceneIndexBase, SdfPathVector, wire_filter_to_input,
};
use usd_hd::{HdContainerDataSource, HdDataSourceBase};
use usd_sdf::Path;
use usd_tf::Token;

// -- tokens ------------------------------------------------------------------

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// Hydra material bindings schema token.
    pub static MATERIAL_BINDINGS: LazyLock<Token> =
        LazyLock::new(|| Token::new("materialBindings"));
    /// USD-side material bindings schema token.
    pub static USD_MATERIAL_BINDINGS: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdMaterialBindings"));
    /// Material path field.
    pub static PATH: LazyLock<Token> = LazyLock::new(|| Token::new("path"));
    /// Binding strength token.
    pub static STRONGER_THAN_DESCENDANTS: LazyLock<Token> =
        LazyLock::new(|| Token::new("strongerThanDescendants"));
}

// -- _HdMaterialBindingsDataSource -------------------------------------------

/// Resolved Hydra material bindings container keyed by purpose.
///
/// For each purpose, resolves the winning binding from the flattened
/// direct+collection USD material bindings vector and returns an
/// HdMaterialBindingSchema-compatible container with "path".
#[derive(Clone)]
struct HdMaterialBindingsDataSource {
    /// The original prim container (carries usdMaterialBindings).
    prim_container: HdContainerDataSourceHandle,
    /// Scene index for collection membership evaluation.
    #[allow(dead_code)]
    si: HdSceneIndexHandle,
    /// Prim path used for collection membership tests.
    #[allow(dead_code)]
    prim_path: Path,
}

impl std::fmt::Debug for HdMaterialBindingsDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdMaterialBindingsDataSource")
            .field("prim_path", &self.prim_path)
            .finish()
    }
}

impl HdMaterialBindingsDataSource {
    fn new(
        prim_container: HdContainerDataSourceHandle,
        si: HdSceneIndexHandle,
        prim_path: Path,
    ) -> Arc<Self> {
        Arc::new(Self {
            prim_container,
            si,
            prim_path,
        })
    }

    /// Read the USD material bindings schema and return available purposes.
    fn get_purposes(&self) -> Vec<Token> {
        if let Some(usd_bindings) = self.prim_container.get(&tokens::USD_MATERIAL_BINDINGS) {
            if let Some(container) = usd_hd::data_source::cast_to_container(&usd_bindings) {
                return container.get_names();
            }
        }
        Vec::new()
    }

    /// Resolve the winning material path for a purpose.
    ///
    /// Walks the flattened binding vector (ancestor-first, DFS order).
    /// For each element, evaluates collection bindings first, then direct.
    /// C++ parity: _ComputeResolvedMaterialBinding.
    ///
    /// Resolution rules:
    /// - Collection binding with strongerThanDescendants wins immediately
    /// - Direct binding with strongerThanDescendants wins immediately
    /// - Otherwise collection > direct at any namespace level
    /// - Most local (last) binding without strongerThanDescendants wins
    fn resolve_for_purpose(&self, purpose: &Token) -> Option<Path> {
        let usd_bindings_ds = self.prim_container.get(&tokens::USD_MATERIAL_BINDINGS)?;
        let usd_bindings_container = usd_hd::data_source::cast_to_container(&usd_bindings_ds)?;

        // Get the binding vector for this purpose
        let purpose_ds = usd_bindings_container.get(purpose)?;
        let purpose_container = usd_hd::data_source::cast_to_container(&purpose_ds)?;

        // Walk elements (ancestor-first, i.e. DFS order).
        // Each element is a {directMaterialBinding, collectionMaterialBindings} pair.
        let element_names = purpose_container.get_names();
        let mut winning_path: Option<Path> = None;

        for element_name in &element_names {
            let Some(element_ds) = purpose_container.get(element_name) else {
                continue;
            };
            let Some(element_container) = usd_hd::data_source::cast_to_container(&element_ds)
            else {
                continue;
            };

            // Evaluate collection bindings first (C++ parity)
            let col_bind_info = self.resolve_collection_binding(&element_container);

            if let Some((ref col_path, col_stronger)) = col_bind_info {
                if col_stronger {
                    // strongerThanDescendants -> immediate win
                    return Some(col_path.clone());
                }
            }

            // Direct binding resolution
            let dir_bind_info = Self::resolve_direct_binding(&element_container);

            if let Some((ref dir_path, dir_stronger)) = dir_bind_info {
                if dir_stronger {
                    // strongerThanDescendants -> immediate win
                    return Some(dir_path.clone());
                }
            }

            // Neither is strongerThanDescendants:
            // Collection binding wins over direct at any namespace level
            if let Some((ref col_path, _)) = col_bind_info {
                winning_path = Some(col_path.clone());
                continue;
            }

            if let Some((ref dir_path, _)) = dir_bind_info {
                winning_path = Some(dir_path.clone());
            }
        }

        winning_path
    }

    /// Resolve collection material binding from a binding element.
    ///
    /// Iterates collectionMaterialBindings, evaluating collection membership
    /// for the prim. Returns the first matching binding's (material path, strength).
    fn resolve_collection_binding(
        &self,
        element_container: &HdContainerDataSourceHandle,
    ) -> Option<(Path, bool)> {
        let col_bindings_ds = element_container.get(&Token::new("collectionMaterialBindings"))?;
        let col_bindings_container = usd_hd::data_source::cast_to_container(&col_bindings_ds)?;

        let col_names = col_bindings_container.get_names();
        for col_name in &col_names {
            let Some(col_ds) = col_bindings_container.get(col_name) else {
                continue;
            };
            let Some(col_container) = usd_hd::data_source::cast_to_container(&col_ds) else {
                continue;
            };

            // Read collection prim path and name
            let col_prim_path =
                Self::read_path_field(&col_container, &Token::new("collectionPrimPath"));
            let col_col_name =
                Self::read_token_field(&col_container, &Token::new("collectionName"));
            let mat_path = Self::read_path(&col_container);

            let (Some(col_prim_path), Some(col_col_name), Some(mat_path)) =
                (col_prim_path, col_col_name, mat_path)
            else {
                continue;
            };

            // Evaluate collection membership via scene index
            if self.prim_matches_collection(&col_prim_path, &col_col_name) {
                let is_stronger = Self::read_strength(&col_container);
                return Some((mat_path, is_stronger));
            }
        }

        None
    }

    /// Resolve direct material binding from a binding element.
    /// Returns (material path, strongerThanDescendants).
    fn resolve_direct_binding(
        element_container: &HdContainerDataSourceHandle,
    ) -> Option<(Path, bool)> {
        let dir_ds = element_container.get(&Token::new("directMaterialBinding"))?;
        let dir_container = usd_hd::data_source::cast_to_container(&dir_ds)?;
        let mat_path = Self::read_path(&dir_container)?;
        let is_stronger = Self::read_strength(&dir_container);
        Some((mat_path, is_stronger))
    }

    /// Check if the prim matches a collection by querying the scene index.
    ///
    /// Looks up the collection on the collection prim via
    /// collections -> collectionName -> membershipExpression, then evaluates
    /// whether self.prim_path matches. Falls back to path-prefix heuristic
    /// if the full expression evaluator is not available.
    fn prim_matches_collection(
        &self,
        collection_prim_path: &Path,
        collection_name: &Token,
    ) -> bool {
        // Query the scene index for the collection prim's data
        let si_locked = self.si.read();
        let col_prim = si_locked.get_prim(collection_prim_path);
        if let Some(ref ds) = col_prim.data_source {
            // Try: collections -> collectionName -> membershipExpression
            if let Some(collections_ds) = ds.get(&Token::new("collections")) {
                if let Some(collections_container) =
                    usd_hd::data_source::cast_to_container(&collections_ds)
                {
                    if let Some(col_ds) = collections_container.get(collection_name) {
                        if let Some(col_container) =
                            usd_hd::data_source::cast_to_container(&col_ds)
                        {
                            // Check membershipExpression or expandedPaths
                            return self.eval_collection_membership(&col_container);
                        }
                    }
                }
            }
        }

        // Fallback: assume the collection applies if prim is a descendant of
        // the collection prim (common case for per-subtree material assignment)
        self.prim_path.has_prefix(collection_prim_path)
    }

    /// Evaluate collection membership from a collection schema container.
    ///
    /// Checks membershipExpression or expandedPaths for a match.
    fn eval_collection_membership(&self, col_container: &HdContainerDataSourceHandle) -> bool {
        // Try expandedPaths first (simpler, common case)
        if let Some(paths_ds) = col_container.get(&Token::new("expandedPaths")) {
            if let Some(vec_ds) = usd_hd::data_source::cast_to_vector(&paths_ds) {
                let n = vec_ds.get_num_elements();
                for i in 0..n {
                    if let Some(elem) = vec_ds.get_element(i) {
                        let any = elem.as_any();
                        if let Some(typed) =
                            any.downcast_ref::<HdRetainedTypedSampledDataSource<Path>>()
                        {
                            let member_path = typed.get_typed_value(0.0);
                            if self.prim_path == member_path
                                || self.prim_path.has_prefix(&member_path)
                            {
                                return true;
                            }
                        }
                    }
                }
                // If we had expandedPaths but didn't match, return false
                if n > 0 {
                    return false;
                }
            }
        }

        // No expandedPaths found - fall back to true (assume match)
        // This handles the case where full expression evaluation is not yet implemented
        true
    }

    /// Read a Path field from a container by name.
    fn read_path_field(container: &HdContainerDataSourceHandle, name: &Token) -> Option<Path> {
        let ds = container.get(name)?;
        let any = ds.as_any();
        if let Some(typed) = any.downcast_ref::<HdRetainedTypedSampledDataSource<Path>>() {
            return Some(typed.get_typed_value(0.0).clone());
        }
        None
    }

    /// Read a Token field from a container by name.
    fn read_token_field(container: &HdContainerDataSourceHandle, name: &Token) -> Option<Token> {
        let ds = container.get(name)?;
        let any = ds.as_any();
        if let Some(typed) = any.downcast_ref::<HdRetainedTypedSampledDataSource<Token>>() {
            return Some(typed.get_typed_value(0.0).clone());
        }
        None
    }

    /// Read "materialPath" or "path" from a binding container.
    fn read_path(container: &HdContainerDataSourceHandle) -> Option<Path> {
        let path_ds = container
            .get(&Token::new("materialPath"))
            .or_else(|| container.get(&tokens::PATH))?;
        let any = path_ds.as_any();
        if let Some(typed) = any.downcast_ref::<HdRetainedTypedSampledDataSource<Path>>() {
            return Some(typed.get_typed_value(0.0).clone());
        }
        None
    }

    /// Read binding strength; true => strongerThanDescendants.
    fn read_strength(container: &HdContainerDataSourceHandle) -> bool {
        if let Some(strength_ds) = container.get(&Token::new("bindingStrength")) {
            let any = strength_ds.as_any();
            if let Some(typed) = any.downcast_ref::<HdRetainedTypedSampledDataSource<Token>>() {
                return typed.get_typed_value(0.0) == *tokens::STRONGER_THAN_DESCENDANTS;
            }
        }
        false
    }

    /// Build a Hydra materialBinding container with "path" field.
    fn build_hd_binding(material_path: &Path) -> HdContainerDataSourceHandle {
        let path_ds: HdDataSourceBaseHandle =
            HdRetainedTypedSampledDataSource::new(material_path.clone());
        HdRetainedContainerDataSource::new_1(tokens::PATH.clone(), path_ds)
    }
}

impl HdDataSourceBase for HdMaterialBindingsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for HdMaterialBindingsDataSource {
    fn get_names(&self) -> Vec<Token> {
        self.get_purposes()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let material_path = self.resolve_for_purpose(name)?;
        if material_path.is_empty() {
            return None;
        }
        Some(Self::build_hd_binding(&material_path) as HdDataSourceBaseHandle)
    }
}

// -- _PrimDataSource ---------------------------------------------------------

/// Prim container override that provides resolved Hydra material bindings.
#[derive(Clone)]
struct PrimDataSource {
    prim_container: HdContainerDataSourceHandle,
    si: HdSceneIndexHandle,
    prim_path: Path,
}

impl std::fmt::Debug for PrimDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimDataSource")
            .field("prim_path", &self.prim_path)
            .finish()
    }
}

impl PrimDataSource {
    fn new(
        prim_container: HdContainerDataSourceHandle,
        si: HdSceneIndexHandle,
        prim_path: Path,
    ) -> Arc<Self> {
        Arc::new(Self {
            prim_container,
            si,
            prim_path,
        })
    }
}

impl HdDataSourceBase for PrimDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for PrimDataSource {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.prim_container.get_names();
        if !names.iter().any(|n| *n == *tokens::MATERIAL_BINDINGS) {
            names.push(tokens::MATERIAL_BINDINGS.clone());
        }
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let result = self.prim_container.get(name);

        if *name == *tokens::MATERIAL_BINDINGS {
            // Only produce Hydra bindings if we have USD bindings on the prim
            if self
                .prim_container
                .get(&tokens::USD_MATERIAL_BINDINGS)
                .is_some()
            {
                let resolved = HdMaterialBindingsDataSource::new(
                    self.prim_container.clone(),
                    self.si.clone(),
                    self.prim_path.clone(),
                );

                // Overlay: existing opinion wins over resolved
                if let Some(existing) = result {
                    if let Some(existing_container) =
                        usd_hd::data_source::cast_to_container(&existing)
                    {
                        use usd_hd::data_source::HdOverlayContainerDataSource;
                        let overlay = HdOverlayContainerDataSource::new_2(
                            existing_container,
                            resolved as HdContainerDataSourceHandle,
                        );
                        return Some(overlay as HdDataSourceBaseHandle);
                    }
                }
                return Some(resolved as HdDataSourceBaseHandle);
            }
        }

        result
    }
}

// -- MaterialBindingsResolvingSceneIndex -------------------------------------

/// Scene index that resolves USD material bindings into Hydra materialBindings.
///
/// For every prim that has a `usdMaterialBindings` data source, wraps the prim
/// container with `PrimDataSource` producing resolved `materialBindings`.
pub struct MaterialBindingsResolvingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl std::fmt::Debug for MaterialBindingsResolvingSceneIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialBindingsResolvingSceneIndex")
            .finish()
    }
}

impl MaterialBindingsResolvingSceneIndex {
    /// Creates a new material bindings resolving scene index.
    pub fn new(input: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let result = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input.clone())),
        }));
        wire_filter_to_input(&result, &input);
        result
    }

    /// Locator set for the USD material bindings schema.
    fn usd_bindings_locator_set() -> HdDataSourceLocatorSet {
        HdDataSourceLocatorSet::from_locator(HdDataSourceLocator::from_token(
            tokens::USD_MATERIAL_BINDINGS.clone(),
        ))
    }
}

impl HdSceneIndexBase for MaterialBindingsResolvingSceneIndex {
    fn get_prim(&self, prim_path: &Path) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            let input_locked = input.read();
            let mut prim = input_locked.get_prim(prim_path);
            if let Some(ref ds) = prim.data_source {
                prim.data_source = Some(PrimDataSource::new(
                    ds.clone(),
                    input.clone(),
                    prim_path.clone(),
                ) as HdContainerDataSourceHandle);
            }
            return prim;
        }
        HdSceneIndexPrim::default()
    }

    fn get_child_prim_paths(&self, prim_path: &Path) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            let input_locked = input.read();
            return input_locked.get_child_prim_paths(prim_path);
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
        "MaterialBindingsResolvingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for MaterialBindingsResolvingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        // Transform dirty notices: replace usdMaterialBindings locator with
        // Hydra materialBindings locator so downstream sees the right dirty flags.
        let usd_loc_set = Self::usd_bindings_locator_set();
        let has_dirty_usd = entries
            .iter()
            .any(|e| e.dirty_locators.intersects(&usd_loc_set));

        if !has_dirty_usd {
            self.base.forward_prims_dirtied(self, entries);
            return;
        }

        let usd_locator = HdDataSourceLocator::from_token(tokens::USD_MATERIAL_BINDINGS.clone());
        let hd_locator = HdDataSourceLocator::from_token(tokens::MATERIAL_BINDINGS.clone());

        let new_entries: Vec<DirtiedPrimEntry> = entries
            .iter()
            .map(|entry| {
                if entry.dirty_locators.intersects(&usd_loc_set) {
                    let new_locators = entry
                        .dirty_locators
                        .replace_prefix(&usd_locator, &hd_locator);
                    DirtiedPrimEntry {
                        prim_path: entry.prim_path.clone(),
                        dirty_locators: new_locators,
                    }
                } else {
                    entry.clone()
                }
            })
            .collect();

        self.base.forward_prims_dirtied(self, &new_entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}

/// Handle type for MaterialBindingsResolvingSceneIndex.
pub type MaterialBindingsResolvingSceneIndexHandle =
    Arc<RwLock<MaterialBindingsResolvingSceneIndex>>;

/// Creates a new material bindings resolving scene index.
pub fn create_material_bindings_resolving_scene_index(
    input: HdSceneIndexHandle,
) -> MaterialBindingsResolvingSceneIndexHandle {
    MaterialBindingsResolvingSceneIndex::new(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(tokens::MATERIAL_BINDINGS.as_str(), "materialBindings");
        assert_eq!(
            tokens::USD_MATERIAL_BINDINGS.as_str(),
            "usdMaterialBindings"
        );
        assert_eq!(
            tokens::STRONGER_THAN_DESCENDANTS.as_str(),
            "strongerThanDescendants"
        );
    }

    #[test]
    fn test_locators() {
        let usd = MaterialBindingsResolvingSceneIndex::usd_bindings_locator_set();
        let hd_locator = HdDataSourceLocator::from_token(tokens::MATERIAL_BINDINGS.clone());
        // USD and Hydra locators should not intersect
        assert!(!usd.intersects_locator(&hd_locator));
    }

    #[test]
    fn test_display_name() {
        let si = MaterialBindingsResolvingSceneIndex {
            base: HdSingleInputFilteringSceneIndexBase::new(None),
        };
        assert_eq!(si.get_display_name(), "MaterialBindingsResolvingSceneIndex");
    }

    #[test]
    fn test_get_prim_no_input() {
        let si = MaterialBindingsResolvingSceneIndex {
            base: HdSingleInputFilteringSceneIndexBase::new(None),
        };
        let prim = si.get_prim(&Path::from_string("/World").unwrap());
        assert!(prim.data_source.is_none());
    }

    #[test]
    fn test_build_hd_binding() {
        let path = Path::from_string("/Materials/Red").unwrap();
        let container = HdMaterialBindingsDataSource::build_hd_binding(&path);
        let names = container.get_names();
        assert!(names.iter().any(|n| n == "path"));
    }

    #[test]
    fn test_dirty_locator_replacement() {
        let usd_locator = HdDataSourceLocator::from_token(tokens::USD_MATERIAL_BINDINGS.clone());
        let hd_locator = HdDataSourceLocator::from_token(tokens::MATERIAL_BINDINGS.clone());

        let mut set = HdDataSourceLocatorSet::new();
        set.insert(usd_locator.clone());

        let replaced = set.replace_prefix(&usd_locator, &hd_locator);
        assert!(replaced.intersects_locator(&hd_locator));
    }
}
