
//! Scene globals scene index.
//!
//! Port of pxr/imaging/hdsi/sceneGlobalsSceneIndex.
//!
//! Populates the "sceneGlobals" data source at the root prim per
//! HdSceneGlobalsSchema and provides public API to mutate it.

use std::sync::{Arc, Weak};
use parking_lot::RwLock;
use usd_hd::data_source::{
    HdContainerDataSource, HdDataSourceBaseHandle, HdDataSourceLocator, HdDataSourceLocatorSet,
    HdOverlayContainerDataSource, HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
};
use usd_hd::scene_index::base::SceneIndexDelegate;
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Schema token for scene globals container.
const SCENE_GLOBALS: &str = "sceneGlobals";
/// Schema tokens for scene globals members.
const ACTIVE_RENDER_PASS_PRIM: &str = "activeRenderPassPrim";
const ACTIVE_RENDER_SETTINGS_PRIM: &str = "activeRenderSettingsPrim";
const PRIMARY_CAMERA_PRIM: &str = "primaryCameraPrim";
const CURRENT_FRAME: &str = "currentFrame";
const TIME_CODES_PER_SECOND: &str = "timeCodesPerSecond";
const SCENE_STATE_ID: &str = "sceneStateId";

fn default_prim_path() -> SdfPath {
    SdfPath::absolute_root()
}

fn scene_globals_locator() -> HdDataSourceLocator {
    HdDataSourceLocator::from_token(TfToken::new(SCENE_GLOBALS))
}

fn active_render_pass_prim_locator() -> HdDataSourceLocator {
    scene_globals_locator().append(&TfToken::new(ACTIVE_RENDER_PASS_PRIM))
}

fn active_render_settings_prim_locator() -> HdDataSourceLocator {
    scene_globals_locator().append(&TfToken::new(ACTIVE_RENDER_SETTINGS_PRIM))
}

fn primary_camera_prim_locator() -> HdDataSourceLocator {
    scene_globals_locator().append(&TfToken::new(PRIMARY_CAMERA_PRIM))
}

fn current_frame_locator() -> HdDataSourceLocator {
    scene_globals_locator().append(&TfToken::new(CURRENT_FRAME))
}

fn time_codes_per_second_locator() -> HdDataSourceLocator {
    scene_globals_locator().append(&TfToken::new(TIME_CODES_PER_SECOND))
}

fn scene_state_id_locator() -> HdDataSourceLocator {
    scene_globals_locator().append(&TfToken::new(SCENE_STATE_ID))
}

/// Data source that reads scene globals from the scene index.
#[derive(Clone)]
struct SceneGlobalsDataSource {
    owner: Weak<RwLock<HdsiSceneGlobalsSceneIndex>>,
}

impl SceneGlobalsDataSource {
    fn new(owner: Weak<RwLock<HdsiSceneGlobalsSceneIndex>>) -> Arc<Self> {
        Arc::new(Self { owner })
    }
}

impl std::fmt::Debug for SceneGlobalsDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SceneGlobalsDataSource").finish()
    }
}

impl usd_hd::data_source::HdDataSourceBase for SceneGlobalsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            owner: Weak::clone(&self.owner),
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

impl HdContainerDataSource for SceneGlobalsDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        vec![
            TfToken::new(ACTIVE_RENDER_PASS_PRIM),
            TfToken::new(ACTIVE_RENDER_SETTINGS_PRIM),
            TfToken::new(PRIMARY_CAMERA_PRIM),
            TfToken::new(CURRENT_FRAME),
            TfToken::new(TIME_CODES_PER_SECOND),
            TfToken::new(SCENE_STATE_ID),
        ]
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let owner = self.owner.upgrade()?;
        let guard = owner.read();

        let name_str = name.as_str();
        if name_str == ACTIVE_RENDER_PASS_PRIM {
            Some(HdRetainedTypedSampledDataSource::<SdfPath>::new(
                guard.active_render_pass_prim_path.clone(),
            ) as HdDataSourceBaseHandle)
        } else if name_str == ACTIVE_RENDER_SETTINGS_PRIM {
            guard.active_render_settings_prim_path.as_ref().map(|p| {
                HdRetainedTypedSampledDataSource::<SdfPath>::new(p.clone())
                    as HdDataSourceBaseHandle
            })
        } else if name_str == PRIMARY_CAMERA_PRIM {
            guard.primary_camera_prim_path.as_ref().map(|p| {
                HdRetainedTypedSampledDataSource::<SdfPath>::new(p.clone())
                    as HdDataSourceBaseHandle
            })
        } else if name_str == CURRENT_FRAME {
            Some(HdRetainedTypedSampledDataSource::<f64>::new(guard.time) as HdDataSourceBaseHandle)
        } else if name_str == TIME_CODES_PER_SECOND {
            Some(
                HdRetainedTypedSampledDataSource::<f64>::new(guard.time_codes_per_second)
                    as HdDataSourceBaseHandle,
            )
        } else if name_str == SCENE_STATE_ID {
            Some(
                HdRetainedTypedSampledDataSource::<i32>::new(guard.scene_state_id)
                    as HdDataSourceBaseHandle,
            )
        } else {
            None
        }
    }
}

fn is_equal_time_code(t0: f64, t1: f64) -> bool {
    if t0.is_nan() && t1.is_nan() {
        return true;
    }
    t0 == t1
}

/// Scene index that populates sceneGlobals data source and provides mutation API.
pub struct HdsiSceneGlobalsSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    self_weak: Weak<RwLock<Self>>,
    active_render_pass_prim_path: SdfPath,
    active_render_settings_prim_path: Option<SdfPath>,
    primary_camera_prim_path: Option<SdfPath>,
    time: f64,
    time_codes_per_second: f64,
    scene_state_id: i32,
}

impl HdsiSceneGlobalsSceneIndex {
    /// Creates a new scene globals scene index.
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let scene = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            self_weak: Weak::new(),
            active_render_pass_prim_path: SdfPath::default(),
            active_render_settings_prim_path: None,
            primary_camera_prim_path: None,
            time: f64::NAN,
            time_codes_per_second: 24.0,
            scene_state_id: 0,
        }));

        scene.write().self_weak = Arc::downgrade(&scene);

        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&scene) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene.read().add_observer(Arc::new(filtering_observer));
        }
        scene
    }

    /// Sets the active render pass prim path and notifies observers if changed.
    pub fn set_active_render_pass_prim_path(this: &Arc<RwLock<Self>>, path: SdfPath) {
        let observed = {
            let mut guard = this.write();
            if guard.active_render_pass_prim_path == path {
                return;
            }
            guard.active_render_pass_prim_path = path;
            guard.base.base().is_observed()
        };
        if observed {
            Self::send_dirtied(this, &active_render_pass_prim_locator());
        }
    }

    /// Sets the active render settings prim path and notifies observers if changed.
    pub fn set_active_render_settings_prim_path(this: &Arc<RwLock<Self>>, path: SdfPath) {
        let observed = {
            let mut guard = this.write();
            if guard.active_render_settings_prim_path.as_ref() == Some(&path) {
                return;
            }
            guard.active_render_settings_prim_path = Some(path);
            guard.base.base().is_observed()
        };
        if observed {
            Self::send_dirtied(this, &active_render_settings_prim_locator());
        }
    }

    /// Sets the primary camera prim path and notifies observers if changed.
    pub fn set_primary_camera_prim_path(this: &Arc<RwLock<Self>>, path: SdfPath) {
        let observed = {
            let mut guard = this.write();
            if guard.primary_camera_prim_path.as_ref() == Some(&path) {
                return;
            }
            guard.primary_camera_prim_path = Some(path);
            guard.base.base().is_observed()
        };
        if observed {
            Self::send_dirtied(this, &primary_camera_prim_locator());
        }
    }

    /// Sets the current frame and notifies observers if changed.
    pub fn set_current_frame(this: &Arc<RwLock<Self>>, time: f64) {
        let observed = {
            let mut guard = this.write();
            if is_equal_time_code(guard.time, time) {
                return;
            }
            guard.time = time;
            guard.base.base().is_observed()
        };
        if observed {
            Self::send_dirtied(this, &current_frame_locator());
        }
    }

    /// Sets time codes per second and notifies observers if changed.
    pub fn set_time_codes_per_second(this: &Arc<RwLock<Self>>, tps: f64) {
        let observed = {
            let mut guard = this.write();
            if guard.time_codes_per_second == tps {
                return;
            }
            guard.time_codes_per_second = tps;
            guard.base.base().is_observed()
        };
        if observed {
            Self::send_dirtied(this, &time_codes_per_second_locator());
        }
    }

    /// Sets the scene state id and notifies observers if changed.
    pub fn set_scene_state_id(this: &Arc<RwLock<Self>>, id: i32) {
        let observed = {
            let mut guard = this.write();
            if guard.scene_state_id == id {
                return;
            }
            guard.scene_state_id = id;
            guard.base.base().is_observed()
        };
        if observed {
            Self::send_dirtied(this, &scene_state_id_locator());
        }
    }

    fn send_dirtied(this: &Arc<RwLock<Self>>, locator: &HdDataSourceLocator) {
        let mut set = HdDataSourceLocatorSet::new();
        set.insert(locator.clone());
        let entries = vec![DirtiedPrimEntry::new(default_prim_path(), set)];
        let guard = this.read();
        let delegate = SceneIndexDelegate(Arc::clone(this));
        let sender = &delegate as &dyn HdSceneIndexBase;
        guard.base.base().send_prims_dirtied(sender, &entries);
    }
}

impl HdSceneIndexBase for HdsiSceneGlobalsSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let mut prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::default()
        };

        if *prim_path == default_prim_path() {
            let scene_globals_container = HdRetainedContainerDataSource::new_1(
                TfToken::new(SCENE_GLOBALS),
                SceneGlobalsDataSource::new(Weak::clone(&self.self_weak)) as HdDataSourceBaseHandle,
            );
            prim.data_source = Some(
                HdOverlayContainerDataSource::overlayed(
                    Some(scene_globals_container),
                    prim.data_source,
                )
                .expect("overlay"),
            );
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
        "Scene Globals Scene Index".to_string()
    }
}

impl FilteringObserverTarget for HdsiSceneGlobalsSceneIndex {
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
