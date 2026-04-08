//! Legacy display style override scene index.
//!
//! Port of pxr/imaging/hdsi/legacyDisplayStyleOverrideSceneIndex.
//!
//! Overrides the legacy display style (refine level and cull style) for each prim.

use crate::utils;
use parking_lot::RwLock;
use std::sync::Arc;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdOverlayContainerDataSource,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
};
use usd_hd::scene_index::base::SceneIndexDelegate;
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_hd::tokens::{CULL_STYLE, DISPLAY_STYLE, REFINE_LEVEL};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Optional int for refine level (C++ OptionalInt).
#[derive(Debug, Clone)]
pub struct OptionalInt {
    /// Whether a value is set.
    pub has_value: bool,
    /// The value when has_value is true.
    pub value: i32,
}

impl OptionalInt {
    /// Creates an empty optional.
    pub fn none() -> Self {
        Self {
            has_value: false,
            value: 0,
        }
    }

    /// Creates an optional with a value.
    pub fn some(value: i32) -> Self {
        Self {
            has_value: true,
            value,
        }
    }
}

impl PartialEq for OptionalInt {
    fn eq(&self, other: &Self) -> bool {
        if !self.has_value && !other.has_value {
            return true;
        }
        self.has_value == other.has_value && self.value == other.value
    }
}

impl Eq for OptionalInt {}

/// Shared style info for overlay data sources.
#[derive(Debug)]
struct StyleInfo {
    refine_level: OptionalInt,
    refine_level_ds: Option<HdDataSourceBaseHandle>,
    cull_style_fallback: TfToken,
    cull_style_fallback_ds: Option<HdDataSourceBaseHandle>,
}

/// Data source for displayStyle that provides refineLevel.
#[derive(Debug, Clone)]
struct RefineLevelDataSource {
    style_info: Arc<RwLock<StyleInfo>>,
}

impl RefineLevelDataSource {
    fn new(style_info: Arc<RwLock<StyleInfo>>) -> Arc<Self> {
        Arc::new(Self { style_info })
    }
}

impl HdContainerDataSource for RefineLevelDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        vec![REFINE_LEVEL.clone()]
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if name.as_str() == REFINE_LEVEL.as_str() {
            let guard = self.style_info.read();
            return guard.refine_level_ds.clone();
        }
        None
    }
}

impl usd_hd::data_source::HdDataSourceBase for RefineLevelDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            style_info: Arc::clone(&self.style_info),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        None
    }
}

/// Data source for displayStyle that provides cullStyle.
#[derive(Debug, Clone)]
struct CullStyleFallbackDataSource {
    style_info: Arc<RwLock<StyleInfo>>,
}

impl CullStyleFallbackDataSource {
    fn new(style_info: Arc<RwLock<StyleInfo>>) -> Arc<Self> {
        Arc::new(Self { style_info })
    }
}

impl HdContainerDataSource for CullStyleFallbackDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        vec![CULL_STYLE.clone()]
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        if name.as_str() == CULL_STYLE.as_str() {
            let guard = self.style_info.read();
            return guard.cull_style_fallback_ds.clone();
        }
        None
    }
}

impl usd_hd::data_source::HdDataSourceBase for CullStyleFallbackDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            style_info: Arc::clone(&self.style_info),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        None
    }
}

fn display_style_locator() -> HdDataSourceLocator {
    HdDataSourceLocator::from_token(DISPLAY_STYLE.clone())
}

fn refine_level_locator() -> HdDataSourceLocator {
    display_style_locator().append(&REFINE_LEVEL)
}

/// Scene index that overrides legacy display style for every prim.
pub struct HdsiLegacyDisplayStyleOverrideSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    style_info: Arc<RwLock<StyleInfo>>,
    overlay_ds: HdContainerDataSourceHandle,
    underlay_ds: HdContainerDataSourceHandle,
}

impl HdsiLegacyDisplayStyleOverrideSceneIndex {
    /// Creates a new legacy display style override scene index.
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let style_info = Arc::new(RwLock::new(StyleInfo {
            refine_level: OptionalInt::none(),
            refine_level_ds: None,
            cull_style_fallback: TfToken::new(""),
            cull_style_fallback_ds: None,
        }));

        let refine_level_ds = HdRetainedContainerDataSource::new_1(
            DISPLAY_STYLE.clone(),
            RefineLevelDataSource::new(Arc::clone(&style_info)) as HdDataSourceBaseHandle,
        );
        let cull_style_ds = HdRetainedContainerDataSource::new_1(
            DISPLAY_STYLE.clone(),
            CullStyleFallbackDataSource::new(Arc::clone(&style_info)) as HdDataSourceBaseHandle,
        );

        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            style_info: Arc::clone(&style_info),
            overlay_ds: refine_level_ds,
            underlay_ds: cull_style_ds,
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

    /// Sets the refine level for every prim.
    ///
    /// If `None` (empty optional), a null data source is returned for the locator.
    pub fn set_refine_level(this: &Arc<RwLock<Self>>, refine_level: OptionalInt) {
        let locators = {
            let guard = this.write();
            let mut style = guard.style_info.write();
            if refine_level == style.refine_level {
                return;
            }
            style.refine_level = refine_level.clone();
            style.refine_level_ds = if refine_level.has_value {
                Some(
                    HdRetainedTypedSampledDataSource::<i32>::new(refine_level.value)
                        as HdDataSourceBaseHandle,
                )
            } else {
                None
            };
            let mut set = HdDataSourceLocatorSet::new();
            set.insert(refine_level_locator());
            set
        };
        Self::dirty_all_prims(this, &locators);
    }

    /// Sets the cull style fallback for every prim.
    pub fn set_cull_style_fallback(this: &Arc<RwLock<Self>>, cull_style_fallback: TfToken) {
        let locators = {
            let guard = this.write();
            let mut style = guard.style_info.write();
            if cull_style_fallback == style.cull_style_fallback {
                return;
            }
            style.cull_style_fallback = cull_style_fallback.clone();
            style.cull_style_fallback_ds = if !cull_style_fallback.as_str().is_empty() {
                Some(
                    HdRetainedTypedSampledDataSource::<TfToken>::new(cull_style_fallback)
                        as HdDataSourceBaseHandle,
                )
            } else {
                None
            };
            let mut set = HdDataSourceLocatorSet::new();
            set.insert(display_style_locator());
            set
        };
        Self::dirty_all_prims(this, &locators);
    }

    fn dirty_all_prims(this: &Arc<RwLock<Self>>, locators: &HdDataSourceLocatorSet) {
        let input_scene = {
            let guard = this.read();
            if !guard.base.base().is_observed() {
                return;
            }
            guard.base.get_input_scene().cloned()
        };
        let input_scene = match input_scene {
            Some(s) => s,
            None => return,
        };

        let root = SdfPath::absolute_root();
        let paths = utils::collect_prim_paths(&input_scene, &root);

        let entries: Vec<DirtiedPrimEntry> = paths
            .into_iter()
            .map(|p| DirtiedPrimEntry::new(p, locators.clone()))
            .collect();

        let guard = this.read();
        let delegate = SceneIndexDelegate(Arc::clone(this));
        let sender = &delegate as &dyn HdSceneIndexBase;
        guard.base.base().send_prims_dirtied(sender, &entries);
    }
}

impl HdSceneIndexBase for HdsiLegacyDisplayStyleOverrideSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let mut prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::default()
        };

        if prim.data_source.is_some() {
            prim.data_source = Some(HdOverlayContainerDataSource::new_3(
                self.overlay_ds.clone(),
                prim.data_source.expect("checked"),
                self.underlay_ds.clone(),
            ));
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
        "Legacy Display Style Override Scene Index".to_string()
    }
}

impl FilteringObserverTarget for HdsiLegacyDisplayStyleOverrideSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if self.base.base().is_observed() {
            self.base.forward_prims_added(self, entries);
        }
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if self.base.base().is_observed() {
            self.base.forward_prims_removed(self, entries);
        }
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if self.base.base().is_observed() {
            self.base.forward_prims_dirtied(self, entries);
        }
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
