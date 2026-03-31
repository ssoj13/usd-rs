//! ExtentResolvingSceneIndex - resolves extent from extentsHint.
//!
//! Port of pxr/usdImaging/usdImaging/extentResolvingSceneIndex.cpp
//!
//! If a prim has no authored `extent` but has `extentsHint`, this scene index
//! computes the extent from the hints for the configured purposes (geometry,
//! render, proxy by default). When multiple purposes are configured, the per-
//! purpose extents are unioned into a single bounding box.

use std::collections::HashSet;
use std::sync::Arc;
use parking_lot::RwLock;
use usd_gf::Vec3d;
use usd_hd::data_source::{
    HdDataSourceBaseHandle, HdDataSourceLocator, HdDataSourceLocatorSet,
    HdRetainedContainerDataSource, HdRetainedSampledDataSource, HdRetainedSmallVectorDataSource,
    HdRetainedTypedSampledDataSource, HdSampledDataSource, HdTypedSampledDataSource,
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
use usd_vt::Value;

// -- tokens ------------------------------------------------------------------

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static EXTENT: LazyLock<Token> = LazyLock::new(|| Token::new("extent"));
    pub static EXTENTS_HINT: LazyLock<Token> = LazyLock::new(|| Token::new("extentsHint"));
    pub static PURPOSES: LazyLock<Token> = LazyLock::new(|| Token::new("purposes"));
    pub static MIN: LazyLock<Token> = LazyLock::new(|| Token::new("min"));
    pub static MAX: LazyLock<Token> = LazyLock::new(|| Token::new("max"));
}

// -- Info --------------------------------------------------------------------

/// Shared configuration extracted from input args at construction time.
#[derive(Debug)]
struct Info {
    /// Purposes for which extentsHint should be evaluated.
    purposes: HashSet<Token>,
}

impl Info {
    fn new(input_args: &Option<HdContainerDataSourceHandle>) -> Self {
        let mut purposes = HashSet::new();

        if let Some(args) = input_args {
            if let Some(purposes_ds) = args.get(&tokens::PURPOSES) {
                // Try to read as a vector data source
                if let Some(vec_ds) = usd_hd::data_source::cast_to_vector(&purposes_ds) {
                    let n = vec_ds.get_num_elements();
                    for i in 0..n {
                        if let Some(elem) = vec_ds.get_element(i) {
                            let any = elem.as_any();
                            if let Some(sampled) = any.downcast_ref::<HdRetainedSampledDataSource>()
                            {
                                if let Some(token) = sampled.get_value(0.0).get::<Token>() {
                                    purposes.insert(token.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Default: geometry
        if purposes.is_empty() {
            purposes.insert(Token::new("geometry"));
        }

        Self { purposes }
    }
}

// -- _PrimSource -------------------------------------------------------------

/// Prim data source overlay that provides `extent` from `extentsHint`.
///
/// If the prim already has an authored `extent`, it passes through.
/// Otherwise, if `extentsHint` exists, computes extent from the configured
/// purposes, unioning per-purpose bounding boxes when multiple purposes
/// are configured.
#[derive(Clone, Debug)]
struct PrimSource {
    prim_source: HdContainerDataSourceHandle,
    info: Arc<Info>,
}

impl PrimSource {
    fn new(prim_source: HdContainerDataSourceHandle, info: Arc<Info>) -> Arc<Self> {
        Arc::new(Self { prim_source, info })
    }

    /// Check if prim has extentsHint data.
    fn has_extents_hint(&self) -> bool {
        self.prim_source.get(&tokens::EXTENTS_HINT).is_some()
    }

    /// Compute extent from extentsHint for the configured purposes.
    fn get_extent_from_hints(&self) -> Option<HdContainerDataSourceHandle> {
        if self.info.purposes.is_empty() {
            return None;
        }

        let hints_ds = self.prim_source.get(&tokens::EXTENTS_HINT)?;
        let hints_container = usd_hd::data_source::cast_to_container(&hints_ds)?;

        // Single purpose: return the extent directly from that purpose
        if self.info.purposes.len() == 1 {
            let purpose = self.info.purposes.iter().next().unwrap();
            let extent_ds = hints_container.get(purpose)?;
            if let Some(extent_container) = usd_hd::data_source::cast_to_container(&extent_ds) {
                return Some(extent_container);
            }
            return None;
        }

        // Multiple purposes: union the bounding boxes
        let mut bbox_min = Vec3d::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut bbox_max = Vec3d::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        let mut found_any = false;

        for purpose in &self.info.purposes {
            if let Some(extent_ds) = hints_container.get(purpose) {
                if let Some(extent_container) = usd_hd::data_source::cast_to_container(&extent_ds) {
                    if let (Some(min_ds), Some(max_ds)) = (
                        extent_container.get(&tokens::MIN),
                        extent_container.get(&tokens::MAX),
                    ) {
                        let min_any = min_ds.as_any();
                        let max_any = max_ds.as_any();
                        if let (Some(min_typed), Some(max_typed)) = (
                            min_any.downcast_ref::<HdRetainedTypedSampledDataSource<Vec3d>>(),
                            max_any.downcast_ref::<HdRetainedTypedSampledDataSource<Vec3d>>(),
                        ) {
                            let min_val = min_typed.get_typed_value(0.0);
                            let max_val = max_typed.get_typed_value(0.0);
                            bbox_min.x = bbox_min.x.min(min_val.x);
                            bbox_min.y = bbox_min.y.min(min_val.y);
                            bbox_min.z = bbox_min.z.min(min_val.z);
                            bbox_max.x = bbox_max.x.max(max_val.x);
                            bbox_max.y = bbox_max.y.max(max_val.y);
                            bbox_max.z = bbox_max.z.max(max_val.z);
                            found_any = true;
                        }
                    }
                }
            }
        }

        if !found_any {
            return None;
        }

        // Build extent container with min/max
        let min_ds: HdDataSourceBaseHandle = HdRetainedTypedSampledDataSource::new(bbox_min);
        let max_ds: HdDataSourceBaseHandle = HdRetainedTypedSampledDataSource::new(bbox_max);
        Some(HdRetainedContainerDataSource::new_2(
            tokens::MIN.clone(),
            min_ds,
            tokens::MAX.clone(),
            max_ds,
        ))
    }
}

impl HdDataSourceBase for PrimSource {
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

impl HdContainerDataSource for PrimSource {
    fn get_names(&self) -> Vec<Token> {
        let mut result = self.prim_source.get_names();
        if self.has_extents_hint() && !result.iter().any(|n| *n == *tokens::EXTENT) {
            result.push(tokens::EXTENT.clone());
        }
        result
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // Pass through existing data first
        if let Some(result) = self.prim_source.get(name) {
            return Some(result);
        }

        // Use extentsHint as fallback for extent
        if *name == *tokens::EXTENT {
            return self
                .get_extent_from_hints()
                .map(|c| c as HdDataSourceBaseHandle);
        }

        None
    }
}

// -- ExtentResolvingSceneIndex -----------------------------------------------

/// Scene index that resolves `extent` from `extentsHint` for prims that lack
/// an authored extent.
pub struct ExtentResolvingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    info: Arc<Info>,
}

impl std::fmt::Debug for ExtentResolvingSceneIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtentResolvingSceneIndex").finish()
    }
}

impl ExtentResolvingSceneIndex {
    /// Creates a new extent resolving scene index.
    pub fn new(input: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        Self::new_with_input_args(input, None)
    }

    /// Creates with input args (may contain "purposes" vector).
    pub fn new_with_input_args(
        input: HdSceneIndexHandle,
        input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let info = Arc::new(Info::new(&input_args));
        let result = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input.clone())),
            info,
        }));
        wire_filter_to_input(&result, &input);
        result
    }
}

impl HdSceneIndexBase for ExtentResolvingSceneIndex {
    fn get_prim(&self, prim_path: &Path) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            let input_locked = input.read();
            let mut prim = input_locked.get_prim(prim_path);
            if let Some(ref ds) = prim.data_source {
                prim.data_source = Some(PrimSource::new(ds.clone(), self.info.clone())
                    as HdContainerDataSourceHandle);
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
        "ExtentResolvingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for ExtentResolvingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        // If extentsHint is dirty but extent is not, add extent to dirty set.
        let extents_hint_loc = HdDataSourceLocatorSet::from_locator(
            HdDataSourceLocator::from_token(tokens::EXTENTS_HINT.clone()),
        );
        let extent_loc = HdDataSourceLocatorSet::from_locator(HdDataSourceLocator::from_token(
            tokens::EXTENT.clone(),
        ));

        let needs_transform = entries.iter().any(|e| {
            e.dirty_locators.intersects(&extents_hint_loc)
                && !e.dirty_locators.intersects(&extent_loc)
        });

        if !needs_transform {
            self.base.forward_prims_dirtied(self, entries);
            return;
        }

        let extent_locator = HdDataSourceLocator::from_token(tokens::EXTENT.clone());
        let new_entries: Vec<DirtiedPrimEntry> = entries
            .iter()
            .map(|entry| {
                if entry.dirty_locators.intersects(&extents_hint_loc)
                    && !entry.dirty_locators.intersects(&extent_loc)
                {
                    let mut new_locators = entry.dirty_locators.clone();
                    new_locators.insert(extent_locator.clone());
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

/// Resolved extent data source with min/max accessors.
#[derive(Clone)]
pub struct ResolvedExtentDataSource {
    min: [f64; 3],
    max: [f64; 3],
}

impl std::fmt::Debug for ResolvedExtentDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedExtentDataSource")
            .field("min", &self.min)
            .field("max", &self.max)
            .finish()
    }
}

impl ResolvedExtentDataSource {
    /// Creates a new resolved extent data source.
    pub fn new(min: [f64; 3], max: [f64; 3]) -> Self {
        Self { min, max }
    }

    /// Returns the minimum point.
    pub fn min(&self) -> [f64; 3] {
        self.min
    }

    /// Returns the maximum point.
    pub fn max(&self) -> [f64; 3] {
        self.max
    }
}

impl usd_hd::HdDataSourceBase for ResolvedExtentDataSource {
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

impl HdContainerDataSource for ResolvedExtentDataSource {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::MIN.clone(), tokens::MAX.clone()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::MIN {
            let v = Vec3d::new(self.min[0], self.min[1], self.min[2]);
            return Some(HdRetainedTypedSampledDataSource::new(v) as HdDataSourceBaseHandle);
        }
        if *name == *tokens::MAX {
            let v = Vec3d::new(self.max[0], self.max[1], self.max[2]);
            return Some(HdRetainedTypedSampledDataSource::new(v) as HdDataSourceBaseHandle);
        }
        None
    }
}

/// Handle type for ExtentResolvingSceneIndex.
pub type ExtentResolvingSceneIndexHandle = Arc<RwLock<ExtentResolvingSceneIndex>>;

/// Builds default input args for extent resolving (purposes: geometry, render, proxy).
pub fn extent_resolving_input_args() -> HdContainerDataSourceHandle {
    use usd_hd::tokens::{RENDER_TAG_GEOMETRY, RENDER_TAG_PROXY, RENDER_TAG_RENDER};

    let purpose_ds: Vec<HdDataSourceBaseHandle> = [
        RENDER_TAG_GEOMETRY.clone(),
        RENDER_TAG_RENDER.clone(),
        RENDER_TAG_PROXY.clone(),
    ]
    .iter()
    .map(|t| HdRetainedSampledDataSource::new(Value::from(t.clone())) as HdDataSourceBaseHandle)
    .collect();
    let purposes_vec: HdDataSourceBaseHandle = HdRetainedSmallVectorDataSource::new(&purpose_ds);
    let mut children = std::collections::HashMap::new();
    children.insert(tokens::PURPOSES.clone(), purposes_vec);
    HdRetainedContainerDataSource::new(children)
}

/// Creates a new extent resolving scene index.
pub fn create_extent_resolving_scene_index(
    input: HdSceneIndexHandle,
) -> ExtentResolvingSceneIndexHandle {
    create_extent_resolving_scene_index_with_args(input, Some(extent_resolving_input_args()))
}

/// Creates with input args.
pub fn create_extent_resolving_scene_index_with_args(
    input: HdSceneIndexHandle,
    input_args: Option<HdContainerDataSourceHandle>,
) -> ExtentResolvingSceneIndexHandle {
    ExtentResolvingSceneIndex::new_with_input_args(input, input_args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extent_data_source() {
        let ds = ResolvedExtentDataSource::new([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
        assert_eq!(ds.min(), [-1.0, -1.0, -1.0]);
        assert_eq!(ds.max(), [1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_extent_data_source_names() {
        let ds = ResolvedExtentDataSource::new([0.0; 3], [1.0; 3]);
        let names = ds.get_names();
        assert_eq!(names.len(), 2);
        assert!(names.iter().any(|n| n == "min"));
        assert!(names.iter().any(|n| n == "max"));
    }

    #[test]
    fn test_extent_data_source_get() {
        let ds = ResolvedExtentDataSource::new([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]);
        assert!(ds.get(&tokens::MIN).is_some());
        assert!(ds.get(&tokens::MAX).is_some());
        assert!(ds.get(&Token::new("nonexistent")).is_none());
    }

    #[test]
    fn test_info_default_purpose() {
        let info = Info::new(&None);
        assert!(info.purposes.contains(&Token::new("geometry")));
    }

    #[test]
    fn test_dirty_entry_transform() {
        let extents_hint_loc = HdDataSourceLocatorSet::from_locator(
            HdDataSourceLocator::from_token(tokens::EXTENTS_HINT.clone()),
        );
        let extent_loc = HdDataSourceLocatorSet::from_locator(HdDataSourceLocator::from_token(
            tokens::EXTENT.clone(),
        ));

        let mut set = HdDataSourceLocatorSet::new();
        set.insert(HdDataSourceLocator::from_token(
            tokens::EXTENTS_HINT.clone(),
        ));
        assert!(set.intersects(&extents_hint_loc));
        assert!(!set.intersects(&extent_loc));
    }
}
