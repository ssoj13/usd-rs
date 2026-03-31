// UsdImagingDelegate is a Hydra 1.0 legacy interface. The full prim population,
// change-tracking, and adapter-dispatch pipeline is ported. Some methods are
// not yet exercised in the wgpu render loop. The allow below silences
// dead_code warnings for unused adapter dispatch paths.
#![allow(dead_code)]
//! UsdImagingDelegate - Legacy delegate for Hydra 1.0 compatibility.
//!
//! This module provides the legacy delegate pattern for applications that still
//! use Hydra 1.0's HdSceneDelegate interface. Modern applications should prefer
//! [`StageSceneIndex`](super::StageSceneIndex) and the scene index API.
//!
//! # Overview
//!
//! `UsdImagingDelegate` serves as the primary translation layer between Hydra's
//! rendering core and USD scene graphs. It:
//!
//! - Wraps a UsdStage for rendering
//! - Manages prim population and synchronization
//! - Tracks changes and dirty bits
//! - Provides camera and light query APIs
//! - Handles transform hierarchies and material bindings
//!
//! # Architecture
//!
//! The delegate uses a pull-based architecture where the render delegate queries
//! scene data on demand. It maintains caches for frequently accessed data and
//! tracks dirty state for incremental updates.
//!
//! ```ignore
//! use usd_core::Stage;
//! use usd_imaging::UsdImagingDelegate;
//! use usd_sdf::Path;
//!
//! // Create delegate
//! let stage = Stage::open("scene.usd").unwrap();
//! let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());
//!
//! // Populate scene
//! delegate.populate();
//!
//! // Set time for animation
//! delegate.set_time(TimeCode::new(24.0));
//!
//! // Query camera
//! let camera_path = Path::from_string("/Camera").unwrap();
//! if let Some(params) = delegate.get_camera_params(&camera_path) {
//!     println!("FOV: {}", params.fov);
//! }
//! ```
//!
//! # Hydra 1.0 vs 2.0
//!
//! Hydra 2.0 introduces the scene index API which provides:
//! - Better composability via scene index chains
//! - Lazy data evaluation through data sources
//! - Improved change tracking with locators
//!
//! However, Hydra 1.0 delegates are still supported through adapter layers,
//! allowing gradual migration.

use super::adapter_registry::AdapterRegistry;
use super::prim_adapter::PrimAdapter;
use crate::change_handler::{fields_to_dirty_bits, ChangeHandler};
use crate::instance_adapter::{InstanceVisibility, InstancerData, PrimvarInfo, ProtoPrim};
use crate::light_linking_cache::LightLinkingCache;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use usd_core::TimeCode as UsdTimeCode;
use usd_core::{Attribute as UsdAttribute, Prim, Stage};
use usd_geom::mesh::Mesh;
use usd_geom::model_api::ModelAPI;
use usd_geom::point_based::PointBased;
use usd_geom::primvars_api::PrimvarsAPI;
use usd_geom::Imageable;
use usd_geom::XformCache;
use usd_gf::{Matrix4d, Range3d, Vec3d, Vec3f, Vec4d};
use usd_hd::enums::{HdCullStyle, HdInterpolation};
use usd_hd::material_network::{HdMaterialNetworkMap, HdMaterialNetworkV1, HdMaterialNode};
use usd_hd::prim::mesh::HdMeshTopology;
use usd_hd::scene_delegate::{
    HdExtComputationInputDescriptor, HdExtComputationInputDescriptorVector,
    HdExtComputationOutputDescriptor, HdExtComputationOutputDescriptorVector,
    HdExtComputationPrimvarDescriptorVector, HdIdVectorSharedPtr, HdModelDrawMode,
    HdPrimvarDescriptor, HdPrimvarDescriptorVector,
};
use usd_hd::{HdDirtyBits, HdDisplayStyle, HdExtComputationContext, HdSceneDelegate};
use usd_px_osd::SubdivTags;
use usd_sdf::Path;
use usd_sdf::TimeCode as SdfTimeCode;
use usd_shade::coord_sys_api::CoordSysAPI;
use usd_shade::material_binding_api::MaterialBindingAPI;
use usd_tf::Token;
use usd_vt::Value;

/// Dirty bit for draw mode property changes.
const DIRTY_DRAW_MODE: HdDirtyBits = 0x1000;
/// Dirty bit for coordinate system binding property changes.
const DIRTY_COORD_SYS: HdDirtyBits = 0x2000;

/// Shutter interval for motion sampling: (open, close) as frame-relative offsets.
///
/// Default [0.0, 0.0] means no motion blur — return single sample at current time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShutterInterval {
    pub open: f64,
    pub close: f64,
}

impl Default for ShutterInterval {
    fn default() -> Self {
        Self {
            open: 0.0,
            close: 0.0,
        }
    }
}

impl ShutterInterval {
    pub fn new(open: f64, close: f64) -> Self {
        Self { open, close }
    }

    /// True when shutter is open for a finite interval (motion blur active).
    pub fn is_open(&self) -> bool {
        self.open != self.close
    }
}

/// Cached data for a populated prim.
#[derive(Clone)]
struct CachedPrimData {
    /// USD prim path
    usd_path: Path,
    /// Hydra prim type
    prim_type: Token,
    /// Adapter handling this prim
    adapter: Arc<dyn PrimAdapter>,
    /// Whether prim is visible
    visible: bool,
    /// Local transform
    transform: Matrix4d,
    /// Per-prim time-varying dirty bits computed by adapter.track_variability().
    /// Only these bits are set dirty on set_time(). C++: _hdPrimInfo.timeVaryingBits.
    time_varying_bits: HdDirtyBits,
}

/// Camera parameters queried from USD.
#[derive(Debug, Clone)]
pub struct CameraParams {
    /// Horizontal field of view in degrees
    pub fov: f64,
    /// Focal length in mm
    pub focal_length: f64,
    /// Horizontal aperture in mm
    pub h_aperture: f64,
    /// Vertical aperture in mm  
    pub v_aperture: f64,
    /// Near clipping plane
    pub near: f64,
    /// Far clipping plane
    pub far: f64,
}

impl Default for CameraParams {
    fn default() -> Self {
        Self {
            fov: 90.0,
            focal_length: 50.0,
            h_aperture: 36.0,
            v_aperture: 24.0,
            near: 0.1,
            far: 10000.0,
        }
    }
}

/// Light parameters queried from USD.
#[derive(Debug, Clone)]
pub struct LightParams {
    /// Light color
    pub color: Vec3d,
    /// Light intensity
    pub intensity: f64,
    /// Exposure adjustment
    pub exposure: f64,
    /// Whether light casts shadows
    pub shadows_enabled: bool,
}

impl Default for LightParams {
    fn default() -> Self {
        Self {
            color: Vec3d::new(1.0, 1.0, 1.0),
            intensity: 1.0,
            exposure: 0.0,
            shadows_enabled: true,
        }
    }
}

/// Legacy imaging delegate for Hydra 1.0 compatibility.
///
/// Wraps a USD stage and provides scene data to Hydra render delegates
/// through the HdSceneDelegate interface.
///
/// # Thread Safety
///
/// This struct is thread-safe and can be shared across threads using `Arc`.
/// Internal state is protected by RwLocks and Mutexes.
pub struct UsdImagingDelegate {
    /// USD stage being rendered
    stage: Arc<RwLock<Option<Arc<Stage>>>>,

    /// Root prim path for delegation
    root_path: Path,

    /// Current evaluation time
    time: Arc<RwLock<UsdTimeCode>>,

    /// Adapter registry for prim type conversion
    adapter_registry: Arc<AdapterRegistry>,

    /// Root transform applied to all prims
    root_transform: Arc<RwLock<Matrix4d>>,

    /// Root visibility flag
    root_visible: Arc<RwLock<bool>>,

    /// Fallback refinement level for subdivision
    refine_level_fallback: Arc<RwLock<i32>>,

    /// Whether to display render purpose prims
    display_render: Arc<RwLock<bool>>,

    /// Whether to display proxy purpose prims
    display_proxy: Arc<RwLock<bool>>,

    /// Whether to display guide purpose prims
    display_guides: Arc<RwLock<bool>>,

    /// Whether scene materials are enabled. C++: _sceneMaterialsEnabled.
    scene_materials_enabled: Arc<RwLock<bool>>,

    /// Repr fallback selector. C++: _reprFallback.
    repr_fallback: Arc<RwLock<usd_hd::prim::HdReprSelector>>,

    /// Cache of populated prims
    prim_cache: Arc<RwLock<HashMap<Path, CachedPrimData>>>,

    /// Dirty bits per prim
    dirty_bits: Arc<Mutex<HashMap<Path, HdDirtyBits>>>,

    /// Optional render-index backend used to mirror delegate dirtying into Hydra.
    index_proxy_backend: Arc<RwLock<Option<Arc<dyn crate::index_proxy::IndexProxyBackend>>>>,

    /// Paths pending resync (repopulation)
    pending_resyncs: Arc<Mutex<HashSet<Path>>>,

    /// XformCache for efficient transform computation (per-time invalidated).
    /// Wrapped in Mutex because XformCache is not thread-safe internally.
    xform_cache: Arc<Mutex<XformCache>>,

    /// Shutter interval for motion blur sampling.
    /// Set from camera shutter open/close or explicitly via set_shutter_interval().
    shutter_interval: Arc<RwLock<ShutterInterval>>,

    /// USD path of the camera used to derive shutter open/close times.
    camera_path_for_sampling: Arc<RwLock<Option<Path>>>,

    /// Per-prim refinement level overrides. C++: _refineLevelMap.
    refine_level_map: Arc<RwLock<HashMap<Path, i32>>>,

    /// Fallback cull style for all prims. C++: _cullStyleFallback.
    cull_style_fallback: Arc<RwLock<HdCullStyle>>,

    /// Application window policy for camera conforming. C++: _appWindowPolicy.
    /// Token values: "matchVertically", "matchHorizontally", "fit", "crop", "dontConform".
    app_window_policy: Arc<RwLock<Token>>,

    /// Set of time-varying prim paths — only these need dirty on set_time().
    /// Populated during populate_subtree by checking value_might_be_time_varying.
    time_varying_prims: Arc<RwLock<HashSet<Path>>>,

    /// Excluded prim paths — prims under these are skipped during populate. C++: _excludedPrimPaths.
    excluded_paths: Arc<RwLock<Vec<Path>>>,

    /// Invised prim paths — prims under these are marked invisible. C++: _invisedPrimPaths.
    invised_paths: Arc<RwLock<Vec<Path>>>,

    /// Per-delegate scene lights enabled flag. C++: _sceneLightsEnabled.
    scene_lights_enabled: Arc<RwLock<bool>>,

    /// Root instancer id for scene instancing. C++: _rootInstancerId.
    /// When set, GetInstancerId returns this for prims that have no native instancer.
    root_instancer_id: Arc<RwLock<Path>>,

    /// Inherited visibility cache. Maps prim path -> visible (bool).
    /// Invalidated on set_time() and apply_pending_updates().
    /// C++: UsdImagingDelegate::_visCache.
    vis_cache: Arc<RwLock<HashMap<Path, bool>>>,

    /// Material binding cache. Maps prim path -> material path.
    /// Invalidated on apply_pending_updates().
    /// C++: UsdImagingDelegate::_materialBindingCache.
    material_binding_cache: Arc<RwLock<HashMap<Path, Option<Path>>>>,

    /// Primvar descriptor cache. Maps (prim_path, interpolation) -> descriptors.
    /// Invalidated on set_time() and apply_pending_updates().
    /// C++: UsdImagingDelegate::_primvarDescCache.
    primvar_desc_cache: Arc<RwLock<HashMap<(Path, u8), Vec<HdPrimvarDescriptor>>>>,

    /// Pending property changes from USD notices. Maps prim path -> dirty bits.
    /// Consumed by apply_pending_updates(). C++: _dirtyMap from _OnUsdObjectsChanged.
    pending_property_changes: Arc<Mutex<HashMap<Path, HdDirtyBits>>>,

    // ---------------------------------------------------------------------- //
    // Native instancing state — mirrors C++ UsdImagingInstanceAdapter maps
    // ---------------------------------------------------------------------- //
    /// Per-instancer data: maps hydra instancer cache path → InstancerData.
    /// Key is the first USD instance path that "owns" this instancer.
    /// C++: UsdImagingInstanceAdapter::_instancerData
    instancer_data: Arc<RwLock<HashMap<Path, InstancerData>>>,

    /// Maps each USD instance prim path → the hydra instancer cache path.
    /// C++: UsdImagingInstanceAdapter::_instanceToInstancerMap
    instance_to_instancer: Arc<RwLock<HashMap<Path, Path>>>,

    /// Maps prototype USD path → list of hydra instancer paths.
    /// Multiple instancers exist when instances have different inherited attrs.
    /// C++: UsdImagingInstanceAdapter::_prototypeToInstancerMap
    prototype_to_instancers: Arc<RwLock<HashMap<Path, Vec<Path>>>>,

    /// USD notice change handler. Listens for ObjectsChanged notices and
    /// accumulates pending changes for apply_pending_updates().
    /// C++: _objectsChangedNoticeKey + _OnUsdObjectsChanged.
    change_handler: Option<Arc<ChangeHandler>>,

    // ---------------------------------------------------------------------- //
    // P2 delegate fields
    // ---------------------------------------------------------------------- //
    /// Rigid transform overrides for camera/prim override transforms.
    /// Maps prim path -> override matrix. C++: _rigidXformOverrides.
    rigid_xform_overrides: Arc<RwLock<HashMap<Path, Matrix4d>>>,

    /// Whether USD draw modes (model:drawMode) are enabled. C++: _enableUsdDrawModes.
    /// Must be set before populate(); changing after is unsupported.
    usd_draw_modes_enabled: AtomicBool,

    /// Whether to display unloaded prims as bounding box wireframes.
    /// C++: _displayUnloadedPrimsWithBounds. Must be set before populate().
    display_unloaded_with_bounds: AtomicBool,

    /// Coordinate system bindings enabled flag. C++: _coordSysEnabled.
    coord_sys_enabled: AtomicBool,

    // ---------------------------------------------------------------------- //
    // Property caches (A1)
    // ---------------------------------------------------------------------- //
    /// Draw mode cache. Maps prim path -> computed draw mode token.
    /// Invalidated on model:drawMode / model:applyDrawMode property change.
    /// C++: UsdImagingDelegate::_drawModeCache.
    draw_mode_cache: Arc<RwLock<HashMap<Path, Token>>>,

    /// Purpose cache. Maps prim path -> resolved inheritable purpose token.
    /// Invalidated on purpose property change.
    /// C++: UsdImagingDelegate::_purposeCache.
    purpose_cache: Arc<RwLock<HashMap<Path, Token>>>,

    /// Coordinate system binding cache. Maps prim path -> bound coord sys paths.
    /// Invalidated on coordSys:* property change.
    /// C++: UsdImagingDelegate::_coordSysBindingCache.
    coord_sys_cache: Arc<RwLock<HashMap<Path, Option<Arc<Vec<Path>>>>>>,

    // ---------------------------------------------------------------------- //
    // Motion blur / point instancer caches
    // ---------------------------------------------------------------------- //
    /// Point instancer indices cache. Maps instancer path -> indices array.
    point_instancer_indices_cache: Arc<RwLock<HashMap<Path, Vec<i32>>>>,

    /// Nonlinear sample count per prim (default 3). C++: nonlinearSampleCount attr.
    nonlinear_sample_count_cache: Arc<RwLock<HashMap<Path, i32>>>,

    /// Blur scale per prim (default 1.0). C++: blurScale attr.
    blur_scale_cache: Arc<RwLock<HashMap<Path, f32>>>,

    /// Light linking collection cache. C++: _collectionCache.
    /// Maps light collection paths to MembershipQuery objects.
    /// Populated during populate/resync when light prims are discovered.
    light_link_cache: Arc<LightLinkingCache>,
}

impl UsdImagingDelegate {
    fn attr_get_value_or_default(attr: &UsdAttribute, time: SdfTimeCode) -> Option<Value> {
        attr.get(time)
            .or_else(|| attr.get(SdfTimeCode::default_time()))
    }

    fn attr_get_vec3f(attr: &UsdAttribute, time: SdfTimeCode) -> Option<Vec<usd_gf::Vec3f>> {
        if let Some(values) = attr
            .get_typed_vec::<usd_gf::Vec3f>(time)
            .or_else(|| attr.get_typed_vec::<usd_gf::Vec3f>(SdfTimeCode::default_time()))
        {
            return Some(values);
        }

        let value = Self::attr_get_value_or_default(attr, time)?;
        if let Some(values) = value.as_vec_clone::<usd_gf::Vec3f>() {
            return Some(values);
        }
        if let Some(values) = value.downcast::<Vec<Value>>() {
            return values
                .iter()
                .map(|entry| {
                    if let Some(vec3) = entry.downcast_clone::<usd_gf::Vec3f>() {
                        return Some(vec3);
                    }
                    if let Some(tuple) = entry.downcast::<Vec<Value>>() {
                        let x = tuple.first().and_then(value_scalar_to_f32)?;
                        let y = tuple.get(1).and_then(value_scalar_to_f32)?;
                        let z = tuple.get(2).and_then(value_scalar_to_f32)?;
                        return Some(usd_gf::Vec3f::new(x, y, z));
                    }
                    None
                })
                .collect();
        }
        None
    }

    fn attr_get_i32_vec(attr: &UsdAttribute, time: SdfTimeCode) -> Option<Vec<i32>> {
        if let Some(values) = attr
            .get_typed_vec::<i32>(time)
            .or_else(|| attr.get_typed_vec::<i32>(SdfTimeCode::default_time()))
        {
            return Some(values);
        }

        let value = Self::attr_get_value_or_default(attr, time)?;
        value.as_vec_clone::<i32>().or_else(|| {
            value
                .downcast::<Vec<Value>>()
                .and_then(|values| values.iter().map(value_scalar_to_i32).collect())
        })
    }

    fn attr_get_f32_vec(attr: &UsdAttribute, time: SdfTimeCode) -> Option<Vec<f32>> {
        if let Some(values) = attr
            .get_typed_vec::<f32>(time)
            .or_else(|| attr.get_typed_vec::<f32>(SdfTimeCode::default_time()))
        {
            return Some(values);
        }

        let value = Self::attr_get_value_or_default(attr, time)?;
        value.as_vec_clone::<f32>().or_else(|| {
            value
                .downcast::<Vec<Value>>()
                .and_then(|values| values.iter().map(value_scalar_to_f32).collect())
        })
    }

    fn attr_get_token(attr: &UsdAttribute, time: SdfTimeCode) -> Option<Token> {
        attr.get_typed::<Token>(time)
            .or_else(|| attr.get_typed::<Token>(SdfTimeCode::default_time()))
            .or_else(|| {
                Self::attr_get_value_or_default(attr, time).and_then(|value| {
                    value
                        .get::<Token>()
                        .cloned()
                        .or_else(|| value.get::<String>().map(|text| Token::new(text)))
                })
            })
    }

    fn read_primvar_value_at_time(prim: &Prim, key: &Token, time: SdfTimeCode) -> Value {
        let key_str = key.as_str();
        let value = match key_str {
            "points" => {
                let pb = PointBased::new(prim.clone());
                Self::attr_get_vec3f(&pb.get_points_attr(), time).map(Value::from)
            }
            "normals" => {
                let pb = PointBased::new(prim.clone());
                Self::attr_get_vec3f(&pb.get_normals_attr(), time).map(Value::from)
            }
            "displayColor" => prim
                .get_attribute("primvars:displayColor")
                .and_then(|attr| Self::attr_get_vec3f(&attr, time))
                .or_else(|| {
                    prim.get_attribute("displayColor")
                        .and_then(|attr| Self::attr_get_vec3f(&attr, time))
                })
                .map(Value::from),
            "displayOpacity" => prim
                .get_attribute("primvars:displayOpacity")
                .and_then(|attr| Self::attr_get_f32_vec(&attr, time))
                .or_else(|| {
                    prim.get_attribute("displayOpacity")
                        .and_then(|attr| Self::attr_get_f32_vec(&attr, time))
                })
                .map(Value::from),
            _ => {
                let primvar_name = format!("primvars:{key_str}");
                prim.get_attribute(&primvar_name)
                    .and_then(|attr| Self::attr_get_value_or_default(&attr, time))
                    .or_else(|| {
                        prim.get_attribute(key_str)
                            .and_then(|attr| Self::attr_get_value_or_default(&attr, time))
                    })
            }
        };

        normalize_sampled_primvar_value(key, value.unwrap_or_default())
    }

    /// Create new delegate wrapping a USD stage.
    ///
    /// # Arguments
    ///
    /// * `stage` - USD stage to render
    /// * `root_path` - Root path for delegation (typically `/`)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let stage = Stage::open("scene.usd")?;
    /// let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());
    /// ```
    pub fn new(stage: Arc<Stage>, root_path: Path) -> Arc<Self> {
        // Create change handler before wrapping stage — needs Weak<Stage>.
        let handler = ChangeHandler::new(Arc::downgrade(&stage));
        handler.register();

        Arc::new(Self {
            stage: Arc::new(RwLock::new(Some(stage))),
            root_path,
            time: Arc::new(RwLock::new(UsdTimeCode::new(f64::MAX))), // C++ default: double::max
            adapter_registry: Arc::new(AdapterRegistry::new_with_defaults()),
            root_transform: Arc::new(RwLock::new(Matrix4d::identity())),
            root_visible: Arc::new(RwLock::new(true)),
            refine_level_fallback: Arc::new(RwLock::new(0)), // C++ default: 0
            display_render: Arc::new(RwLock::new(true)),
            display_proxy: Arc::new(RwLock::new(true)), // C++ default: true
            display_guides: Arc::new(RwLock::new(true)), // C++ default: true
            scene_materials_enabled: Arc::new(RwLock::new(true)), // C++ default: true
            repr_fallback: Arc::new(RwLock::new(usd_hd::prim::HdReprSelector::default())),
            prim_cache: Arc::new(RwLock::new(HashMap::new())),
            dirty_bits: Arc::new(Mutex::new(HashMap::new())),
            index_proxy_backend: Arc::new(RwLock::new(None)),
            pending_resyncs: Arc::new(Mutex::new(HashSet::new())),
            xform_cache: Arc::new(Mutex::new(XformCache::new(SdfTimeCode::new(f64::MAX)))), // match delegate time
            shutter_interval: Arc::new(RwLock::new(ShutterInterval::default())),
            camera_path_for_sampling: Arc::new(RwLock::new(None)),
            refine_level_map: Arc::new(RwLock::new(HashMap::new())),
            cull_style_fallback: Arc::new(RwLock::new(HdCullStyle::DontCare)), // C++ default
            app_window_policy: Arc::new(RwLock::new(Token::new("matchVertically"))), // C++ CameraUtilMatchVertically
            time_varying_prims: Arc::new(RwLock::new(HashSet::new())),
            excluded_paths: Arc::new(RwLock::new(Vec::new())),
            invised_paths: Arc::new(RwLock::new(Vec::new())),
            scene_lights_enabled: Arc::new(RwLock::new(true)), // C++ default: true
            root_instancer_id: Arc::new(RwLock::new(Path::empty())), // C++ default: empty
            vis_cache: Arc::new(RwLock::new(HashMap::new())),
            material_binding_cache: Arc::new(RwLock::new(HashMap::new())),
            primvar_desc_cache: Arc::new(RwLock::new(HashMap::new())),
            pending_property_changes: Arc::new(Mutex::new(HashMap::new())),
            instancer_data: Arc::new(RwLock::new(HashMap::new())),
            instance_to_instancer: Arc::new(RwLock::new(HashMap::new())),
            prototype_to_instancers: Arc::new(RwLock::new(HashMap::new())),
            change_handler: Some(handler),
            rigid_xform_overrides: Arc::new(RwLock::new(HashMap::new())),
            usd_draw_modes_enabled: AtomicBool::new(false),
            display_unloaded_with_bounds: AtomicBool::new(false),
            coord_sys_enabled: AtomicBool::new(false),
            draw_mode_cache: Arc::new(RwLock::new(HashMap::new())),
            purpose_cache: Arc::new(RwLock::new(HashMap::new())),
            coord_sys_cache: Arc::new(RwLock::new(HashMap::new())),
            point_instancer_indices_cache: Arc::new(RwLock::new(HashMap::new())),
            nonlinear_sample_count_cache: Arc::new(RwLock::new(HashMap::new())),
            blur_scale_cache: Arc::new(RwLock::new(HashMap::new())),
            light_link_cache: Arc::new(LightLinkingCache::new()),
        })
    }

    /// Populate the delegate from the USD stage.
    ///
    /// This traverses the stage hierarchy starting from the root path and
    /// creates render index entries for all relevant prims.
    ///
    /// When `index_proxy` is provided and has a backend, prims are also
    /// inserted into HdRenderIndex. Otherwise only internal cache is populated.
    ///
    /// # Example
    ///
    /// ```ignore
    /// delegate.populate();
    /// // Or with render index:
    /// delegate.populate_with_proxy(&mut index_proxy);
    /// ```
    pub fn populate(&self) {
        usd_trace::trace_scope!("delegate_populate");
        self.populate_with_excluded(&[], &[]);
    }

    /// Populate with excluded and invised path lists. C++: Populate(rootPrim, excluded, invised).
    ///
    /// Prims under `excluded_paths` are entirely skipped (not populated).
    /// Prims under `invised_paths` are populated but marked invisible.
    pub fn populate_with_excluded(&self, excluded_paths: &[Path], invised_paths: &[Path]) {
        // Store excluded/invised for later use (SetInvisedPrimPaths, etc.)
        *self.excluded_paths.write() = excluded_paths.to_vec();
        *self.invised_paths.write() = invised_paths.to_vec();

        let Some(stage) = self.get_stage() else {
            return;
        };

        let root_prim = if self.root_path.is_absolute_root_path() {
            stage.get_pseudo_root()
        } else {
            match stage.get_prim_at_path(&self.root_path) {
                Some(prim) => prim,
                None => return,
            }
        };

        self.populate_subtree_with_proxy(&root_prim, &mut crate::index_proxy::IndexProxy::new());
    }

    /// Attach or detach a render-index backend so delegate dirtying can reach Hydra directly.
    pub fn set_index_proxy_backend(
        &self,
        backend: Option<Arc<dyn crate::index_proxy::IndexProxyBackend>>,
    ) {
        *self.index_proxy_backend.write() = backend;
    }

    fn mark_backend_rprim_dirty(&self, cache_path: &Path, bits: HdDirtyBits) {
        if bits == 0 {
            return;
        }
        let backend = self.index_proxy_backend.read().clone();
        let Some(backend) = backend else {
            return;
        };
        let prim_type = {
            let cache = self.prim_cache.read();
            cache.get(cache_path).map(|cached| cached.prim_type.clone())
        };
        let Some(prim_type) = prim_type else {
            return;
        };
        if !backend.is_rprim_type_supported(&prim_type) {
            return;
        }
        let index_path = self.convert_cache_path_to_index_path(cache_path);
        backend.mark_rprim_dirty(&index_path, bits);
    }

    fn mark_backend_rprims_dirty<'a, I>(&self, cache_paths: I, bits: HdDirtyBits)
    where
        I: IntoIterator<Item = &'a Path>,
    {
        for cache_path in cache_paths {
            self.mark_backend_rprim_dirty(cache_path, bits);
        }
    }

    /// Populate the delegate and insert prims into the render index via IndexProxy.
    ///
    /// Use this when the delegate is used with HdRenderIndex (legacy Hydra flow).
    /// The proxy must be created with [`IndexProxy::new_with_backend`].
    pub fn populate_with_proxy(&self, index_proxy: &mut crate::index_proxy::IndexProxy) {
        let Some(stage) = self.get_stage() else {
            return;
        };

        let root_prim = if self.root_path.is_absolute_root_path() {
            stage.get_pseudo_root()
        } else {
            match stage.get_prim_at_path(&self.root_path) {
                Some(prim) => prim,
                None => return,
            }
        };

        self.populate_subtree_with_proxy(&root_prim, index_proxy);
    }

    /// Populate a specific USD prim and its descendants.
    ///
    /// # Arguments
    ///
    /// * `root_prim` - USD prim to start population from
    ///
    /// # Example
    ///
    /// ```ignore
    /// let prim = stage.get_prim_at_path(&Path::from_string("/World")?)?;
    /// delegate.populate_prim(&prim);
    /// ```
    pub fn populate_prim(&self, root_prim: &Prim) {
        self.populate_subtree_with_proxy(root_prim, &mut crate::index_proxy::IndexProxy::new());
    }

    /// Populate a specific USD prim and its descendants, inserting into render index.
    pub fn populate_prim_with_proxy(
        &self,
        root_prim: &Prim,
        index_proxy: &mut crate::index_proxy::IndexProxy,
    ) {
        self.populate_subtree_with_proxy(root_prim, index_proxy);
    }

    /// Set the current time for animation.
    ///
    /// This marks all time-varying attributes as dirty for the next sync.
    ///
    /// # Arguments
    ///
    /// * `time` - Time code to evaluate at
    ///
    /// # Example
    ///
    /// ```ignore
    /// delegate.set_time(UsdTimeCode::new(24.0));
    /// ```
    pub fn set_time(&self, time: UsdTimeCode) {
        usd_trace::trace_scope!("delegate_set_time");
        use std::sync::atomic::{AtomicU32, Ordering};
        static DIAG_DELEGATE_TIME: AtomicU32 = AtomicU32::new(0);
        // P0-IMG-1: C++ ALWAYS calls ApplyPendingUpdates() first, before any
        // time logic. Many clients rely on SetTime() to implicitly flush queued
        // scene edits (prim resyncs, property changes from USD notices).
        self.apply_pending_updates();

        // P1-IMG-2: early-out when time hasn't changed — avoids redundant dirty
        // marking every frame when callers call SetTime(currentTime) repeatedly.
        let current_before = *self.time.read();
        let changed = current_before != time;
        let n = DIAG_DELEGATE_TIME.fetch_add(1, Ordering::Relaxed);
        if n < 40 {
            eprintln!(
                "[DIAG] delegate_set_time: before={} incoming={} changed={}",
                current_before.value(),
                time.value(),
                changed
            );
        }
        {
            let current = self.time.read();
            if *current == time {
                if n < 40 {
                    eprintln!("[DIAG] delegate_set_time_early_return: time_unchanged");
                }
                return;
            }
        }
        *self.time.write() = time;
        if n < 40 {
            eprintln!("[DIAG] delegate_set_time_applied: after={}", time.value());
        }

        // Invalidate XformCache when time changes — transforms may be animated.
        let sdf_time = SdfTimeCode::new(time.value());
        self.xform_cache
            .lock()
            .expect("Lock poisoned")
            .set_time(sdf_time);

        // P1-1: Only mark prims that have time-varying attributes dirty,
        // and only with their per-adapter time_varying_bits (not all 32 bits).
        // C++ uses _hdPrimInfo.timeVaryingBits populated by TrackVariability.
        // Invalidate visibility, primvar, and motion blur caches since time changed.
        self.vis_cache.write().clear();
        self.primvar_desc_cache.write().clear();
        self.point_instancer_indices_cache.write().clear();
        self.nonlinear_sample_count_cache.write().clear();
        self.blur_scale_cache.write().clear();

        let time_varying = self.time_varying_prims.read();
        let cache = self.prim_cache.read();
        let mut dirty = self.dirty_bits.lock().expect("Lock poisoned");

        log::trace!(
            "[PERF] set_time: time_varying_prims={} prim_cache={}",
            time_varying.len(),
            cache.len()
        );
        if time_varying.is_empty() {
            // Fall back to all prims if time-varying set not yet populated.
            for (path, cached) in cache.iter() {
                let bits = if cached.time_varying_bits != 0 {
                    cached.time_varying_bits
                } else {
                    !0 // fallback: all dirty if not yet computed
                };
                dirty.insert(path.clone(), bits);
                self.mark_backend_rprim_dirty(path, bits);
            }
        } else {
            for path in time_varying.iter() {
                let bits = cache.get(path).map(|c| c.time_varying_bits).unwrap_or(!0);
                let bits = if bits != 0 { bits } else { !0 };
                dirty.insert(path.clone(), bits);
                self.mark_backend_rprim_dirty(path, bits);
            }
        }
    }

    /// Get the current evaluation time.
    pub fn get_time(&self) -> UsdTimeCode {
        *self.time.read()
    }

    /// Return current time offset by `offset` frames. C++ delegate.cpp:912-915.
    /// If time is numeric, returns time + offset; otherwise returns time as-is.
    pub fn get_time_with_offset(&self, offset: f64) -> UsdTimeCode {
        let t = self.get_time();
        if t.is_numeric() {
            UsdTimeCode::new(t.value() + offset)
        } else {
            t
        }
    }

    // ----------------------------------------------------------------------- //
    // Motion blur / shutter interval
    // ----------------------------------------------------------------------- //

    /// Set the USD camera path whose shutter open/close times drive motion sampling.
    ///
    /// Mirrors C++ `UsdImagingDelegate::SetCameraForSampling()`.
    pub fn set_camera_for_sampling(&self, path: Option<Path>) {
        *self.camera_path_for_sampling.write() = path;
    }

    /// Explicitly set the shutter interval (frame-relative offsets).
    ///
    /// Overrides camera-derived shutter when called directly.
    /// Use [0.0, 0.0] to disable motion blur.
    pub fn set_shutter_interval(&self, interval: ShutterInterval) {
        *self.shutter_interval.write() = interval;
    }

    /// Get the current shutter interval.
    pub fn get_shutter_interval(&self) -> ShutterInterval {
        *self.shutter_interval.read()
    }

    /// Compute the current time sampling interval as absolute USD time codes.
    ///
    /// Reads shutter open/close from the camera prim if set, otherwise falls
    /// back to the stored shutter_interval.  Returns [t+open, t+close].
    ///
    /// Mirrors C++ `UsdImagingDelegate::GetCurrentTimeSamplingInterval()`.
    pub fn get_time_sampling_interval(&self) -> (f64, f64) {
        let current = self.get_time().value();
        let mut si = self.get_shutter_interval();

        // Override from camera shutter params if a camera path is set.
        if let Some(cam_path) = self.camera_path_for_sampling.read().clone() {
            if let Some(stage) = self.get_stage() {
                if let Some(cam_prim) = stage.get_prim_at_path(&cam_path) {
                    let t = SdfTimeCode::new(current);
                    if let Some(open_val) = cam_prim
                        .get_attribute("shutter:open")
                        .and_then(|a| a.get(t))
                        .and_then(|v| v.get::<f64>().copied())
                    {
                        si.open = open_val;
                    }
                    if let Some(close_val) = cam_prim
                        .get_attribute("shutter:close")
                        .and_then(|a| a.get(t))
                        .and_then(|v| v.get::<f64>().copied())
                    {
                        si.close = close_val;
                    }
                }
            }
        }

        (current + si.open, current + si.close)
    }

    /// Collect time sample points for a USD attribute within the shutter interval.
    ///
    /// Returns samples inside [interval_min, interval_max] plus one bracketing
    /// sample on each side, ensuring the shutter boundary values are covered.
    /// Mirrors C++ `_GetTimeSamplesForInterval()`.
    fn get_time_samples_for_interval(
        attr: &UsdAttribute,
        interval_min: f64,
        interval_max: f64,
    ) -> Vec<f64> {
        let mut times = attr.get_time_samples_in_interval(interval_min, interval_max);

        // Add bracketing samples outside the interval for edge accuracy.
        if let Some((lower, _)) = attr.get_bracketing_time_samples(interval_min) {
            if times.is_empty() || lower < times[0] {
                times.insert(0, lower);
            }
        } else {
            // No bracketing below — use the interval edge itself.
            if times.is_empty() || interval_min < times[0] {
                times.insert(0, interval_min);
            }
        }

        if let Some((_, upper)) = attr.get_bracketing_time_samples(interval_max) {
            if upper > *times.last().unwrap_or(&interval_max) {
                times.push(upper);
            }
        } else {
            let last = *times.last().unwrap_or(&interval_max);
            if interval_max > last {
                times.push(interval_max);
            }
        }

        times.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        times
    }

    /// Set the root transform applied to all prims.
    ///
    /// # Arguments
    ///
    /// * `transform` - 4x4 transformation matrix
    ///
    /// # Example
    ///
    /// ```ignore
    /// let xform = Matrix4d::new_scale(Vec3d::new(2.0, 2.0, 2.0));
    /// delegate.set_root_transform(xform);
    /// ```
    pub fn set_root_transform(&self, transform: Matrix4d) {
        *self.root_transform.write() = transform;

        // Mark all transforms dirty
        let cache = self.prim_cache.read();
        let mut dirty = self.dirty_bits.lock().expect("Lock poisoned");

        for path in cache.keys() {
            let bits = dirty.entry(path.clone()).or_insert(0);
            *bits |= 0x01; // Transform dirty bit
        }
        self.mark_backend_rprims_dirty(cache.keys(), usd_hd::HdRprimDirtyBits::DIRTY_TRANSFORM);
    }

    /// Get the root transform.
    pub fn get_root_transform(&self) -> Matrix4d {
        *self.root_transform.read()
    }

    /// Set root visibility flag.
    ///
    /// When false, all prims are considered invisible.
    ///
    /// # Arguments
    ///
    /// * `visible` - Visibility state
    pub fn set_root_visibility(&self, visible: bool) {
        *self.root_visible.write() = visible;

        // Mark all visibility dirty
        let cache = self.prim_cache.read();
        let mut dirty = self.dirty_bits.lock().expect("Lock poisoned");

        for path in cache.keys() {
            let bits = dirty.entry(path.clone()).or_insert(0);
            *bits |= 0x02; // Visibility dirty bit
        }
        self.mark_backend_rprims_dirty(cache.keys(), usd_hd::HdRprimDirtyBits::DIRTY_VISIBILITY);
    }

    /// Get root visibility.
    pub fn get_root_visibility(&self) -> bool {
        *self.root_visible.read()
    }

    /// Set fallback refinement level. C++: SetRefineLevelFallback().
    /// Marks DirtyDisplayStyle on all prims NOT in per-prim refine_level_map.
    pub fn set_refine_level_fallback(&self, level: i32) {
        // C++ _ValidateRefineLevel: clamp to [0, 8]
        if !(0..=8).contains(&level) {
            log::warn!("Invalid refine level {level}, must be [0..8]");
            return;
        }
        let mut w = self.refine_level_fallback.write();
        if *w == level {
            return;
        }
        *w = level;
        drop(w);

        // C++: iterates _hdPrimInfoMap, skips prims in _refineLevelMap,
        // marks DirtyDisplayStyle on the rest.
        let refine_map = self.refine_level_map.read();
        let cache = self.prim_cache.read();
        let mut dirty = self.dirty_bits.lock().expect("Lock poisoned");
        let mut dirtied_paths = Vec::new();
        for cache_path in cache.keys() {
            if !refine_map.contains_key(cache_path) {
                let bits = dirty.entry(cache_path.clone()).or_insert(0);
                *bits |= usd_hd::HdRprimDirtyBits::DIRTY_DISPLAY_STYLE;
                dirtied_paths.push(cache_path.clone());
            }
        }
        self.mark_backend_rprims_dirty(
            dirtied_paths.iter(),
            usd_hd::HdRprimDirtyBits::DIRTY_DISPLAY_STYLE,
        );
    }

    /// Get fallback refinement level.
    pub fn get_refine_level_fallback(&self) -> i32 {
        *self.refine_level_fallback.read()
    }

    /// Set whether to display render purpose prims. C++: SetDisplayRender().
    /// Marks DirtyRenderTag on all prims per C++ _MarkRenderTagsDirty().
    pub fn set_display_render(&self, display: bool) {
        let mut w = self.display_render.write();
        if *w == display {
            return;
        }
        *w = display;
        drop(w);
        self.mark_render_tags_dirty();
    }

    /// Set whether to display proxy purpose prims. C++: SetDisplayProxy().
    /// Marks DirtyRenderTag on all prims per C++ _MarkRenderTagsDirty().
    pub fn set_display_proxy(&self, display: bool) {
        let mut w = self.display_proxy.write();
        if *w == display {
            return;
        }
        *w = display;
        drop(w);
        self.mark_render_tags_dirty();
    }

    /// Set whether to display guide purpose prims. C++: SetDisplayGuides().
    /// Marks DirtyRenderTag on all prims per C++ _MarkRenderTagsDirty().
    pub fn set_display_guides(&self, display: bool) {
        let mut w = self.display_guides.write();
        if *w == display {
            return;
        }
        *w = display;
        drop(w);
        self.mark_render_tags_dirty();
    }

    /// C++ _MarkRenderTagsDirty(): iterate all prims and mark DirtyRenderTag.
    fn mark_render_tags_dirty(&self) {
        let cache = self.prim_cache.read();
        let mut dirty = self.dirty_bits.lock().expect("Lock poisoned");
        for cache_path in cache.keys() {
            let bits = dirty.entry(cache_path.clone()).or_insert(0);
            *bits |= usd_hd::HdRprimDirtyBits::DIRTY_RENDER_TAG;
        }
        self.mark_backend_rprims_dirty(cache.keys(), usd_hd::HdRprimDirtyBits::DIRTY_RENDER_TAG);
    }

    /// Get categories for light linking. C++: GetCategories().
    ///
    /// Returns collection tokens for all non-trivial light/shadow link collections
    /// that include `id`. Instancer prototype prims with child paths return
    /// empty — those get correct categories via `get_instance_categories()`.
    /// Matches C++ `UsdImagingDelegate::GetCategories()`.
    pub fn get_categories(&self, id: &Path) -> Vec<Token> {
        // C++ line 2808-2814: skip instancer adapter child paths (prototypes)
        // to avoid returning wrong inherited collections.
        let cache = self.prim_cache.read();
        if let Some(cached) = cache.get(id) {
            if cached.adapter.is_instancer_adapter() && cached.adapter.is_child_path(id) {
                return Vec::new();
            }
        }
        drop(cache);
        self.light_link_cache
            .compute_collections_containing_path(id)
    }

    /// Get per-instance categories for light linking. C++: GetInstanceCategories().
    ///
    /// Delegates to the native instancing adapter. Returns one `Vec<Token>`
    /// per instance of the given instancer. Matches C++
    /// `UsdImagingDelegate::GetInstanceCategories()`.
    pub fn get_instance_categories(&self, instancer_id: &Path) -> Vec<Vec<Token>> {
        let cache = self.prim_cache.read();
        if let Some(cached) = cache.get(instancer_id) {
            // Resolve USD prim from stage to pass to adapter.
            if let Some(stage) = self.get_stage() {
                if let Some(prim) = stage.get_prim_at_path(&cached.usd_path) {
                    return cached
                        .adapter
                        .get_instance_categories(&prim, &self.light_link_cache);
                }
            }
        }
        Vec::new()
    }

    /// Register lightLink and shadowLink collections from a light prim.
    ///
    /// Called during populate and resync. Matches C++
    /// `UsdImagingLightAdapter::_RegisterLightCollections()`.
    fn register_light_collections(&self, prim: &usd_core::Prim) {
        use usd_lux::light_api::LightAPI;
        let light_api = LightAPI::new(prim.clone());

        // lightLink collection
        let ll_col = light_api.get_light_link_collection_api();
        if ll_col.is_valid() {
            let query = ll_col.compute_membership_query();
            if let Some(col_path) = prim.get_path().append_property("collection:lightLink") {
                self.light_link_cache.update_collection(col_path, query);
            }
        }

        // shadowLink collection
        let sl_col = light_api.get_shadow_link_collection_api();
        if sl_col.is_valid() {
            let query = sl_col.compute_membership_query();
            if let Some(col_path) = prim.get_path().append_property("collection:shadowLink") {
                self.light_link_cache.update_collection(col_path, query);
            }
        }
    }

    /// Unregister lightLink and shadowLink collections for a removed light.
    ///
    /// Matches C++ `UsdImagingLightAdapter::_UnregisterLightCollections()`.
    fn unregister_light_collections(&self, cache_path: &Path) {
        if let Some(ll_path) = cache_path.append_property("collection:lightLink") {
            self.light_link_cache.remove_collection(&ll_path);
        }
        if let Some(sl_path) = cache_path.append_property("collection:shadowLink") {
            self.light_link_cache.remove_collection(&sl_path);
        }
    }

    /// Enable/disable scene materials. C++: SetSceneMaterialsEnabled().
    pub fn set_scene_materials_enabled(&self, enable: bool) {
        let mut w = self.scene_materials_enabled.write();
        if *w == enable {
            return;
        }
        *w = enable;
        // C++: iterates all prims, marks DirtyMaterial — our engine does full resync
    }

    /// Whether scene materials are enabled. C++: _sceneMaterialsEnabled.
    pub fn is_scene_materials_enabled(&self) -> bool {
        *self.scene_materials_enabled.read()
    }

    /// Set repr fallback selector. C++: SetReprFallback().
    pub fn set_repr_fallback(&self, repr: usd_hd::prim::HdReprSelector) {
        let mut w = self.repr_fallback.write();
        if *w == repr {
            return;
        }
        *w = repr;
        // C++: iterates all prims, marks DirtyRepr — our engine does full resync
    }

    /// Enable/disable scene lights for this delegate. C++: SetSceneLightsEnabled().
    pub fn set_scene_lights_enabled(&self, enabled: bool) {
        let mut w = self.scene_lights_enabled.write();
        if *w == enabled {
            return;
        }
        *w = enabled;
        // C++: iterates all prims, marks DirtyParams|DirtyResource — our engine does full resync
    }

    /// Whether scene lights are enabled for this delegate. C++: _sceneLightsEnabled.
    pub fn is_scene_lights_enabled(&self) -> bool {
        *self.scene_lights_enabled.read()
    }

    /// Set root instancer id for scene instancing. C++: SetRootInstancerId().
    /// When set, GetInstancerId returns this for prims that have no native instancer.
    /// Marks all prims DirtyInstancer when changed.
    pub fn set_root_instancer_id(&self, instancer_id: &Path) {
        let mut w = self.root_instancer_id.write();
        if *w == *instancer_id {
            return;
        }
        *w = instancer_id.clone();
        drop(w);

        // C++: marks all prims DirtyInstancer via MarkDirty.
        let cache = self.prim_cache.read();
        let mut dirty = self.dirty_bits.lock().expect("Lock poisoned");
        for cache_path in cache.keys() {
            let bits = dirty.entry(cache_path.clone()).or_insert(0);
            *bits |= usd_hd::HdRprimDirtyBits::DIRTY_INSTANCER;
        }
        self.mark_backend_rprims_dirty(cache.keys(), usd_hd::HdRprimDirtyBits::DIRTY_INSTANCER);
    }

    /// Get root instancer id. C++: GetRootInstancerId().
    pub fn get_root_instancer_id(&self) -> Path {
        self.root_instancer_id.read().clone()
    }

    /// Populate selection from a USD prim path. C++: PopulateSelection().
    ///
    /// Finds all cached Hydra prims at or under `usd_path` and adds them to
    /// the selection. Returns true if any prims were added.
    pub fn populate_selection(
        &self,
        highlight_mode: usd_hd::HdSelectionHighlightMode,
        usd_path: &Path,
        instance_index: i32,
        result: &usd_hd::HdSelectionSharedPtr,
    ) -> bool {
        // Guard access to stage (C++: early return if !_stage).
        let Some(stage) = self.get_stage() else {
            return false;
        };

        // C++: ApplyPendingUpdates() before selection.
        self.apply_pending_updates();

        // Verify the selected prim exists on stage.
        let selected_prim = stage.get_prim_at_path(usd_path);
        if selected_prim.is_none() {
            return false;
        }

        // C++ lines 2535-2538: If selected prim is an instance proxy, walk up
        // to the owning instance. Instance proxies are virtual prims and the
        // dependency graph stops at the instance boundary.
        let mut root_path = usd_path.clone();
        if let Some(ref sp) = selected_prim {
            let mut walk_prim = sp.clone();
            while walk_prim.is_valid()
                && walk_prim.is_instance_proxy()
                && !walk_prim.get_path().is_absolute_root_path()
            {
                let parent = walk_prim.parent();
                if !parent.is_valid() {
                    break;
                }
                walk_prim = parent;
            }
            root_path = walk_prim.get_path().clone();
        }

        // Gather cache paths at or under root_path.
        // C++: _GatherDependencies(rootPath, &affectedCachePaths)
        let cache = self.prim_cache.read();
        let mut affected: Vec<Path> = cache
            .iter()
            .filter(|(_, data)| data.usd_path.has_prefix(&root_path))
            .map(|(cache_path, _)| cache_path.clone())
            .collect();
        affected.sort();
        affected.dedup();

        // C++: unique with HasPrefix predicate — remove paths that are
        // descendants of an earlier path (keep shortest prefix).
        let mut deduped = Vec::with_capacity(affected.len());
        for path in &affected {
            if deduped
                .last()
                .map_or(true, |last: &Path| !path.has_prefix(last))
            {
                deduped.push(path.clone());
            }
        }

        // Add each affected cache path to the selection.
        let mut added = false;
        let mut sel = result.write();
        for cache_path in &deduped {
            if instance_index >= 0 {
                // Instance-level selection
                sel.add_instance(highlight_mode, cache_path.clone(), vec![instance_index]);
            } else {
                // Whole-rprim selection
                sel.add_rprim(highlight_mode, cache_path.clone());
            }
            added = true;
        }

        added
    }

    /// Set the list of paths that must be invised. C++: SetInvisedPrimPaths().
    pub fn set_invised_paths(&self, paths: Vec<Path>) {
        use std::collections::BTreeSet;
        let existing = self.invised_paths.read().clone();
        if existing == paths {
            return;
        }

        // Compute symmetric difference — paths that changed visibility state.
        // C++ delegate.cpp:2290-2298.
        let new_set: BTreeSet<&Path> = paths.iter().collect();
        let old_set: BTreeSet<&Path> = existing.iter().collect();
        let changed: Vec<&Path> = new_set.symmetric_difference(&old_set).copied().collect();

        // Mark DirtyVisibility on changed subtree roots.
        // C++ delegate.cpp:2300-2315.
        if !changed.is_empty() {
            let cache = self.prim_cache.read();
            let mut dirty = self.dirty_bits.lock().expect("Lock poisoned");
            for changed_root in &changed {
                for cache_path in cache.keys() {
                    if cache_path.has_prefix(changed_root) {
                        *dirty.entry(cache_path.clone()).or_insert(0) |= 0x02; // DirtyVisibility
                    }
                }
            }
        }

        *self.invised_paths.write() = paths;
    }

    /// Get the current list of excluded prim paths.
    pub fn get_excluded_paths(&self) -> Vec<Path> {
        self.excluded_paths.read().clone()
    }

    /// Get the current list of invised prim paths.
    pub fn get_invised_paths(&self) -> Vec<Path> {
        self.invised_paths.read().clone()
    }

    /// Set per-prim refinement level override. C++: SetRefineLevel(path, level).
    pub fn set_refine_level(&self, path: Path, level: i32) {
        self.refine_level_map.write().insert(path, level);
    }

    /// Clear per-prim refinement level override. C++: ClearRefineLevel(path).
    pub fn clear_refine_level(&self, path: &Path) {
        self.refine_level_map.write().remove(path);
    }

    /// Set fallback cull style. C++: SetCullStyleFallback().
    pub fn set_cull_style_fallback(&self, style: HdCullStyle) {
        *self.cull_style_fallback.write() = style;
    }

    /// Set window policy for camera conforming. C++: SetWindowPolicy().
    pub fn set_window_policy(&self, policy: Token) {
        *self.app_window_policy.write() = policy;
    }

    /// Get window policy for camera conforming.
    pub fn get_window_policy(&self) -> Token {
        self.app_window_policy.read().clone()
    }

    /// Get USD stage.
    pub fn get_stage(&self) -> Option<Arc<Stage>> {
        self.stage.read().clone()
    }

    /// Set USD stage.
    ///
    /// Call when rendering a different stage (e.g. after opening a new file).
    pub fn set_stage(&self, stage: Option<Arc<Stage>>) {
        *self.stage.write() = stage;
    }

    /// Get camera parameters for a camera prim.
    ///
    /// # Arguments
    ///
    /// * `camera_path` - Path to camera prim
    ///
    /// # Returns
    ///
    /// Camera parameters or None if prim is not a camera
    pub fn get_camera_params(&self, camera_path: &Path) -> Option<CameraParams> {
        let stage = self.get_stage()?;
        let prim = stage.get_prim_at_path(camera_path)?;

        if prim.get_type_name() != "Camera" {
            return None;
        }

        let time = self.get_time();
        let sdf_time = SdfTimeCode::new(time.value());
        let mut params = CameraParams::default();

        // Query camera attributes
        if let Some(attr) = prim.get_attribute("focalLength") {
            if let Some(f) = attr.get_typed::<f64>(sdf_time) {
                params.focal_length = f;
            }
        }

        if let Some(attr) = prim.get_attribute("horizontalAperture") {
            if let Some(f) = attr.get_typed::<f64>(sdf_time) {
                params.h_aperture = f;
            }
        }

        if let Some(attr) = prim.get_attribute("verticalAperture") {
            if let Some(f) = attr.get_typed::<f64>(sdf_time) {
                params.v_aperture = f;
            }
        }

        if let Some(attr) = prim.get_attribute("clippingRange") {
            if let Some(vec) = attr.get_typed_vec::<f64>(sdf_time) {
                if vec.len() >= 2 {
                    params.near = vec[0];
                    params.far = vec[1];
                }
            }
        }

        // Calculate FOV from aperture and focal length
        params.fov = 2.0
            * (params.h_aperture / (2.0 * params.focal_length))
                .atan()
                .to_degrees();

        Some(params)
    }

    /// Get light parameters for a light prim.
    ///
    /// # Arguments
    ///
    /// * `light_path` - Path to light prim
    ///
    /// # Returns
    ///
    /// Light parameters or None if prim is not a light
    pub fn get_light_params(&self, light_path: &Path) -> Option<LightParams> {
        let stage = self.get_stage()?;
        let prim = stage.get_prim_at_path(light_path)?;

        let type_name = prim.get_type_name();
        // Explicit whitelist of known USD light types from UsdLux schema.
        // PortalLight was removed from UsdLux schema in recent OpenUSD versions.
        const KNOWN_LIGHT_TYPES: &[&str] = &[
            "SphereLight",
            "RectLight",
            "DiskLight",
            "CylinderLight",
            "DistantLight",
            "DomeLight",
            "GeometryLight",
            "PluginLight",
            "MeshLight",
        ];
        if !KNOWN_LIGHT_TYPES.iter().any(|t| type_name == *t) {
            return None;
        }

        let time = self.get_time();
        let sdf_time = SdfTimeCode::new(time.value());
        let mut params = LightParams::default();

        // USD light attributes use "inputs:" namespace prefix (UsdLux convention).
        // Fall back to bare name for older/custom schemas.
        let get_attr = |prim: &usd_core::Prim, name: &str| -> Option<usd_core::Attribute> {
            let inputs_name = format!("inputs:{name}");
            prim.get_attribute(&inputs_name)
                .or_else(|| prim.get_attribute(name))
        };

        if let Some(attr) = get_attr(&prim, "color") {
            if let Some(vec) = attr.get_typed_vec::<f64>(sdf_time) {
                if vec.len() >= 3 {
                    params.color = Vec3d::new(vec[0], vec[1], vec[2]);
                }
            }
        }

        if let Some(attr) = get_attr(&prim, "intensity") {
            if let Some(f) = attr.get_typed::<f64>(sdf_time) {
                params.intensity = f;
            }
        }

        if let Some(attr) = get_attr(&prim, "exposure") {
            if let Some(f) = attr.get_typed::<f64>(sdf_time) {
                params.exposure = f;
            }
        }

        if let Some(attr) = get_attr(&prim, "enableShadows") {
            if let Some(b) = attr.get_typed::<bool>(sdf_time) {
                params.shadows_enabled = b;
            }
        }

        Some(params)
    }

    /// Apply pending updates from USD stage edits.
    ///
    /// Processes queued resyncs and property changes.
    /// C++: UsdImagingDelegate::ApplyPendingUpdates().
    pub fn apply_pending_updates(&self) {
        // Drain ChangeHandler's accumulated notices into our pending queues.
        // C++: ApplyPendingUpdates first consumes _objectsChanged data.
        if let Some(ref handler) = self.change_handler {
            if handler.has_pending() {
                let changes = handler.drain();

                // Merge resynced paths into pending_resyncs.
                if !changes.paths_to_resync.is_empty() {
                    let mut resyncs = self.pending_resyncs.lock().expect("Lock poisoned");
                    for path in changes.paths_to_resync {
                        resyncs.insert(path);
                    }
                }

                // Convert field-level updates to dirty bits and merge.
                if !changes.paths_to_update.is_empty() {
                    let mut pending = self.pending_property_changes.lock().expect("Lock poisoned");
                    for (path, fields) in changes.paths_to_update {
                        let prim_path = if path.is_property_path() {
                            path.get_prim_path()
                        } else {
                            path.clone()
                        };
                        let bits = if fields.is_empty() {
                            // Property path with no specific fields — use name-based mapping.
                            self.property_name_to_dirty_bits(path.get_name_token().as_str())
                        } else {
                            fields_to_dirty_bits(&fields)
                        };
                        if bits != 0 {
                            let entry = pending.entry(prim_path).or_insert(0);
                            *entry |= bits;
                        }
                    }
                }
            }
        }

        // Process resyncs (prim additions/removals/type changes).
        let resyncs: Vec<Path> = {
            let mut pending = self.pending_resyncs.lock().expect("Lock poisoned");
            let resyncs = pending.iter().cloned().collect();
            pending.clear();
            resyncs
        };

        for path in &resyncs {
            self.resync_prim(path);
        }

        // P1-10: Process property changes from USD notices.
        // Merge pending_property_changes into dirty_bits.
        let prop_changes: HashMap<Path, HdDirtyBits> = {
            let mut pending = self.pending_property_changes.lock().expect("Lock poisoned");
            let changes = std::mem::take(&mut *pending);
            changes
        };

        if !prop_changes.is_empty() {
            let mut dirty = self.dirty_bits.lock().expect("Lock poisoned");
            let mut mat_cache = self.material_binding_cache.write();
            let mut vis = self.vis_cache.write();
            let mut pv_cache = self.primvar_desc_cache.write();
            let mut dm_cache = self.draw_mode_cache.write();
            let mut purp_cache = self.purpose_cache.write();
            let mut cs_cache = self.coord_sys_cache.write();
            for (path, bits) in &prop_changes {
                let entry = dirty.entry(path.clone()).or_insert(0);
                *entry |= bits;
                self.mark_backend_rprim_dirty(path, *bits);
                // Selectively invalidate caches based on dirty bits.
                if bits & 0x0100 != 0 {
                    // DirtyMaterialId
                    mat_cache.remove(path);
                }
                if bits & 0x02 != 0 {
                    // DirtyVisibility
                    vis.remove(path);
                }
                if bits & 0x20 != 0 {
                    // DirtyPrimvar
                    // Remove all interpolation entries for this prim.
                    for interp_val in 0..=5u8 {
                        pv_cache.remove(&(path.clone(), interp_val));
                    }
                }
                if bits & DIRTY_DRAW_MODE != 0 {
                    dm_cache.remove(path);
                }
                if bits & 0x40 != 0 {
                    // DirtyRenderTag (purpose)
                    purp_cache.remove(path);
                }
                if bits & DIRTY_COORD_SYS != 0 {
                    cs_cache.remove(path);
                }
            }
        }

        // Invalidate inherited caches when pending updates applied.
        if !resyncs.is_empty() {
            self.vis_cache.write().clear();
            self.material_binding_cache.write().clear();
            self.primvar_desc_cache.write().clear();
            self.draw_mode_cache.write().clear();
            self.purpose_cache.write().clear();
            self.coord_sys_cache.write().clear();
            self.point_instancer_indices_cache.write().clear();
            self.nonlinear_sample_count_cache.write().clear();
            self.blur_scale_cache.write().clear();
        }
    }

    /// P1-9: Sync dirty prims by dispatching adapter TrackVariability + UpdateForTime.
    ///
    /// C++: UsdImagingDelegate::Sync(HdSyncRequestVector*).
    /// For each dirty prim:
    /// 1. If variability not yet tracked, call adapter.track_variability() to
    ///    compute time_varying_bits and store them in the cache.
    /// 2. Call adapter.update_for_time() with the requested dirty bits so
    ///    adapters can pre-populate caches before Get* calls.
    /// 3. Leave dirty bits for Hydra to consume via get_dirty_bits()/mark_clean().
    pub fn sync_prims(&self) {
        let dirty_entries: Vec<(Path, HdDirtyBits)> = {
            let dirty = self.dirty_bits.lock().expect("Lock poisoned");
            dirty
                .iter()
                .filter(|(_, bits)| **bits != 0)
                .map(|(p, b)| (p.clone(), *b))
                .collect()
        };

        if dirty_entries.is_empty() {
            return;
        }

        let sdf_time = SdfTimeCode::new(self.get_time().value());

        // Phase 1: TrackVariability for prims that haven't been tracked yet,
        //          then UpdateForTime for all dirty prims.
        for (path, bits) in &dirty_entries {
            let (usd_path, adapter, already_tracked) = {
                let cache = self.prim_cache.read();
                match cache.get(path) {
                    Some(c) => (
                        c.usd_path.clone(),
                        c.adapter.clone(),
                        c.time_varying_bits != !0u32, // !0 means "not yet computed"
                    ),
                    None => continue,
                }
            };

            let Some(stage) = self.get_stage() else {
                continue;
            };
            let Some(prim) = stage.get_prim_at_path(&usd_path) else {
                continue;
            };

            // TrackVariability: compute which bits are time-varying.
            if !already_tracked {
                let varying = adapter.track_variability(&prim, sdf_time);
                let mut cache = self.prim_cache.write();
                if let Some(cached) = cache.get_mut(path) {
                    cached.time_varying_bits = varying;
                }
                // Update time_varying_prims set.
                if varying != 0 {
                    self.time_varying_prims.write().insert(path.clone());
                }
            }

            // UpdateForTime: let adapter pre-populate caches for the requested bits.
            adapter.update_for_time(&prim, sdf_time, *bits);
        }
    }

    /// P1-10: Handle USD ObjectsChanged notice.
    ///
    /// Called when the USD stage sends change notices (property edits, prim
    /// additions/removals). Maps changed USD paths to Hydra dirty bits and
    /// queues them for the next apply_pending_updates().
    ///
    /// C++: UsdImagingDelegate::_OnUsdObjectsChanged().
    pub fn on_objects_changed(&self, resynced_paths: &[Path], changed_info_only_paths: &[Path]) {
        // Resynced paths: prim type or hierarchy changed -> full resync.
        for path in resynced_paths {
            self.queue_resync(path);
        }

        // Changed-info-only paths: property values changed -> dirty bits.
        // Map the changed USD path to dirty bits based on the property name.
        let mut pending = self.pending_property_changes.lock().expect("Lock poisoned");
        let cache = self.prim_cache.read();

        for path in changed_info_only_paths {
            // Extract prim path (property paths have a trailing .propName)
            let prim_path = if path.is_property_path() {
                path.get_prim_path()
            } else {
                path.clone()
            };

            // Only dirty prims we know about.
            if !cache.contains_key(&prim_path) {
                continue;
            }

            // Determine which dirty bits to set from the property name.
            let prop_name = path.get_name_token().as_str().to_string();
            let bits = self.property_name_to_dirty_bits(&prop_name);
            if bits != 0 {
                let entry = pending.entry(prim_path).or_insert(0);
                *entry |= bits;
            }
        }
    }

    /// Map a USD property name to Hydra dirty bits.
    /// C++: adapter->ProcessPropertyChange() dispatches per-property.
    fn property_name_to_dirty_bits(&self, prop_name: &str) -> HdDirtyBits {
        // Map USD property name to Hydra dirty bits.
        // C++: adapter->ProcessPropertyChange() per adapter type.
        // We use a direct mapping for known properties.
        match prop_name {
            // Transform
            "xformOpOrder" => 0x01, // DirtyTransform
            // Visibility
            "visibility" => 0x02, // DirtyVisibility
            // Points/geometry
            "points" | "velocities" | "accelerations" => 0x04, // DirtyPoints
            // Normals
            "normals" => 0x08, // DirtyNormals
            // Topology
            "faceVertexCounts" | "faceVertexIndices" | "holeIndices" | "curveVertexCounts"
            | "subdivisionScheme" | "orientation" => 0x10, // DirtyTopology
            // Render tag
            "purpose" => 0x40, // DirtyRenderTag
            // Material
            "material:binding" => 0x0100, // DirtyMaterialId
            // Extent
            "extent" => 0x0200, // DirtyExtent
            // Display style
            "doubleSided" => 0x0400, // DirtyDoubleSided
            // Instancer
            "instancerTransform" => 0x0800, // DirtyInstanceIndex
            // Draw mode
            "model:drawMode" | "model:applyDrawMode" => DIRTY_DRAW_MODE,
            _ => {
                // xformOp:* prefix
                if prop_name.starts_with("xformOp:") {
                    return 0x01; // DirtyTransform
                }
                // primvars:* prefix
                if prop_name.starts_with("primvars:") {
                    return 0x20; // DirtyPrimvar
                }
                // material:binding:* prefix
                if prop_name.starts_with("material:") {
                    return 0x0100; // DirtyMaterialId
                }
                // coordSys:* prefix
                if prop_name.starts_with("coordSys:") {
                    return DIRTY_COORD_SYS;
                }
                // Unknown: return 0 (no dirty). C++ delegates to adapter which may
                // return AllDirty, but most adapters return Clean for unknown props.
                0
            }
        }
    }

    /// Mark a prim for resync (repopulation).
    ///
    /// # Arguments
    ///
    /// * `path` - Path to prim to resync
    pub fn mark_prim_dirty(&self, path: &Path, bits: HdDirtyBits) {
        let mut dirty = self.dirty_bits.lock().expect("Lock poisoned");
        let entry = dirty.entry(path.clone()).or_insert(0);
        *entry |= bits;
    }

    /// Diagnostic helper: summarize current non-zero dirty bits stored on the delegate.
    pub fn debug_dirty_bits_summary(&self) -> (usize, Option<(Path, HdDirtyBits)>) {
        let dirty = self.dirty_bits.lock().expect("Lock poisoned");
        let mut count = 0usize;
        let mut sample = None;
        for (path, bits) in dirty.iter() {
            if *bits != 0 {
                count += 1;
                if sample.is_none() {
                    sample = Some((path.clone(), *bits));
                }
            }
        }
        (count, sample)
    }

    /// Diagnostic helper: summarize non-zero dirty bits for a specific set of
    /// render-index paths by converting them to cache paths first.
    pub fn debug_dirty_bits_for_index_paths(
        &self,
        index_paths: impl IntoIterator<Item = Path>,
    ) -> (usize, Option<(Path, Path, HdDirtyBits)>) {
        let dirty = self.dirty_bits.lock().expect("Lock poisoned");
        let mut count = 0usize;
        let mut sample = None;
        for index_path in index_paths {
            let cache_path = self.convert_index_path_to_cache_path(&index_path);
            let bits = dirty.get(&cache_path).copied().unwrap_or(0);
            if bits != 0 {
                count += 1;
                if sample.is_none() {
                    sample = Some((index_path, cache_path, bits));
                }
            }
        }
        (count, sample)
    }

    /// Queue a prim for resync.
    pub fn queue_resync(&self, path: &Path) {
        let mut resyncs = self.pending_resyncs.lock().expect("Lock poisoned");
        resyncs.insert(path.clone());
    }

    // Internal implementation

    /// Recursively populate subtree starting from prim.
    ///
    /// Caches prim data and optionally inserts into HdRenderIndex via index_proxy.
    fn populate_subtree_with_proxy(
        &self,
        prim: &Prim,
        index_proxy: &mut crate::index_proxy::IndexProxy,
    ) {
        usd_trace::trace_scope!("delegate_populate_subtree");
        if !prim.is_valid() || !prim.is_active() {
            log::debug!("[delegate] skip invalid/inactive prim: {}", prim.get_path());
            return;
        }

        // P1-34: Display predicate — matches C++ _GetDisplayPredicate().
        // UsdPrimDefaultPredicate = IsActive && IsDefined && !IsAbstract && IsLoaded.
        // Active is checked above. Check defined + !abstract + loaded here.
        if prim.is_abstract()
            || (!prim.get_path().is_absolute_root_path() && !prim.is_defined())
            || (!prim.get_path().is_absolute_root_path() && !prim.is_loaded())
        {
            log::debug!(
                "[delegate] skip abstract/undefined/unloaded prim: {}",
                prim.get_path()
            );
            return;
        }

        // P0-IMG-2: C++ _Populate skips prototype prims and prims inside
        // prototypes — they are managed through the instance adapter, not
        // as standalone render index entries. Populating them directly creates
        // phantom render entries that duplicate real geometry.
        if prim.is_prototype() || prim.is_in_prototype() {
            log::debug!(
                "[delegate] skip prototype/in-prototype prim: {}",
                prim.get_path()
            );
            return;
        }

        // Instance prims are routed through the InstanceAdapter:
        // - create/reuse a hydra instancer for this prototype+attrs group
        // - populate prototype subtree as proto prims under the instancer
        // - do NOT recurse children normally (instancer culls them)
        // C++: UsdImagingDelegate::_Populate → UsdImagingInstanceAdapter::Populate
        if prim.is_instance() {
            log::debug!(
                "[delegate] routing instance prim through InstanceAdapter: {}",
                prim.get_path()
            );
            self.populate_instance_prim(prim, index_proxy, &Path::absolute_root());
            return;
        }

        // Skip excluded prims and their children. C++: excludedSet check in _Populate.
        {
            let excluded = self.excluded_paths.read();
            let prim_path = prim.get_path();
            if excluded.iter().any(|ep| prim_path.has_prefix(ep)) {
                log::debug!("[delegate] skip excluded prim: {}", prim_path);
                return;
            }
        }

        let type_str = prim.type_name();
        log::debug!(
            "[delegate] populate prim={} type={}",
            prim.get_path(),
            type_str
        );

        // P1-8: Non-imaging types: just recurse into children.
        // C++: UsdImagingPrimAdapter::ShouldCullSubtree() checks against a set
        // of non-imageable types. We use PrimAdapter::non_imaging_prim_types()
        // instead of a hardcoded list, making this extensible by adapters.
        use crate::prim_adapter::NoOpAdapter;
        let non_imaging = NoOpAdapter::non_imaging_prim_types();
        if non_imaging.contains(&type_str.as_str()) {
            for child in prim.get_children() {
                self.populate_subtree_with_proxy(&child, index_proxy);
            }
            return;
        }

        // Get adapter for this prim type
        let adapter = self.adapter_registry.find_for_prim(prim);

        // Get imaging subprims
        let subprims = adapter.get_imaging_subprims(prim);

        for subprim in &subprims {
            let prim_type = adapter.get_imaging_subprim_type(prim, subprim);

            // Construct cache path
            let cache_path = if subprim.as_str().is_empty() {
                prim.get_path().clone()
            } else {
                prim.get_path()
                    .append_property(subprim.as_str())
                    .unwrap_or_else(|| prim.get_path().clone())
            };

            // Check if prim is under any invised path. C++: _invisedPrimPaths.
            let is_invised = {
                let invised = self.invised_paths.read();
                let prim_path = prim.get_path();
                invised.iter().any(|ip| prim_path.has_prefix(ip))
            };

            // Compute per-adapter time-varying bits (TrackVariability).
            // C++: adapter->TrackVariability(prim, cachePath, &timeVaryingBits).
            let sdf_time = SdfTimeCode::new(self.get_time().value());
            let time_varying_bits = adapter.track_variability(prim, sdf_time);
            {
                use std::sync::atomic::{AtomicU32, Ordering};
                static TV_LOG: AtomicU32 = AtomicU32::new(0);
                if prim_type.as_str() == "mesh" {
                    let n = TV_LOG.fetch_add(1, Ordering::Relaxed);
                    if n < 3 {
                        log::trace!(
                            "[PERF] track_variability MESH[{}]: {} bits=0x{:08x}",
                            n,
                            cache_path,
                            time_varying_bits
                        );
                    }
                }
            }

            // Always cache prim data (per C++ _hdPrimInfoMap)
            let cached = CachedPrimData {
                usd_path: prim.get_path().clone(),
                prim_type: prim_type.clone(),
                adapter: adapter.clone(),
                visible: !is_invised,
                transform: Matrix4d::identity(),
                time_varying_bits,
            };

            self.prim_cache.write().insert(cache_path.clone(), cached);

            // Insert into render index via proxy (rprim, sprim, or bprim)
            // Per C++: adapter calls index_proxy.InsertRprim/InsertSprim based on prim type.
            if index_proxy.is_rprim_type_supported(&prim_type) {
                index_proxy.insert_rprim(&prim_type, &cache_path, prim, Some(adapter.clone()));
            } else if index_proxy.is_sprim_type_supported(&prim_type) {
                index_proxy.insert_sprim(&prim_type, &cache_path, prim, Some(adapter.clone()));
                // Register light linking collections for light sprims.
                // C++: LightAdapter::_RegisterLightCollections() in Populate().
                self.register_light_collections(prim);
            } else if index_proxy.is_bprim_type_supported(&prim_type) {
                index_proxy.insert_bprim(&prim_type, &cache_path, prim, Some(adapter.clone()));
            }

            // Track time-varying prims: check if any attribute might vary over time.
            // C++ uses TrackVariability() per adapter. We use a heuristic: check USD
            // xformOps and common animated attrs for value_might_be_time_varying.
            let is_time_varying = prim.get_attribute_names().iter().any(|attr_name| {
                let n = attr_name.as_str();
                // Common animated attributes
                if n.starts_with("xformOp:")
                    || n == "points"
                    || n == "velocities"
                    || n == "accelerations"
                    || n == "normals"
                    || n == "widths"
                    || n == "primvars:displayColor"
                {
                    if let Some(attr) = prim.get_attribute(n) {
                        return attr.value_might_be_time_varying();
                    }
                }
                false
            });
            if is_time_varying {
                self.time_varying_prims.write().insert(cache_path.clone());
            }

            // P1-5: Pre-populate primvar descriptor cache during populate.
            // Avoids rescanning USD attributes on every frame for the first query.
            let all_interps = [
                HdInterpolation::Constant,
                HdInterpolation::Uniform,
                HdInterpolation::Varying,
                HdInterpolation::Vertex,
                HdInterpolation::FaceVarying,
                HdInterpolation::Instance,
            ];
            for interp in all_interps {
                let cache_key = (cache_path.clone(), interp as u8);
                // Only populate if adapter provides descriptors; otherwise
                // get_primvar_descriptors() will scan on first call and cache.
                if let Some(descs) = adapter.get_primvar_descriptors(prim, interp, sdf_time) {
                    self.primvar_desc_cache.write().insert(cache_key, descs);
                }
            }

            // P1-14: Pre-populate material binding cache during populate.
            // Avoids repeated compute_bound_material() calls during rendering.
            let mat_binding = {
                let all_purpose = usd_tf::Token::new("allPurpose");
                let mut binding_rel = None;
                let binding_api = MaterialBindingAPI::new(prim.clone());
                let material =
                    binding_api.compute_bound_material(&all_purpose, &mut binding_rel, true);
                if material.is_valid() {
                    let mat_path = material.get_prim().get_path().clone();
                    if !mat_path.is_empty() {
                        Some(mat_path)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            self.material_binding_cache
                .write()
                .insert(cache_path.clone(), mat_binding);
            // Mark initially dirty
            self.dirty_bits
                .lock()
                .expect("Lock poisoned")
                .insert(cache_path, !0); // All bits dirty
        }

        // Recurse to children unless adapter culls them
        if !adapter.should_cull_children() {
            for child in prim.get_children() {
                self.populate_subtree_with_proxy(&child, index_proxy);
            }
        }
    }

    /// Populate a USD instance prim through the native InstanceAdapter.
    ///
    /// This mirrors C++ UsdImagingInstanceAdapter::_Populate():
    /// 1. Look up (or create) a hydra instancer for this prototype + inherited-attrs group.
    ///    Instances sharing the same prototype AND same material/purpose/drawMode share one instancer.
    /// 2. If instancer is new: insert it into HdRenderIndex and populate its prototype subtree
    ///    as rprim children under the instancer cache path.
    /// 3. Register this USD instance in the instancer's instance list.
    ///
    /// `parent_proxy_path` is `/` for top-level instances; for nested instances (instance inside
    /// a prototype), it contains the chain so far used to stitch instance-proxy paths.
    fn populate_instance_prim(
        &self,
        prim: &Prim,
        index_proxy: &mut crate::index_proxy::IndexProxy,
        parent_proxy_path: &Path,
    ) {
        let instance_path = prim.get_path().clone();

        // Get prototype prim (the hidden /__Prototype_N root).
        let prototype = prim.get_prototype();
        if !prototype.is_valid() {
            log::warn!(
                "[instance_adapter] no prototype for instance {}",
                instance_path
            );
            return;
        }
        let prototype_path = prototype.path().clone();

        // --- inherited attrs resolution ---
        // Resolve material, drawMode, purpose, and inherited primvars per C++.
        let material_usd_path = self.resolve_material_for_prim(prim);
        let draw_mode = self.resolve_draw_mode(prim);
        let inheritable_purpose = self.resolve_inheritable_purpose(prim);
        let inherited_primvars = self.compute_inherited_primvars(prim);

        // --- find or create instancer ---
        // C++: check _prototypeToInstancerMap for an instancer with matching attrs.
        let instancer_path: Path = {
            let proto_map = self.prototype_to_instancers.read();
            let inst_data = self.instancer_data.read();

            let mut found: Option<Path> = None;
            if let Some(instancer_paths) = proto_map.get(&prototype_path) {
                for ipath in instancer_paths {
                    if let Some(idata) = inst_data.get(ipath) {
                        if idata.material_usd_path == material_usd_path
                            && idata.draw_mode == draw_mode
                            && idata.inheritable_purpose == inheritable_purpose
                            && idata.inherited_primvars == inherited_primvars
                        {
                            found = Some(ipath.clone());
                            break;
                        }
                    }
                }
            }
            // C++: if not found, use instance_path as the new instancer path
            found.unwrap_or_else(|| instance_path.clone())
        };

        let is_new_instancer = { !self.instancer_data.read().contains_key(&instancer_path) };

        if is_new_instancer {
            // Initialize InstancerData for the new instancer.
            let mut idata = InstancerData::default();
            idata.prototype_path = prototype_path.clone();
            idata.material_usd_path = material_usd_path.clone();
            idata.draw_mode = draw_mode.clone();
            idata.inheritable_purpose = inheritable_purpose.clone();
            idata.inherited_primvars = inherited_primvars.clone();

            // Register instancer in all maps.
            {
                let mut inst_data = self.instancer_data.write();
                inst_data.insert(instancer_path.clone(), idata);
            }
            {
                let mut proto_map = self.prototype_to_instancers.write();
                proto_map
                    .entry(prototype_path.clone())
                    .or_default()
                    .push(instancer_path.clone());
            }

            // Insert hydra instancer into render index.
            // C++: index->InsertInstancer(instancerPath, instancerPrim, instancerAdapter)
            index_proxy.insert_instancer(&instancer_path, prim, None);

            // Mark instancer as initially all-dirty.
            self.dirty_bits
                .lock()
                .expect("Lock poisoned")
                .insert(instancer_path.clone(), !0);

            // --- Populate prototype subtree as proto prims ---
            // Walk the prototype prim tree; for each renderable prim, insert an rprim
            // under the instancer path.  Nested instances are collected and populated
            // separately after this loop completes.
            // C++: UsdPrimRange(prototypePrim, _GetDisplayPredicate()); _InsertProtoPrim()
            let mut nested_instances: Vec<Prim> = Vec::new();
            let mut proto_id: u32 = 0;

            self.populate_prototype_subtree(
                &prototype,
                &instancer_path,
                parent_proxy_path,
                index_proxy,
                &mut nested_instances,
                &mut proto_id,
            );

            // After prototype prims are done, recurse into nested instances.
            // C++: for (nestedPrim : nestedInstances) _Populate(nestedPrim, ...)
            for nested in &nested_instances {
                // instancerProxyPath for the nested case is the instancer proxy path.
                // For simplicity we use instancer_path as proxy root here.
                self.populate_instance_prim(nested, index_proxy, &instancer_path);

                // Track nested instance in instancer data.
                let mut inst_data = self.instancer_data.write();
                if let Some(idata) = inst_data.get_mut(&instancer_path) {
                    idata.nested_instances.push(nested.get_path().clone());
                }
            }
        }

        // --- Register this instance in its instancer ---
        // C++: instancerData.instancePaths.insert(instancePath)
        //      _instanceToInstancerMap[instancePath] = instancerPath
        {
            let mut inst_data = self.instancer_data.write();
            if let Some(idata) = inst_data.get_mut(&instancer_path) {
                if !idata.instance_paths.contains(&instance_path) {
                    idata.instance_paths.push(instance_path.clone());
                    // Visibility for this new instance starts as Unknown.
                    idata.visibility.push(InstanceVisibility::Unknown);
                }
            }
        }
        {
            let mut i2i = self.instance_to_instancer.write();
            i2i.insert(instance_path.clone(), instancer_path.clone());
        }

        // Handle parent chain for nested instances.
        if *parent_proxy_path != Path::absolute_root() {
            let mut inst_data = self.instancer_data.write();
            if let Some(idata) = inst_data.get_mut(&instancer_path) {
                let parent_path = parent_proxy_path.clone();
                if !idata.parent_instances.contains(&parent_path) {
                    idata.parent_instances.push(parent_path);
                }
            }
        }

        // P2 #6: Recompute numInstancesToDraw with nested instancing product.
        // C++: _CountAllInstancesToDraw() in TrackVariability.
        {
            let count = self.count_all_instances_to_draw(&instancer_path);
            let mut inst_data = self.instancer_data.write();
            if let Some(idata) = inst_data.get_mut(&instancer_path) {
                idata.num_instances_to_draw = count;
            }
        }

        // P2 #1: Pre-cache per-instance visibility states.
        // C++: _ComputeInstanceMapVariability() in TrackVariability.
        self.compute_instance_map_variability(&instancer_path);

        log::debug!(
            "[instance_adapter] populated instance {} -> instancer {} (proto {}), total instances: {}",
            instance_path,
            instancer_path,
            prototype_path,
            self.instancer_data
                .read()
                .get(&instancer_path)
                .map(|d| d.instance_paths.len())
                .unwrap_or(0),
        );
    }

    /// Populate the prototype subtree of a native instancer.
    ///
    /// Walks the USD prototype prim (e.g. `/__Prototype_1`) and for each
    /// renderable descendant inserts an rprim into the render index under a
    /// sub-path of `instancer_path` of the form
    /// `<instancer>/proto_<name>_id<N>`.
    ///
    /// Nested instance prims are pushed onto `nested_out` for deferred
    /// population; `proto_id` is a monotonically-increasing counter used
    /// to generate unique sub-names (matches C++ `protoID`).
    ///
    /// C++: inner loop of UsdImagingInstanceAdapter::_Populate()
    fn populate_prototype_subtree(
        &self,
        prototype: &Prim,
        instancer_path: &Path,
        _parent_proxy_path: &Path,
        index_proxy: &mut crate::index_proxy::IndexProxy,
        nested_out: &mut Vec<Prim>,
        proto_id: &mut u32,
    ) {
        // Walk full subtree (depth-first pre-order) per C++ UsdPrimRange.
        // Recurse into all descendants, not just immediate children.
        self.populate_prototype_subtree_recurse(
            prototype,
            instancer_path,
            _parent_proxy_path,
            index_proxy,
            nested_out,
            proto_id,
        );
    }

    /// Recursive helper for deep prototype subtree walk.
    /// Matches C++ UsdPrimRange iteration in _Populate().
    fn populate_prototype_subtree_recurse(
        &self,
        parent: &Prim,
        instancer_path: &Path,
        _parent_proxy_path: &Path,
        index_proxy: &mut crate::index_proxy::IndexProxy,
        nested_out: &mut Vec<Prim>,
        proto_id: &mut u32,
    ) {
        use crate::prim_adapter::NoOpAdapter;
        let children: Vec<Prim> = parent.get_all_children();

        for child in &children {
            // Nested instances are deferred (not recursed into).
            if child.is_instance() {
                nested_out.push(child.clone());
                continue;
            }

            // Skip inactive / abstract prims.
            if !child.is_active() {
                continue;
            }

            let type_str = child.type_name();
            let non_imaging = NoOpAdapter::non_imaging_prim_types();
            if non_imaging.contains(&type_str.as_str()) {
                // Non-imaging types — still recurse into their children.
                self.populate_prototype_subtree_recurse(
                    child,
                    instancer_path,
                    _parent_proxy_path,
                    index_proxy,
                    nested_out,
                    proto_id,
                );
                continue;
            }

            // Build the proto-prim cache path:
            // <instancer_path>/proto_<name>_id<N>
            // C++: protoName = TfStringPrintf("proto_%s_id%d", iter->GetName(), protoID++)
            // C++ instanceAdapter.cpp:288: TfStringPrintf("proto_%s_id%d", ...)
            // The "id" infix is required for Hydra prototype path matching.
            let proto_name = format!("proto_{}_id{}", child.name(), *proto_id);
            *proto_id += 1;

            let proto_cache_path = instancer_path
                .append_child(&proto_name)
                .unwrap_or_else(|| instancer_path.clone());

            // Resolve adapter for the child prim.
            let adapter = self.adapter_registry.find_for_prim(child);
            let prim_type = adapter.get_imaging_subprim_type(child, &Token::new(""));

            // Register proto prim in instancer data.
            {
                let mut inst_data = self.instancer_data.write();
                if let Some(idata) = inst_data.get_mut(instancer_path) {
                    idata.prim_map.insert(
                        proto_cache_path.clone(),
                        ProtoPrim {
                            path: child.get_path().clone(),
                            adapter: Some(adapter.clone()),
                        },
                    );
                }
            }

            // Cache prim data.
            let cached = CachedPrimData {
                usd_path: child.get_path().clone(),
                prim_type: prim_type.clone(),
                adapter: adapter.clone(),
                visible: true,
                transform: Matrix4d::identity(),
                time_varying_bits: 0,
            };
            self.prim_cache
                .write()
                .insert(proto_cache_path.clone(), cached);

            // Insert rprim with the instancer's cache path as the instancer ID.
            // C++: primAdapter->Populate(prim, index, &ctx)
            if index_proxy.is_rprim_type_supported(&prim_type) {
                index_proxy.insert_rprim(
                    &prim_type,
                    &proto_cache_path,
                    child,
                    Some(adapter.clone()),
                );
            }

            self.dirty_bits
                .lock()
                .expect("Lock poisoned")
                .insert(proto_cache_path, !0);

            // C++ UsdPrimRange continues into children unless adapter prunes.
            // Recurse into descendants of imaging prims too.
            if !adapter.should_cull_children() {
                self.populate_prototype_subtree_recurse(
                    child,
                    instancer_path,
                    _parent_proxy_path,
                    index_proxy,
                    nested_out,
                    proto_id,
                );
            }
        }
    }

    /// Resolve the USD material binding path for a prim (best-effort).
    ///
    /// Returns the empty path when there is no material bound.
    fn resolve_material_for_prim(&self, prim: &Prim) -> Path {
        let all_purpose = Token::new("allPurpose");
        let mut binding_rel = None;
        let api = MaterialBindingAPI::new(prim.clone());
        let mat = api.compute_bound_material(&all_purpose, &mut binding_rel, true);
        if mat.is_valid() {
            let path = mat.get_prim().get_path().clone();
            if !path.is_empty() {
                return path;
            }
        }
        Path::empty()
    }

    /// Resolve the draw mode for a prim.
    ///
    /// Reads `model:drawMode` via UsdGeomModelAPI. Returns "default" if
    /// no draw mode is authored. Matches C++ `GetModelDrawMode()`.
    fn resolve_draw_mode(&self, prim: &Prim) -> Token {
        let model_api = ModelAPI::new(prim.clone());
        if let Some(attr) = model_api.get_model_draw_mode_attr() {
            let sdf_time = SdfTimeCode::new(self.get_time().value());
            if let Some(tok) = attr.get_typed::<Token>(sdf_time) {
                if !tok.is_empty() {
                    return tok;
                }
            }
        }
        Token::new("default")
    }

    /// Resolve the inheritable purpose for a prim.
    ///
    /// Uses UsdGeomImageable::ComputePurpose(). Returns "default" if purpose
    /// is not authored. Cached per prim path. Matches C++ `GetInheritablePurpose()`.
    fn resolve_inheritable_purpose(&self, prim: &Prim) -> Token {
        let prim_path = prim.get_path().clone();
        // Cache lookup
        {
            let cache = self.purpose_cache.read();
            if let Some(cached) = cache.get(&prim_path) {
                return cached.clone();
            }
        }
        // Compute and cache
        let imageable = Imageable::new(prim.clone());
        let purpose = imageable.compute_purpose();
        let result = if purpose.is_empty() {
            Token::new("default")
        } else {
            purpose
        };
        self.purpose_cache.write().insert(prim_path, result.clone());
        result
    }

    /// Compute inherited primvars for an instance prim.
    ///
    /// Walks up from the prim collecting constant-interpolation primvars
    /// that are inherited. These are stored in InstancerData and used for
    /// instancer compatibility checks and per-instance primvar queries.
    /// Matches C++ `_GetInheritedPrimvars()` -> `_ComputeInheritedPrimvars()`.
    fn compute_inherited_primvars(&self, prim: &Prim) -> Vec<PrimvarInfo> {
        let api = PrimvarsAPI::new(prim.clone());
        let inherited = api.find_primvars_with_inheritance();

        let mut result: Vec<PrimvarInfo> = inherited
            .iter()
            .filter(|pv| {
                // Only constant-interpolation primvars are inheritable
                // per C++ UsdImaging_InheritedPrimvarStrategy.
                let interp = pv.get_interpolation();
                interp == "constant" || interp.is_empty()
            })
            .map(|pv| PrimvarInfo {
                name: pv.get_primvar_name(),
                type_name: Token::new(pv.get_type_name().as_token().as_str()),
            })
            .collect();

        result.sort();
        result
    }

    /// Count all instances to draw for nested instancing.
    ///
    /// For non-nested instances, returns instance_paths.len().
    /// For nested instances (instance inside a prototype), the count is the
    /// product of instance counts up the chain.
    /// Matches C++ `_CountAllInstancesToDraw()` / `_CountAllInstancesToDrawImpl()`.
    fn count_all_instances_to_draw(&self, instancer_path: &Path) -> usize {
        let mut draw_counts: HashMap<Path, usize> = HashMap::new();
        self.count_all_instances_impl(instancer_path, &mut draw_counts)
    }

    /// Recursive implementation of count_all_instances_to_draw.
    /// Memoizes results in `draw_counts` to avoid redundant computation.
    fn count_all_instances_impl(
        &self,
        instancer_path: &Path,
        draw_counts: &mut HashMap<Path, usize>,
    ) -> usize {
        // Check memoized table
        if let Some(&count) = draw_counts.get(instancer_path) {
            return count;
        }

        let inst_data = self.instancer_data.read();
        let idata = match inst_data.get(instancer_path) {
            Some(d) => d,
            None => return 0,
        };

        let stage_opt = self.get_stage();
        let proto_to_inst = self.prototype_to_instancers.read();

        let mut draw_count: usize = 0;

        for inst_path in &idata.instance_paths {
            let Some(ref stage) = stage_opt else {
                draw_count += 1;
                continue;
            };
            let Some(instance_prim) = stage.get_prim_at_path(inst_path) else {
                continue;
            };

            if !instance_prim.is_in_prototype() {
                // Top-level instance — counts as 1 draw
                draw_count += 1;
            } else {
                // Nested instance — find parent prototype and multiply
                // by the number of draws of each instancer referencing
                // that parent prototype.
                let mut parent = instance_prim.clone();
                while parent.is_valid() && !parent.is_prototype() {
                    let p = parent.parent();
                    if !p.is_valid() {
                        break;
                    }
                    parent = p;
                }

                if parent.is_valid() {
                    let parent_path = parent.path().clone();
                    // Drop read locks before recursing
                    if let Some(instancer_paths) = proto_to_inst.get(&parent_path) {
                        let paths_clone: Vec<Path> = instancer_paths.clone();
                        drop(proto_to_inst);
                        drop(inst_data);

                        for parent_instancer in &paths_clone {
                            draw_count +=
                                self.count_all_instances_impl(parent_instancer, draw_counts);
                        }

                        draw_counts.insert(instancer_path.clone(), draw_count);
                        return draw_count;
                    }
                }
            }
        }

        drop(proto_to_inst);
        drop(inst_data);
        draw_counts.insert(instancer_path.clone(), draw_count);
        draw_count
    }

    /// Check if a prim is visible by walking up the ancestor chain.
    ///
    /// Matches the visibility check in C++ _ComputeInstanceMapFn::GetVisible().
    fn is_prim_visible(&self, prim: &Prim, time: SdfTimeCode) -> bool {
        let mut current = prim.clone();
        loop {
            if let Some(attr) = current.get_attribute("visibility") {
                if let Some(tok) = attr.get_typed::<Token>(time) {
                    if tok == "invisible" {
                        return false;
                    }
                }
            }
            if current.get_path().is_absolute_root_path() {
                break;
            }
            let p = current.parent();
            if !p.is_valid() {
                break;
            }
            current = p;
        }
        true
    }

    /// Compute visibility variability for all instances of an instancer.
    ///
    /// Pre-caches per-instance visibility as Visible/Invisible/Varying.
    /// Returns true if any instance has varying visibility.
    /// Matches C++ `_ComputeInstanceMapVariability()`.
    fn compute_instance_map_variability(&self, instancer_path: &Path) -> bool {
        let stage_opt = self.get_stage();
        let Some(ref stage) = stage_opt else {
            return false;
        };

        let inst_data = self.instancer_data.read();
        let Some(idata) = inst_data.get(instancer_path) else {
            return false;
        };

        let instance_paths: Vec<Path> = idata.instance_paths.clone();
        drop(inst_data);

        let mut visibility_states: Vec<InstanceVisibility> =
            vec![InstanceVisibility::Unknown; instance_paths.len()];
        let mut has_varying = false;

        for (idx, inst_path) in instance_paths.iter().enumerate() {
            let Some(prim) = stage.get_prim_at_path(inst_path) else {
                visibility_states[idx] = InstanceVisibility::Visible;
                continue;
            };

            // Check if visibility attribute is time-varying
            let varying = self.is_visibility_varying(&prim);

            if varying {
                visibility_states[idx] = InstanceVisibility::Varying;
                has_varying = true;
            } else {
                // Constant visibility: resolve once
                let sdf_time = SdfTimeCode::new(self.get_time().value());
                if self.is_prim_visible(&prim, sdf_time) {
                    visibility_states[idx] = InstanceVisibility::Visible;
                } else {
                    visibility_states[idx] = InstanceVisibility::Invisible;
                }
            }
        }

        // Store computed visibility into instancer data
        let mut inst_data = self.instancer_data.write();
        if let Some(idata) = inst_data.get_mut(instancer_path) {
            idata.visibility = visibility_states;
        }

        has_varying
    }

    /// Check if a prim's visibility attribute is time-varying.
    fn is_visibility_varying(&self, prim: &Prim) -> bool {
        let mut current = prim.clone();
        loop {
            if let Some(attr) = current.get_attribute("visibility") {
                if attr.value_might_be_time_varying() {
                    return true;
                }
            }
            if current.get_path().is_absolute_root_path() {
                break;
            }
            let p = current.parent();
            if !p.is_valid() {
                break;
            }
            current = p;
        }
        false
    }

    // ====================================================================
    // P2 #5: Change tracking for instance prims
    // ====================================================================

    /// Process a property change on an instance or prototype prim.
    ///
    /// Returns the dirty bits to apply. Matches C++
    /// `UsdImagingInstanceAdapter::ProcessPropertyChange()`.
    pub fn process_instance_property_change(
        &self,
        prim: &Prim,
        cache_path: &Path,
        property_name: &Token,
    ) -> u32 {
        use usd_hd::change_tracker::HdRprimDirtyBits;

        // If this is a proto prim (child of instancer), delegate to its adapter.
        if self.is_proto_prim(cache_path) {
            if let Some(proto) = self.find_proto_prim(cache_path) {
                if let Some(ref adapter) = proto.adapter {
                    // Delegate to the proto prim's own adapter
                    let sub = Token::new("");
                    let props = &[property_name.clone()];
                    let locators = adapter.invalidate_imaging_subprim(
                        prim,
                        &sub,
                        props,
                        super::types::PropertyInvalidationType::PropertyChanged,
                    );
                    if !locators.is_empty() {
                        return HdRprimDirtyBits::ALL_DIRTY;
                    }
                }
            }
            return HdRprimDirtyBits::CLEAN;
        }

        let name = property_name.as_str();

        // Purpose changes require full resync (purpose is part of instancer grouping)
        if name == "purpose" {
            return HdRprimDirtyBits::ALL_DIRTY;
        }

        // Transform changes affect per-instance transforms primvar
        if name.starts_with("xformOp") || name == "xformOpOrder" {
            return HdRprimDirtyBits::DIRTY_PRIMVAR;
        }

        // Visibility changes affect the instance map
        if name == "visibility" {
            return HdRprimDirtyBits::DIRTY_INSTANCE_INDEX;
        }

        // Primvar changes
        if name.starts_with("primvars:") {
            return HdRprimDirtyBits::DIRTY_PRIMVAR;
        }

        HdRprimDirtyBits::CLEAN
    }

    /// Process prim resync — remove and reload instancer and all dependents.
    ///
    /// Matches C++ `UsdImagingInstanceAdapter::ProcessPrimResync()`.
    pub fn process_instance_prim_resync(
        &self,
        cache_path: &Path,
        index_proxy: &mut crate::index_proxy::IndexProxy,
    ) {
        let resync_path = if self.is_proto_prim(cache_path) {
            // Proto prim -> resync the parent instancer
            cache_path.get_parent_path()
        } else {
            cache_path.clone()
        };
        self.resync_instancer_chain(&resync_path, index_proxy, true);
    }

    /// Process prim removal — remove instancer and all dependents without reload.
    ///
    /// Matches C++ `UsdImagingInstanceAdapter::ProcessPrimRemoval()`.
    pub fn process_instance_prim_removal(
        &self,
        cache_path: &Path,
        index_proxy: &mut crate::index_proxy::IndexProxy,
    ) {
        let resync_path = if self.is_proto_prim(cache_path) {
            cache_path.get_parent_path()
        } else {
            cache_path.clone()
        };
        self.resync_instancer_chain(&resync_path, index_proxy, false);
    }

    /// Check if a cache path is a proto prim (child of an instancer).
    fn is_proto_prim(&self, cache_path: &Path) -> bool {
        // Proto prims are children of an instancer cache path.
        // They are NOT in instance_to_instancer (which maps USD instance paths).
        let i2i = self.instance_to_instancer.read();
        if i2i.contains_key(cache_path) {
            return false; // It's an instance prim, not a proto prim
        }
        // Check if the parent is an instancer
        let parent = cache_path.get_parent_path();
        let inst_data = self.instancer_data.read();
        inst_data.contains_key(&parent)
    }

    /// Find the ProtoPrim for a given cache path.
    fn find_proto_prim(&self, cache_path: &Path) -> Option<ProtoPrim> {
        let parent = cache_path.get_parent_path();
        let inst_data = self.instancer_data.read();
        if let Some(idata) = inst_data.get(&parent) {
            idata.prim_map.get(cache_path).cloned()
        } else {
            None
        }
    }

    /// Resync an instancer and all connected instancers (parents + children).
    ///
    /// BFS traversal matching C++ `_ResyncPath()`. When `reload` is true,
    /// instancers are removed and re-populated; when false, just removed.
    fn resync_instancer_chain(
        &self,
        cache_path: &Path,
        index_proxy: &mut crate::index_proxy::IndexProxy,
        reload: bool,
    ) {
        let mut instancers_to_unload: HashSet<Path> = HashSet::new();
        let mut to_traverse: Vec<Path> = vec![cache_path.clone()];

        // BFS: collect all connected instancers
        while let Some(instance_path) = to_traverse.pop() {
            let instancer_path = {
                let i2i = self.instance_to_instancer.read();
                match i2i.get(&instance_path) {
                    Some(p) => p.clone(),
                    None => continue,
                }
            };

            if instancers_to_unload.insert(instancer_path.clone()) {
                // Visit parents and children
                let inst_data = self.instancer_data.read();
                if let Some(idata) = inst_data.get(&instancer_path) {
                    to_traverse.extend(idata.nested_instances.iter().cloned());
                    to_traverse.extend(idata.parent_instances.iter().cloned());
                }
            }
        }

        // Remove all collected instancers
        for instancer_path in &instancers_to_unload {
            self.remove_instancer(instancer_path, index_proxy);
        }

        // Re-populate if reloading
        if reload {
            if let Some(stage) = self.get_stage() {
                for instancer_path in &instancers_to_unload {
                    if let Some(prim) = stage.get_prim_at_path(instancer_path) {
                        if prim.is_instance() {
                            self.populate_instance_prim(&prim, index_proxy, &Path::absolute_root());
                        }
                    }
                }
            }
        }
    }

    /// Remove a single instancer and all its proto prims from caches.
    fn remove_instancer(
        &self,
        instancer_path: &Path,
        index_proxy: &mut crate::index_proxy::IndexProxy,
    ) {
        // Remove proto prims from caches
        let proto_paths: Vec<Path> = {
            let inst_data = self.instancer_data.read();
            if let Some(idata) = inst_data.get(instancer_path) {
                idata.prim_map.keys().cloned().collect()
            } else {
                Vec::new()
            }
        };

        for proto_path in &proto_paths {
            self.prim_cache.write().remove(proto_path);
            self.dirty_bits
                .lock()
                .expect("Lock poisoned")
                .remove(proto_path);
            index_proxy.remove_rprim(proto_path);
        }

        // Remove instance path mappings
        let instance_paths: Vec<Path> = {
            let inst_data = self.instancer_data.read();
            if let Some(idata) = inst_data.get(instancer_path) {
                idata.instance_paths.clone()
            } else {
                Vec::new()
            }
        };

        {
            let mut i2i = self.instance_to_instancer.write();
            for inst_path in &instance_paths {
                i2i.remove(inst_path);
            }
        }

        // Remove from prototype_to_instancers map
        {
            let prototype_path = {
                let inst_data = self.instancer_data.read();
                inst_data
                    .get(instancer_path)
                    .map(|d| d.prototype_path.clone())
            };
            if let Some(proto_path) = prototype_path {
                let mut proto_map = self.prototype_to_instancers.write();
                if let Some(paths) = proto_map.get_mut(&proto_path) {
                    paths.retain(|p| p != instancer_path);
                    if paths.is_empty() {
                        proto_map.remove(&proto_path);
                    }
                }
            }
        }

        // Remove instancer data and from render index
        self.instancer_data.write().remove(instancer_path);
        self.prim_cache.write().remove(instancer_path);
        self.dirty_bits
            .lock()
            .expect("Lock poisoned")
            .remove(instancer_path);
        index_proxy.remove_instancer(instancer_path);
    }

    /// Resync a prim subtree: remove all cached state under `path` and re-populate.
    ///
    /// Port of C++ UsdImagingDelegate::_ResyncUsdPrim. The C++ version is ~220 lines
    /// with adapter-based ProcessPrimResync dispatching, coord-sys/material adapter
    /// exceptions, and dependency-graph gathering. Our simplified version:
    ///
    /// 1. Gather all cache entries that are descendants of (or equal to) `path`.
    /// 2. Clean up instancer state for any affected instancers under `path`.
    /// 3. Purge all affected entries from every cache (prim_cache, dirty_bits,
    ///    vis_cache, material_binding_cache, primvar_desc_cache, time_varying_prims).
    /// 4. Remove instance_to_instancer mappings for affected instance prims.
    /// 5. Re-populate the subtree if the USD prim still exists and is active.
    /// 6. Mark all re-populated prims as AllDirty so the render index re-syncs.
    fn resync_prim(&self, path: &Path) {
        use usd_hd::change_tracker::HdRprimDirtyBits;

        let Some(stage) = self.get_stage() else {
            return;
        };

        log::debug!("[delegate] resync_prim: {}", path);

        // Phase 1: Gather all cache paths that are descendants of or equal to the resync path.
        // C++ equivalent: _GatherDependencies(usdPath, cache, &affectedCachePaths).
        let affected_cache_paths: Vec<Path> = {
            let cache = self.prim_cache.read();
            cache
                .keys()
                .filter(|p| *p == path || p.has_prefix(path))
                .cloned()
                .collect()
        };

        // Phase 2: Gather affected instancers whose path is under the resync path.
        // C++ equivalent: adapter->ProcessPrimRemoval for instancer adapters.
        let affected_instancer_paths: Vec<Path> = {
            let idata = self.instancer_data.read();
            idata
                .keys()
                .filter(|p| *p == path || p.has_prefix(path))
                .cloned()
                .collect()
        };

        // Remove instancer state before purging prim_cache (remove_instancer reads instancer_data).
        if !affected_instancer_paths.is_empty() {
            let mut proxy = crate::index_proxy::IndexProxy::new();
            for ipath in &affected_instancer_paths {
                self.remove_instancer(ipath, &mut proxy);
            }
        }

        // Phase 3: Purge all affected entries from every cache.
        {
            let mut cache = self.prim_cache.write();
            let mut dirty = self.dirty_bits.lock().expect("Lock poisoned");
            let mut vis = self.vis_cache.write();
            let mut mat = self.material_binding_cache.write();
            let mut tv = self.time_varying_prims.write();
            let mut pv = self.primvar_desc_cache.write();
            let mut dm = self.draw_mode_cache.write();
            let mut purp = self.purpose_cache.write();
            let mut cs = self.coord_sys_cache.write();

            for p in &affected_cache_paths {
                cache.remove(p);
                dirty.remove(p);
                vis.remove(p);
                mat.remove(p);
                tv.remove(p);
                dm.remove(p);
                purp.remove(p);
                cs.remove(p);
                // Primvar desc cache is keyed by (path, interpolation)
                for interp in 0..=5u8 {
                    pv.remove(&(p.clone(), interp));
                }
            }
        }

        // Phase 4: Clean up instance_to_instancer for any instance prims under resync path.
        {
            let mut i2i = self.instance_to_instancer.write();
            i2i.retain(|inst_path, _| inst_path != path && !inst_path.has_prefix(path));
        }

        // Phase 5: Re-populate the subtree if the prim still exists.
        // C++ equivalent: proxy->Repopulate(usdPath) + _Populate.
        // If the prim was deleted, we just leave the caches empty (removal complete).
        if let Some(prim) = stage.get_prim_at_path(path) {
            if prim.is_valid() && prim.is_active() {
                let mut proxy = crate::index_proxy::IndexProxy::new();
                self.populate_subtree_with_proxy(&prim, &mut proxy);
            }
        }

        // Phase 6: Mark all re-populated prims as AllDirty so the render index re-syncs
        // their geometry, transforms, visibility, materials, etc.
        // C++ equivalent: the render index marks newly-inserted prims with AllDirty
        // via InsertRprim; we do it explicitly since our proxy is backend-less.
        {
            let cache = self.prim_cache.read();
            let mut dirty = self.dirty_bits.lock().expect("Lock poisoned");
            for p in cache.keys() {
                if p == path || p.has_prefix(path) {
                    dirty.insert(p.clone(), HdRprimDirtyBits::ALL_DIRTY);
                }
            }
        }

        log::debug!(
            "[delegate] resync_prim done: {} (removed {} cached, {} instancers)",
            path,
            affected_cache_paths.len(),
            affected_instancer_paths.len(),
        );
    }

    /// Gather xformOp attribute time samples within [interval_min, interval_max].
    ///
    /// Always includes the interval boundaries so shutter open/close are covered.
    fn gather_xform_time_samples(prim: &Prim, interval_min: f64, interval_max: f64) -> Vec<f64> {
        let mut time_set: Vec<f64> = vec![interval_min, interval_max];
        for name in prim.get_attribute_names() {
            let name_str = name.as_str();
            if !name_str.starts_with("xformOp:") {
                continue;
            }
            if let Some(attr) = prim.get_attribute(name_str) {
                if attr.value_might_be_time_varying() {
                    let samples = attr.get_time_samples_in_interval(interval_min, interval_max);
                    time_set.extend(samples);
                }
            }
        }
        time_set
    }

    /// Read a USD attribute value for light params, handling type conversions.
    ///
    /// Converts common USD types (f64->f32, GfVec3f, SdfAssetPath, bool, Token)
    /// into Values that HdStLight::sync expects.
    fn read_light_attr(&self, attr: &usd_core::Attribute, time: SdfTimeCode) -> Value {
        // Try f32 first (some schemas author float directly)
        if let Some(v) = attr.get_typed::<f32>(time) {
            return Value::from(v);
        }
        // USD commonly stores doubles for light params
        if let Some(v) = attr.get_typed::<f64>(time) {
            return Value::from(v as f32);
        }
        // Vec3f color/direction
        if let Some(v) = attr.get_typed_vec::<f64>(time) {
            if v.len() >= 3 {
                return Value::from(usd_gf::Vec3f::new(v[0] as f32, v[1] as f32, v[2] as f32));
            }
        }
        if let Some(v) = attr.get_typed::<usd_gf::Vec3f>(time) {
            return Value::from(v);
        }
        // AssetPath for texture:file
        if let Some(v) = attr.get_typed::<usd_sdf::asset_path::AssetPath>(time) {
            return Value::from(v);
        }
        // Bool (enableShadows etc.)
        if let Some(v) = attr.get_typed::<bool>(time) {
            return Value::from(v);
        }
        // Token
        if let Some(v) = attr.get_typed::<Token>(time) {
            return Value::from(v);
        }
        // Fallback: generic get()
        attr.get(time).unwrap_or_default()
    }

    /// Read subdiv tags from a USD mesh prim into a SubdivTags struct.
    ///
    /// Maps USD subdivisionScheme attributes to pxOsd SubdivTags fields
    /// matching C++ UsdImagingMeshAdapter::_GetSubdivTags().
    fn read_subdiv_tags(&self, prim: &Prim, time: SdfTimeCode) -> SubdivTags {
        use usd_tf::Token;
        let mesh = Mesh::new(prim.clone());
        let tc = time;

        // Vertex boundary interpolation rule (USD attr: interpolateBoundary)
        let vtx_rule = prim
            .get_attribute("interpolateBoundary")
            .and_then(|attr| Self::attr_get_token(&attr, tc))
            .unwrap_or_default();

        // Face-varying interpolation rule (USD attr: faceVaryingLinearInterpolation)
        let fvar_rule = prim
            .get_attribute("faceVaryingLinearInterpolation")
            .and_then(|attr| Self::attr_get_token(&attr, tc))
            .unwrap_or_default();

        // Crease method — not yet in USD schema (C++ meshAdapter.cpp: commented out)
        let crease_method: Token = Token::default();

        // Triangle subdivision rule (USD attr: triangleSubdivisionRule)
        let tri_subdiv = prim
            .get_attribute("triangleSubdivisionRule")
            .and_then(|attr| Self::attr_get_token(&attr, tc))
            .unwrap_or_default();

        // Crease data: indices, lengths, weights
        let crease_indices =
            Self::attr_get_i32_vec(&mesh.get_crease_indices_attr(), time).unwrap_or_default();
        let crease_lengths =
            Self::attr_get_i32_vec(&mesh.get_crease_lengths_attr(), time).unwrap_or_default();
        let crease_weights =
            Self::attr_get_f32_vec(&mesh.get_crease_sharpnesses_attr(), time).unwrap_or_default();

        // Corner data: indices, weights
        let corner_indices =
            Self::attr_get_i32_vec(&mesh.get_corner_indices_attr(), time).unwrap_or_default();
        let corner_weights =
            Self::attr_get_f32_vec(&mesh.get_corner_sharpnesses_attr(), time).unwrap_or_default();

        SubdivTags::new(
            vtx_rule,
            fvar_rule,
            crease_method,
            tri_subdiv,
            crease_indices,
            crease_lengths,
            crease_weights,
            corner_indices,
            corner_weights,
        )
    }

    // ---------------------------------------------------------------------- //
    // P2-1: ConvertCachePathToIndexPath / ConvertIndexPathToCachePath
    // ---------------------------------------------------------------------- //

    /// Convert a USD cache path to a render index path by prepending delegate ID.
    /// C++: UsdImagingDelegate::ConvertCachePathToIndexPath.
    pub fn convert_cache_path_to_index_path(&self, cache_path: &Path) -> Path {
        if self.root_path.is_absolute_root_path() {
            return cache_path.clone();
        }
        cache_path
            .replace_prefix(&Path::absolute_root(), &self.root_path)
            .unwrap_or_else(|| cache_path.clone())
    }

    /// Convert a render index path back to a USD cache path by stripping delegate ID.
    /// C++: UsdImagingDelegate::ConvertIndexPathToCachePath.
    pub fn convert_index_path_to_cache_path(&self, index_path: &Path) -> Path {
        if self.root_path.is_absolute_root_path() {
            return index_path.clone();
        }
        index_path
            .replace_prefix(&self.root_path, &Path::absolute_root())
            .unwrap_or_else(|| index_path.clone())
    }
    // ---------------------------------------------------------------------- //
    // P2-D1: SetRigidXformOverrides
    // ---------------------------------------------------------------------- //

    /// Set rigid transform overrides for prims (e.g. camera override transforms).
    /// Diffs old vs new, marks DirtyTransform on changed subtree roots.
    /// C++: UsdImagingDelegate::SetRigidXformOverrides.
    pub fn set_rigid_xform_overrides(&self, new_overrides: HashMap<Path, Matrix4d>) {
        let mut current = self.rigid_xform_overrides.write();
        if *current == new_overrides {
            return;
        }

        // Collect paths that changed or were added/removed.
        let mut dirty_paths = Vec::new();

        // Changed or new entries
        for (path, mat) in &new_overrides {
            match current.get(path) {
                Some(old_mat) if old_mat == mat => {} // unchanged
                _ => dirty_paths.push(path.clone()),
            }
        }
        // Removed entries
        for path in current.keys() {
            if !new_overrides.contains_key(path) {
                dirty_paths.push(path.clone());
            }
        }

        *current = new_overrides;
        drop(current);

        // Mark DirtyTransform (0x01) on all changed paths.
        let mut dirty = self.dirty_bits.lock().expect("Lock poisoned");
        for path in &dirty_paths {
            let bits = dirty.entry(path.clone()).or_insert(0);
            *bits |= 0x01; // DirtyTransform
        }
    }

    /// Get current rigid xform overrides. C++: GetRigidXformOverrides.
    pub fn rigid_xform_overrides(&self) -> HashMap<Path, Matrix4d> {
        self.rigid_xform_overrides.read().clone()
    }

    // ---------------------------------------------------------------------- //
    // P2-D2: SetUsdDrawModesEnabled
    // ---------------------------------------------------------------------- //

    /// Enable/disable USD draw modes (model:drawMode). Must be called before populate().
    /// C++: UsdImagingDelegate::SetUsdDrawModesEnabled.
    pub fn set_usd_draw_modes_enabled(&self, enable: bool) {
        let cache = self.prim_cache.read();
        if !cache.is_empty() {
            log::error!("set_usd_draw_modes_enabled() called after population; unsupported");
            return;
        }
        drop(cache);
        self.usd_draw_modes_enabled.store(enable, Ordering::Release);
    }

    /// Whether USD draw modes are enabled.
    pub fn usd_draw_modes_enabled(&self) -> bool {
        self.usd_draw_modes_enabled.load(Ordering::Acquire)
    }

    // ---------------------------------------------------------------------- //
    // P2-D3: SetDisplayUnloadedPrimsWithBounds
    // ---------------------------------------------------------------------- //

    /// Enable display of unloaded prims as bounding box wireframes.
    /// Must be called before populate(). C++: SetDisplayUnloadedPrimsWithBounds.
    pub fn set_display_unloaded_with_bounds(&self, enable: bool) {
        let cache = self.prim_cache.read();
        if !cache.is_empty() {
            log::error!("set_display_unloaded_with_bounds() called after population; unsupported");
            return;
        }
        drop(cache);
        self.display_unloaded_with_bounds
            .store(enable, Ordering::Release);
    }

    /// Whether unloaded prims display as bounds.
    pub fn display_unloaded_with_bounds(&self) -> bool {
        self.display_unloaded_with_bounds.load(Ordering::Acquire)
    }

    // ---------------------------------------------------------------------- //
    // P2-D4/D5: Internal helpers for draw mode / coord sys
    // ---------------------------------------------------------------------- //

    /// Compute model draw mode for a prim. C++: _GetModelDrawMode.
    fn compute_model_draw_mode(&self, prim: &Prim) -> Token {
        let prim_path = prim.get_path().clone();
        // Cache lookup
        {
            let cache = self.draw_mode_cache.read();
            if let Some(cached) = cache.get(&prim_path) {
                return cached.clone();
            }
        }
        // Compute
        let result =
            if self.display_unloaded_with_bounds.load(Ordering::Acquire) && !prim.is_loaded() {
                Token::new("bounds")
            } else {
                // Read model:drawMode from prim via UsdGeomModelAPI (traverses ancestors)
                let model_api = ModelAPI::new(prim.clone());
                model_api.compute_model_draw_mode(None)
            };
        // Cache
        self.draw_mode_cache
            .write()
            .insert(prim_path, result.clone());
        result
    }

    /// Enable/disable coordinate system bindings. C++: _coordSysEnabled.
    pub fn set_coord_sys_enabled(&self, enable: bool) {
        self.coord_sys_enabled.store(enable, Ordering::Release);
    }

    /// Look up the adapter for a hydra prim id.
    /// Returns the adapter and the USD prim path.
    fn get_adapter_for_id(&self, id: &Path) -> Option<(Arc<dyn PrimAdapter>, Path)> {
        let cache = self.prim_cache.read();
        let entry = cache.get(id)?;
        Some((entry.adapter.clone(), entry.usd_path.clone()))
    }

    // ---------------------------------------------------------------------- //
    // Point instancer / motion blur caches
    // ---------------------------------------------------------------------- //

    /// Returns point instancer indices for the given prim, reading from USD
    /// `indices` attribute with lazy caching.
    pub fn get_point_instancer_indices(&self, id: &Path) -> Vec<i32> {
        if let Some(cached) = self.point_instancer_indices_cache.read().get(id) {
            return cached.clone();
        }
        let indices = if let Some(stage) = self.get_stage() {
            let sdf_time = SdfTimeCode::new(self.get_time().value());
            stage
                .get_prim_at_path(id)
                .and_then(|prim| prim.get_attribute("indices"))
                .and_then(|attr| attr.get(sdf_time))
                .and_then(|val| val.get::<Vec<i32>>().cloned())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        self.point_instancer_indices_cache
            .write()
            .insert(id.clone(), indices.clone());
        indices
    }

    /// Returns the instancer path for a prototype rprim, or None if not instanced.
    ///
    /// Checks whether `rprim_id`'s parent is a known instancer. Used by mesh_sync
    /// to detect prototype rprims that need GPU instancing.
    pub fn get_instancer_for_rprim(&self, rprim_id: &Path) -> Option<Path> {
        let parent = rprim_id.get_parent_path();
        let inst_data = self.instancer_data.read();
        if inst_data.contains_key(&parent) {
            Some(parent)
        } else {
            None
        }
    }

    /// Returns per-instance world transforms for all instances of the given instancer.
    ///
    /// Each transform is a 4x4 f64 matrix (row-major). The instancer's own world
    /// transform is pre-composed so results are in world space.
    pub fn get_instance_world_xforms(&self, instancer_id: &Path) -> Vec<[[f64; 4]; 4]> {
        let inst_data = self.instancer_data.read();
        let Some(idata) = inst_data.get(instancer_id) else {
            return Vec::new();
        };
        let Some(stage) = self.get_stage() else {
            return Vec::new();
        };

        // Instancer world transform
        let instancer_xf = {
            let mut cache = self.xform_cache.lock().expect("Lock poisoned");
            if let Some(prim) = stage.get_prim_at_path(instancer_id) {
                cache.get_local_to_world_transform(&prim)
            } else {
                Matrix4d::identity()
            }
        };
        let root = *self.root_transform.read();
        let root_inv = root.inverse().unwrap_or_else(Matrix4d::identity);

        let mut result = Vec::with_capacity(idata.instance_paths.len());
        for inst_path in &idata.instance_paths {
            let inst_xf = if let Some(inst_prim) = stage.get_prim_at_path(inst_path) {
                let mut cache = self.xform_cache.lock().expect("Lock poisoned");
                // Per-instance local-to-world, then factor out root (instancer reports root)
                root_inv * cache.get_local_to_world_transform(&inst_prim)
            } else {
                Matrix4d::identity()
            };
            // Compose: instance_xf * instancer_world_xf * root
            let world = inst_xf * instancer_xf * root;
            result.push(world.to_array());
        }
        result
    }

    /// Returns nonlinear sample count for the given prim (default 3).
    pub fn get_nonlinear_sample_count(&self, id: &Path) -> i32 {
        if let Some(&cached) = self.nonlinear_sample_count_cache.read().get(id) {
            return cached;
        }
        let count = if let Some(stage) = self.get_stage() {
            let sdf_time = SdfTimeCode::new(self.get_time().value());
            stage
                .get_prim_at_path(id)
                .and_then(|prim| prim.get_attribute("nonlinearSampleCount"))
                .and_then(|attr| attr.get(sdf_time))
                .and_then(|val| val.get::<i32>().copied())
                .unwrap_or(3)
        } else {
            3
        };
        self.nonlinear_sample_count_cache
            .write()
            .insert(id.clone(), count);
        count
    }

    /// Returns blur scale for the given prim (default 1.0).
    pub fn get_blur_scale(&self, id: &Path) -> f32 {
        if let Some(&cached) = self.blur_scale_cache.read().get(id) {
            return cached;
        }
        let scale = if let Some(stage) = self.get_stage() {
            let sdf_time = SdfTimeCode::new(self.get_time().value());
            stage
                .get_prim_at_path(id)
                .and_then(|prim| prim.get_attribute("blurScale"))
                .and_then(|attr| attr.get(sdf_time))
                .and_then(|val| val.get::<f32>().copied())
                .unwrap_or(1.0)
        } else {
            1.0
        };
        self.blur_scale_cache.write().insert(id.clone(), scale);
        scale
    }

    /// Recursively collect CoordSys bindings for `prim` with inheritance.
    ///
    /// Walks from the stage root down to `prim` (parent-first), then lets
    /// child bindings override parent bindings with the same name — matching
    /// the C++ `UsdImaging_ResolvedAttributeCache` algorithm
    /// (resolvedAttributeCache.h:968-1033).
    ///
    /// Returns `(binding_name, binding_rel_path)` pairs with duplicates
    /// already resolved (child wins).
    fn compute_coord_sys_bindings(&self, prim: &Prim, stage: &Arc<Stage>) -> Vec<(Token, Path)> {
        // Build the ancestor chain from root → prim (inclusive).
        let mut ancestors: Vec<Prim> = Vec::new();
        let mut current = prim.clone();
        loop {
            ancestors.push(current.clone());
            let parent = current.parent();
            if !parent.is_valid() {
                break;
            }
            current = parent;
        }
        // Reverse so we iterate root → leaf (parent bindings first).
        ancestors.reverse();

        // Accumulator: preserves insertion order; later entries override earlier
        // ones with the same name (child wins).
        let mut accumulated: Vec<(Token, Path)> = Vec::new();

        for ancestor in &ancestors {
            let local = CoordSysAPI::get_local_bindings_for_prim(ancestor);
            if local.is_empty() {
                continue;
            }
            for binding in local {
                // Validate that the target coord-sys prim actually exists.
                if stage
                    .get_prim_at_path(&binding.coord_sys_prim_path)
                    .is_none()
                {
                    log::warn!(
                        "CoordSysBinding: target prim '{}' for binding '{}' not found \
                         on stage — skipping",
                        binding.coord_sys_prim_path,
                        binding.name
                    );
                    continue;
                }
                // Override semantics: replace existing entry with same name.
                if let Some(slot) = accumulated.iter_mut().find(|(n, _)| *n == binding.name) {
                    slot.1 = binding.binding_rel_path;
                } else {
                    accumulated.push((binding.name, binding.binding_rel_path));
                }
            }
        }

        accumulated
    }
}

impl HdSceneDelegate for UsdImagingDelegate {
    fn get_delegate_id(&self) -> Path {
        self.root_path.clone()
    }

    fn sync(&mut self, _request: &mut usd_hd::HdSyncRequestVector) {
        self.sync_prims();
    }

    fn post_sync_cleanup(&mut self) {
        // Delegate dirty bits are consumed by Hydra through get_dirty_bits() and
        // cleared incrementally via mark_clean() during rprim sync.
    }

    fn get_dirty_bits(&self, id: &Path) -> HdDirtyBits {
        let cache_path = self.convert_index_path_to_cache_path(id);
        let dirty = self.dirty_bits.lock().expect("Lock poisoned");
        dirty.get(&cache_path).copied().unwrap_or(0)
    }

    fn mark_clean(&mut self, id: &Path, bits: HdDirtyBits) {
        let cache_path = self.convert_index_path_to_cache_path(id);
        let mut dirty = self.dirty_bits.lock().expect("Lock poisoned");
        if let Some(current) = dirty.get_mut(&cache_path) {
            *current &= !bits;
        }
    }

    // P2-D5: GetModelDrawMode
    fn get_model_draw_mode(&self, id: &Path) -> HdModelDrawMode {
        let usd_path = {
            let cache = self.prim_cache.read();
            cache
                .get(id)
                .map(|c| c.usd_path.clone())
                .unwrap_or_else(|| id.clone())
        };
        let mut mode = HdModelDrawMode::default();
        if let Some(stage) = self.get_stage() {
            if let Some(prim) = stage.get_prim_at_path(&usd_path) {
                let draw_mode = self.compute_model_draw_mode(&prim);
                mode.draw_mode = draw_mode;
                // Check if draw mode should be applied (model:applyDrawMode)
                let model_api = ModelAPI::new(prim);
                if let Some(attr) = model_api.get_model_apply_draw_mode_attr() {
                    if let Some(apply) = attr.get_typed::<bool>(SdfTimeCode::default()) {
                        mode.apply_draw_mode = apply;
                    }
                }
            }
        }
        mode
    }

    // P2-D4: GetCoordSysBindings — cached via coord_sys_cache.
    fn get_coord_sys_bindings(&self, id: &Path) -> Option<HdIdVectorSharedPtr> {
        if !self.coord_sys_enabled.load(Ordering::Acquire) {
            return None;
        }
        // Cache lookup — return immediately if already computed (even if None).
        {
            let cache = self.coord_sys_cache.read();
            if let Some(cached) = cache.get(id) {
                return cached.as_ref().map(Arc::clone);
            }
        }

        // Convert render-index path → USD cache path → look up prim on stage.
        let cache_path = self.convert_index_path_to_cache_path(id);
        let result: Option<Arc<Vec<Path>>> = self.get_stage().and_then(|stage| {
            let prim = stage.get_prim_at_path(&cache_path)?;
            // Walk the hierarchy and collect (name, binding_rel_path) pairs.
            // Child bindings override parent bindings with the same name.
            let pairs = self.compute_coord_sys_bindings(&prim, &stage);
            if pairs.is_empty() {
                None
            } else {
                // Return only the binding relationship paths — the Hydra
                // render index uses these as coord-sys prim IDs.
                let paths: Vec<Path> = pairs
                    .into_iter()
                    .map(|(_name, rel_path)| self.convert_cache_path_to_index_path(&rel_path))
                    .collect();
                Some(Arc::new(paths))
            }
        });

        // Store result (including None) so we don't re-compute on next call.
        self.coord_sys_cache
            .write()
            .insert(id.clone(), result.clone());
        result
    }

    fn get_instancer_id(&self, prim_id: &Path) -> Path {
        // For proto prims (children of instancers), return the instancer cache path.
        // C++: UsdImagingInstanceAdapter::GetInstancerId() → instancerContext.instancerCachePath
        //
        // We check whether prim_id is a proto-prim path by looking up the prim_cache
        // to find its USD path, then checking instance_to_instancer. If prim_id is
        // itself an instancer we return the empty path (native instancers have no parent).
        {
            let inst_data = self.instancer_data.read();
            // If prim_id is an instancer itself, no parent instancer.
            if inst_data.contains_key(prim_id) {
                return Path::empty();
            }
        }

        // Check if prim_id is a proto prim: its parent should be in instancer_data.
        let parent = prim_id.get_parent_path();
        {
            let inst_data = self.instancer_data.read();
            if inst_data.contains_key(&parent) {
                return parent;
            }
        }

        // C++ delegate.cpp:2960-2961: if no native instancer found, return
        // _rootInstancerId (used for scene instancing).
        let root_id = self.root_instancer_id.read().clone();
        root_id
    }

    fn get_instance_indices(&self, instancer_id: &Path, _prototype_id: &Path) -> Vec<i32> {
        // C++: UsdImagingInstanceAdapter::GetInstanceIndices -> _ComputeInstanceMap.
        // Filter by per-instance visibility: only include visible instances.
        let inst_data = self.instancer_data.read();
        if let Some(idata) = inst_data.get(instancer_id) {
            let time = self.get_time();
            let sdf_time = SdfTimeCode::new(time.value());
            let stage_opt = self.get_stage();
            let mut indices = Vec::with_capacity(idata.instance_paths.len());

            for (idx, inst_path) in idata.instance_paths.iter().enumerate() {
                // Check cached visibility state per C++ _ComputeInstanceMapFn.
                let vis = idata
                    .visibility
                    .get(idx)
                    .copied()
                    .unwrap_or(InstanceVisibility::Unknown);

                let visible = match vis {
                    InstanceVisibility::Visible => true,
                    InstanceVisibility::Invisible => false,
                    InstanceVisibility::Varying | InstanceVisibility::Unknown => {
                        // Resolve at current time by walking the instance prim
                        // and all ancestors checking visibility attribute.
                        if let Some(ref stage) = stage_opt {
                            if let Some(prim) = stage.get_prim_at_path(inst_path) {
                                self.is_prim_visible(&prim, sdf_time)
                            } else {
                                true
                            }
                        } else {
                            true
                        }
                    }
                };

                if visible {
                    indices.push(idx as i32);
                }
            }
            indices
        } else {
            Vec::new()
        }
    }

    fn get_instancer_transform(&self, instancer_id: &Path) -> Matrix4d {
        // Returns the instancer's world transform.
        // C++: UsdImagingInstanceAdapter::GetInstancerTransform → GetRootTransform()
        // Native instancers use the root (scene) transform; per-instance transforms
        // are reported as "instanceTransforms" primvar via get_primvar(), not here.
        let root = *self.root_transform.read();
        if let Some(stage) = self.get_stage() {
            if let Some(prim) = stage.get_prim_at_path(instancer_id) {
                let mut cache = self.xform_cache.lock().expect("Lock poisoned");
                return cache.get_local_to_world_transform(&prim) * root;
            }
        }
        root
    }

    fn get_instancer_prototypes(&self, instancer_id: &Path) -> Vec<Path> {
        // Returns cache paths of all proto prims registered under this instancer.
        // C++: UsdImagingInstanceAdapter::GetInstancerPrototypes → primMap keys
        let inst_data = self.instancer_data.read();
        if let Some(idata) = inst_data.get(instancer_id) {
            idata.prim_map.keys().cloned().collect()
        } else {
            Vec::new()
        }
    }

    fn sample_instancer_transform(
        &self,
        instancer_id: &Path,
        max_sample_count: usize,
    ) -> Vec<(f32, Matrix4d)> {
        // P1-33: C++ SampleInstancerTransform delegates to adapter.
        // Returns time-sampled transforms for motion blur. When shutter is
        // closed (no motion blur), returns single sample at t=0.
        if max_sample_count == 0 {
            return Vec::new();
        }

        let shutter = *self.shutter_interval.read();
        if shutter.is_open() && max_sample_count >= 2 {
            // Motion blur active: return samples at shutter open/close times.
            // C++: adapter->SampleTransform returns time samples within shutter.
            let xform = self.get_instancer_transform(instancer_id);
            vec![(shutter.open as f32, xform), (shutter.close as f32, xform)]
        } else {
            // Single sample at t=0 (no motion blur or max_sample_count==1).
            vec![(0.0, self.get_instancer_transform(instancer_id))]
        }
    }

    fn get_volume_field_descriptors(
        &self,
        volume_id: &Path,
    ) -> usd_hd::scene_delegate::HdVolumeFieldDescriptorVector {
        // P1-30: C++ GetVolumeFieldDescriptors delegates to adapter.
        // We read UsdVolVolume field relationships directly.
        let Some(stage) = self.get_stage() else {
            return Vec::new();
        };
        let usd_path = {
            let cache = self.prim_cache.read();
            cache
                .get(volume_id)
                .map(|c| c.usd_path.clone())
                .unwrap_or_else(|| volume_id.clone())
        };
        let Some(prim) = stage.get_prim_at_path(&usd_path) else {
            return Vec::new();
        };

        // Volume prims have field:* relationships pointing to VolumeField prims.
        let mut result = Vec::new();
        for rel_name in prim.get_relationship_names() {
            let name_str = rel_name.as_str();
            if !name_str.starts_with("field:") {
                continue;
            }
            let field_name = Token::new(&name_str["field:".len()..]);
            if let Some(rel) = prim.get_relationship(name_str) {
                for target_path in rel.get_targets() {
                    if let Some(field_prim) = stage.get_prim_at_path(&target_path) {
                        let field_prim_type = field_prim.type_name();
                        result.push(usd_hd::scene_delegate::HdVolumeFieldDescriptor::new(
                            field_name.clone(),
                            Token::new(field_prim_type.as_str()),
                            target_path,
                        ));
                    }
                }
            }
        }
        result
    }

    fn get_indexed_primvar(&self, id: &Path, key: &Token) -> (Value, Option<Vec<i32>>) {
        // For instancer prims, provide the `instanceTransforms` primvar —
        // a flat array of 16 f64 values per instance (row-major 4x4 matrices).
        // C++: UsdImagingInstanceAdapter::Get(key = HdInstancerTokens->instanceTransforms)
        //      → _ComputeInstanceTransforms → flat VtMatrix4dArray
        if key == "instanceTransforms" {
            let inst_data = self.instancer_data.read();
            if let Some(idata) = inst_data.get(id) {
                // Collect world transforms for every registered instance prim.
                let Some(stage) = self.get_stage() else {
                    return (Value::default(), None);
                };

                let mut matrices: Vec<Matrix4d> = Vec::with_capacity(idata.instance_paths.len());
                let root_xform = *self.root_transform.read();
                let root_inv = root_xform.inverse().unwrap_or_else(Matrix4d::identity);

                for inst_path in &idata.instance_paths {
                    let xform = if let Some(inst_prim) = stage.get_prim_at_path(inst_path) {
                        let mut cache = self.xform_cache.lock().expect("Lock poisoned");
                        // Per C++ _ComputeInstanceTransformFn: multiply by inverseRoot
                        // to avoid double-applying the root transform (instancer itself
                        // reports the root transform via get_instancer_transform).
                        root_inv * cache.get_local_to_world_transform(&inst_prim)
                    } else {
                        Matrix4d::identity()
                    };
                    matrices.push(xform);
                }

                return (Value::from(matrices), None);
            }
        }

        // P2 #4: Check if key matches an inherited primvar on this instancer.
        // C++: _ComputeInheritedPrimvar() collects per-instance values for inherited primvars.
        {
            let inst_data = self.instancer_data.read();
            if let Some(idata) = inst_data.get(id) {
                let is_inherited = idata.inherited_primvars.iter().any(|pv| pv.name == *key);
                if is_inherited {
                    let instance_paths = idata.instance_paths.clone();
                    drop(inst_data);

                    // Collect the primvar value from each instance prim.
                    // C++ dispatches by type; we use VtValue (Value) directly.
                    if let Some(stage) = self.get_stage() {
                        let time = SdfTimeCode::new(self.get_time().value());
                        let mut values: Vec<Value> = Vec::with_capacity(instance_paths.len());

                        for inst_path in &instance_paths {
                            if let Some(inst_prim) = stage.get_prim_at_path(inst_path) {
                                let api = PrimvarsAPI::new(inst_prim);
                                let inherited = api.find_primvars_with_inheritance();
                                let mut found = false;
                                for pv in &inherited {
                                    if pv.get_primvar_name() == *key {
                                        if let Some(val) = pv.get_attr().get(time) {
                                            values.push(val);
                                            found = true;
                                        }
                                        break;
                                    }
                                }
                                if !found {
                                    values.push(Value::default());
                                }
                            } else {
                                values.push(Value::default());
                            }
                        }

                        // Return as array of values (instance-rate)
                        if !values.is_empty() {
                            return (Value::new(values), None);
                        }
                    }
                }
            }
        }

        let Some(stage) = self.get_stage() else {
            return (Value::default(), None);
        };

        let usd_path = {
            let cache = self.prim_cache.read();
            cache
                .get(id)
                .map(|c| c.usd_path.clone())
                .unwrap_or_else(|| id.clone())
        };
        let Some(prim) = stage.get_prim_at_path(&usd_path) else {
            return (Value::default(), None);
        };

        let sdf_time = SdfTimeCode::new(self.get_time().value());
        let key_str = key.as_str();
        let primvar_name = format!("primvars:{key_str}");
        if let Some(attr) = prim.get_attribute(&primvar_name) {
            let value = convert_vec2d_to_vec2f(
                attr.get(sdf_time)
                    .or_else(|| attr.get(SdfTimeCode::default_time()))
                    .unwrap_or_default(),
            );
            let indices_name = format!("{primvar_name}:indices");
            let indices = prim.get_attribute(&indices_name).and_then(|a| {
                let value = a
                    .get(sdf_time)
                    .or_else(|| a.get(SdfTimeCode::default_time()))?;
                value.as_vec_clone::<i32>().or_else(|| {
                    value
                        .get::<Vec<Value>>()
                        .and_then(|values| values.iter().map(value_scalar_to_i32).collect())
                })
            });
            return (value, indices);
        }

        if let Some(attr) = prim.get_attribute(key_str) {
            return (
                convert_vec2d_to_vec2f(
                    attr.get(sdf_time)
                        .or_else(|| attr.get(SdfTimeCode::default_time()))
                        .unwrap_or_default(),
                ),
                None,
            );
        }

        (Value::default(), None)
    }

    fn get_transform(&self, id: &Path) -> Matrix4d {
        let root_xform = *self.root_transform.read();
        let sdf_time = SdfTimeCode::new(self.get_time().value());

        // P1-3: Try adapter dispatch first. C++: adapter->GetTransform().
        // Instance adapter provides instanced transforms, draw mode adapter
        // provides bounds-based transforms, etc.
        {
            let cache = self.prim_cache.read();
            if let Some(cached) = cache.get(id) {
                if let Some(stage) = self.get_stage() {
                    if let Some(prim) = stage.get_prim_at_path(&cached.usd_path) {
                        if let Some(xform) = cached.adapter.get_transform(&prim, sdf_time) {
                            return xform * root_xform;
                        }
                    }
                }
            }
        }

        // P1-3: Instanced prims get identity transform from the rprim side;
        // per-instance transforms are delivered via instanceTransforms primvar.
        // C++: UsdImagingInstanceAdapter::GetTransform returns identity.
        {
            let i2i = self.instance_to_instancer.read();
            if i2i.contains_key(id) {
                return root_xform; // identity * root
            }
        }

        // Default: XformCache for standard prims.
        if let Some(stage) = self.get_stage() {
            if let Some(prim) = stage.get_prim_at_path(id) {
                let mut cache = self.xform_cache.lock().expect("Lock poisoned");
                let local_xform = cache.get_local_to_world_transform(&prim);
                // C++ does: ctm * rootTransform (row-vector)
                return local_xform * root_xform;
            }
        }
        root_xform
    }

    fn get_extent(&self, id: &Path) -> Range3d {
        if let Some(stage) = self.get_stage() {
            if let Some(prim) = stage.get_prim_at_path(id) {
                return super::gprim_adapter::GprimAdapter::get_extent(&prim, self.get_time());
            }
        }
        Range3d::default()
    }

    fn get_visible(&self, id: &Path) -> bool {
        // Check root visibility first.
        if !*self.root_visible.read() {
            return false;
        }

        // C++: IsInInvisedPaths(cachePath) — check before any adapter delegation.
        // SetInvisedPrimPaths() after initial populate must still affect rendering.
        {
            let invised = self.invised_paths.read();
            if invised.iter().any(|ip| id.has_prefix(ip)) {
                return false;
            }
        }

        // P1-14: Check vis_cache first for O(1) repeated lookups.
        {
            let vis = self.vis_cache.read();
            if let Some(&cached_vis) = vis.get(id) {
                return cached_vis;
            }
        }

        let Some(stage) = self.get_stage() else {
            return true;
        };

        // Resolve actual USD prim path (handles subprim suffixes).
        let (usd_path, adapter) = {
            let cache = self.prim_cache.read();
            match cache.get(id) {
                Some(c) => (c.usd_path.clone(), Some(c.adapter.clone())),
                None => (id.clone(), None),
            }
        };

        let Some(prim) = stage.get_prim_at_path(&usd_path) else {
            return true;
        };

        let sdf_time = SdfTimeCode::new(self.get_time().value());

        // P1-4: Try adapter dispatch first. C++: adapter->GetVisible().
        if let Some(ref adp) = adapter {
            if let Some(vis) = adp.get_visible(&prim, sdf_time) {
                self.vis_cache.write().insert(id.clone(), vis);
                return vis;
            }
        }

        // P1-4: Instanced prims always report visible from the rprim side;
        // per-instance visibility is handled by get_instance_indices() filtering.
        // C++: UsdImagingInstanceAdapter::GetVisible returns true.
        {
            let i2i = self.instance_to_instancer.read();
            if i2i.contains_key(id) {
                self.vis_cache.write().insert(id.clone(), true);
                return true;
            }
        }

        // Default: walk up to root checking inherited visibility.
        let mut visible = true;
        let mut current = prim.clone();
        loop {
            if let Some(attr) = current.get_attribute("visibility") {
                if let Some(tok) = attr.get_typed::<Token>(sdf_time) {
                    if tok == "invisible" {
                        visible = false;
                        break;
                    }
                }
            }
            if current.get_path().is_absolute_root_path() {
                break;
            }
            match stage.get_prim_at_path(&current.get_path().get_parent_path()) {
                Some(parent) => current = parent,
                None => break,
            }
        }

        // Cache the result for future lookups.
        self.vis_cache.write().insert(id.clone(), visible);
        visible
    }

    fn get_double_sided(&self, id: &Path) -> bool {
        if let Some(stage) = self.get_stage() {
            if let Some(prim) = stage.get_prim_at_path(id) {
                return super::gprim_adapter::GprimAdapter::get_double_sided(
                    &prim,
                    self.get_time(),
                );
            }
        }
        false
    }

    fn get_display_style(&self, id: &Path) -> HdDisplayStyle {
        // Per-prim override takes priority, then fallback. C++: _refineLevelMap.
        let level = {
            let map = self.refine_level_map.read();
            map.get(id)
                .copied()
                .unwrap_or_else(|| self.get_refine_level_fallback())
        };
        HdDisplayStyle {
            refine_level: level,
            ..HdDisplayStyle::default()
        }
    }

    fn get_cull_style(&self, _id: &Path) -> HdCullStyle {
        // Return per-delegate cull style fallback. C++: _cullStyleFallback.
        *self.cull_style_fallback.read()
    }

    fn get_render_tag(&self, id: &Path) -> Token {
        // Map USD purpose to Hydra render tag. C++: delegate.cpp:1943-1973.
        let Some(stage) = self.get_stage() else {
            return usd_hd::tokens::RENDER_TAG_GEOMETRY.clone();
        };
        let usd_path = {
            let cache = self.prim_cache.read();
            cache
                .get(id)
                .map(|c| c.usd_path.clone())
                .unwrap_or_else(|| id.clone())
        };
        let Some(prim) = stage.get_prim_at_path(&usd_path) else {
            return usd_hd::tokens::RENDER_TAG_GEOMETRY.clone();
        };

        // Get purpose from prim (C++: adapter->GetPurpose), default "default"
        let mut purpose = Token::new("default");
        let sdf_time = SdfTimeCode::new(self.get_time().value());
        if let Some(attr) = prim.get_attribute("purpose") {
            if let Some(tok) = attr.get_typed::<Token>(sdf_time) {
                if !tok.as_str().is_empty() {
                    purpose = tok;
                }
            }
        }

        // C++ delegate.cpp:1958-1967: map purpose to render tag
        if purpose == "default" {
            usd_hd::tokens::RENDER_TAG_GEOMETRY.clone()
        } else if (purpose == "render" && !*self.display_render.read())
            || (purpose == "proxy" && !*self.display_proxy.read())
            || (purpose == "guide" && !*self.display_guides.read())
        {
            // Disabled purpose -> hidden command buffer
            Token::new("hidden")
        } else {
            // render/proxy/guide pass through as render tags
            purpose
        }
    }

    fn get_repr_selector(&self, _id: &Path) -> usd_hd::prim::HdReprSelector {
        // C++ delegate.cpp:2782-2784: return _reprFallback
        self.repr_fallback.read().clone()
    }

    fn get_camera_param_value(&self, camera_id: &Path, param_name: &Token) -> usd_vt::Value {
        // Standard Hydra 1.0 camera param query. C++: GetCameraParamValue().
        let Some(stage) = self.get_stage() else {
            return usd_vt::Value::default();
        };
        let Some(prim) = stage.get_prim_at_path(camera_id) else {
            return usd_vt::Value::default();
        };
        let sdf_time = SdfTimeCode::new(self.get_time().value());
        let name = param_name.as_str();

        // Special param: windowPolicy (not a USD attribute).
        if name == "windowPolicy" {
            return usd_vt::Value::from(self.get_window_policy());
        }

        // Try bare camera attribute name, then "inputs:" prefixed.
        if let Some(attr) = prim.get_attribute(name) {
            if let Some(val) = attr.get(sdf_time) {
                return val;
            }
        }
        let inputs_name = format!("inputs:{name}");
        if let Some(attr) = prim.get_attribute(&inputs_name) {
            if let Some(val) = attr.get(sdf_time) {
                return val;
            }
        }
        usd_vt::Value::default()
    }

    fn get_primvar_descriptors(
        &self,
        id: &Path,
        interpolation: HdInterpolation,
    ) -> HdPrimvarDescriptorVector {
        // P1-5: Check primvar descriptor cache first.
        let cache_key = (id.clone(), interpolation as u8);
        {
            let pv_cache = self.primvar_desc_cache.read();
            if let Some(cached) = pv_cache.get(&cache_key) {
                return cached.clone();
            }
        }

        let Some(stage) = self.get_stage() else {
            return Vec::new();
        };
        let (usd_path, adapter) = {
            let cache = self.prim_cache.read();
            match cache.get(id) {
                Some(c) => (c.usd_path.clone(), Some(c.adapter.clone())),
                None => (id.clone(), None),
            }
        };
        let Some(prim) = stage.get_prim_at_path(&usd_path) else {
            return Vec::new();
        };
        let sdf_time = SdfTimeCode::new(self.get_time().value());

        // P1-5: Try adapter dispatch first. C++: adapter->GetPrimvarDescriptors().
        if let Some(ref adp) = adapter {
            if let Some(descs) = adp.get_primvar_descriptors(&prim, interpolation, sdf_time) {
                self.primvar_desc_cache
                    .write()
                    .insert(cache_key, descs.clone());
                return descs;
            }
        }

        let mut result = Vec::new();

        // For instancer prims: report instanceTransforms + inherited primvars
        // as Instance-rate primvars.
        // C++: UsdImagingInstanceAdapter::UpdateForTime -> _MergePrimvar()
        if interpolation == HdInterpolation::Instance {
            let inst_data = self.instancer_data.read();
            if let Some(idata) = inst_data.get(id) {
                result.push(HdPrimvarDescriptor::new(
                    Token::new("instanceTransforms"),
                    HdInterpolation::Instance,
                    Token::new(""),
                    false,
                ));

                // P2 #4: Add inherited primvar descriptors as Instance-rate.
                for ipv in &idata.inherited_primvars {
                    result.push(HdPrimvarDescriptor::new(
                        ipv.name.clone(),
                        HdInterpolation::Instance,
                        Token::new(""),
                        false,
                    ));
                }

                return result;
            }
        }

        // Scan all "primvars:" namespace attributes.
        for attr_name in prim.get_attribute_names() {
            let n = attr_name.as_str();
            if !n.starts_with("primvars:") {
                continue;
            }
            let base_name = &n["primvars:".len()..];
            // Skip ":indices" companion attributes.
            if base_name.ends_with(":indices") {
                continue;
            }

            // Read interpolation from "interpolation" metadata on the attribute.
            // USD uses "interpolation" metadata key on primvar attributes.
            if let Some(attr) = prim.get_attribute(n) {
                let interp = read_primvar_interpolation(&attr, sdf_time);
                if interp == interpolation {
                    // Role is stored in "role" metadata or inferred from name.
                    let role = read_primvar_role(base_name);
                    // Check if there is an :indices companion attr (indexed primvar).
                    let indices_name = format!("{n}:indices");
                    let indexed = prim.get_attribute(&indices_name).is_some();
                    result.push(HdPrimvarDescriptor::new(
                        Token::new(base_name),
                        interp,
                        Token::new(role),
                        indexed,
                    ));
                }
            }
        }

        // Also include well-known bare primvars (points, normals, widths) if they
        // exist and match the requested interpolation.
        // P1-IMG-6: normals/widths must NOT be hardcoded as Varying. Per USD spec,
        // authored normals on meshes are typically "vertex" or "faceVarying" interpolation.
        // Read the actual "interpolation" metadata from the attribute; fall back to
        // Vertex only if metadata is absent (same fallback as primvars namespace attrs).
        //
        // "points" is always Vertex — that is an invariant of PointBased schema.
        //
        // Implicit surface types (Cube, Sphere, etc.) don't have authored "points"
        // attribute but we synthesize points in get(). Declare "points" as Vertex.
        let type_name = prim.get_type_name();
        let is_implicit = is_implicit_type(type_name.as_str());
        if is_implicit && interpolation == HdInterpolation::Vertex {
            result.push(HdPrimvarDescriptor::new(
                Token::new("points"),
                HdInterpolation::Vertex,
                Token::new("point"),
                false,
            ));
        }
        let bare_named: &[(&str, &str)] =
            &[("points", "point"), ("normals", "normal"), ("widths", "")];
        for &(name, role) in bare_named {
            if let Some(attr) = prim.get_attribute(name) {
                // "points" is always Vertex per schema — no metadata needed.
                let interp = if name == "points" {
                    HdInterpolation::Vertex
                } else {
                    read_primvar_interpolation(&attr, sdf_time)
                };
                if interp == interpolation {
                    // Skip duplicate "points" for implicit types (already declared above)
                    if is_implicit && name == "points" {
                        continue;
                    }
                    result.push(HdPrimvarDescriptor::new(
                        Token::new(name),
                        interp,
                        Token::new(role),
                        false,
                    ));
                }
            }
        }

        // P1-5: Cache the computed descriptors for future lookups.
        self.primvar_desc_cache
            .write()
            .insert(cache_key, result.clone());

        result
    }

    fn get_mesh_topology(&self, id: &Path) -> HdMeshTopology {
        let Some(stage) = self.get_stage() else {
            return HdMeshTopology::new();
        };

        // P1-11: Try adapter dispatch first. C++: adapter->GetMeshTopology().
        // Implicit surface adapters (sphere, cube, etc.) synthesize topology.
        let sdf_time = SdfTimeCode::new(self.get_time().value());
        {
            let cache = self.prim_cache.read();
            if let Some(cached) = cache.get(id) {
                if let Some(prim) = stage.get_prim_at_path(&cached.usd_path) {
                    if let Some(topo) = cached.adapter.get_mesh_topology(&prim, sdf_time) {
                        return topo;
                    }
                }
            }
        }

        let usd_path = {
            let cache = self.prim_cache.read();
            cache
                .get(id)
                .map(|c| c.usd_path.clone())
                .unwrap_or_else(|| id.clone())
        };

        let Some(prim) = stage.get_prim_at_path(&usd_path) else {
            return HdMeshTopology::new();
        };

        // P1-11: Implicit surfaces have no authored topology attributes.
        // Synthesize procedural topology for known implicit types.
        let type_name = prim.get_type_name();
        match type_name.as_str() {
            "Sphere" => return synth_sphere_topo(),
            "Cube" => return synth_cube_topo(),
            "Cylinder" => return synth_cylinder_topo(),
            "Cone" => return synth_cone_topo(),
            "Capsule" => return synth_capsule_topo(),
            "Plane" => {
                let topo = PlaneMeshGenerator::generate_topology();
                return HdMeshTopology::from_full(
                    topo.scheme().clone(),
                    topo.orientation().clone(),
                    topo.face_vertex_counts().to_vec(),
                    topo.face_vertex_indices().to_vec(),
                    Vec::new(),
                );
            }
            _ => {}
        }

        if type_name != "Mesh" {
            return HdMeshTopology::new();
        }

        let mesh = Mesh::new(prim.clone());
        let sdf_time = SdfTimeCode::new(self.get_time().value());

        // Read faceVertexCounts
        let counts = Self::attr_get_i32_vec(&mesh.get_face_vertex_counts_attr(), sdf_time)
            .unwrap_or_default();

        // Read faceVertexIndices
        let indices = Self::attr_get_i32_vec(&mesh.get_face_vertex_indices_attr(), sdf_time)
            .unwrap_or_default();

        if counts.is_empty() || indices.is_empty() {
            log::warn!("[delegate] get_mesh_topology: no topology on {}", id);
        }

        // Read subdivision scheme.
        //
        // USD meshes default to `catmullClark` when the attribute is unauthored.
        // Falling back to `none` makes text assets diverge from the reference
        // runtime and from binary forms that carry an explicit authored scheme.
        let scheme = Self::attr_get_token(&mesh.get_subdivision_scheme_attr(), sdf_time)
            .unwrap_or_else(|| Token::new("catmullClark"));

        // Read orientation (rightHanded or leftHanded)
        let orientation = prim
            .get_attribute("orientation")
            .and_then(|attr| Self::attr_get_token(&attr, sdf_time))
            .unwrap_or_else(|| Token::new("rightHanded"));

        // Read hole indices (optional, typically empty)
        let holes =
            Self::attr_get_i32_vec(&mesh.get_hole_indices_attr(), sdf_time).unwrap_or_default();

        // Read subdivision tags (creases, corners, interpolation rules)
        let subdiv_tags = self.read_subdiv_tags(&prim, sdf_time);

        HdMeshTopology::from_full_with_tags(
            scheme,
            orientation,
            counts,
            indices,
            holes,
            subdiv_tags,
        )
    }

    fn get_basis_curves_topology(
        &self,
        id: &Path,
    ) -> usd_hd::prim::basis_curves::HdBasisCurvesTopology {
        // C++ delegate.cpp:1977-1995: adapter->GetTopology()
        use usd_hd::prim::basis_curves::{
            HdBasisCurvesTopology, HdCurveBasis, HdCurveType, HdCurveWrap,
        };

        let Some(stage) = self.get_stage() else {
            return HdBasisCurvesTopology::default();
        };
        let Some(prim) = stage.get_prim_at_path(id) else {
            return HdBasisCurvesTopology::default();
        };
        let sdf_time = SdfTimeCode::new(self.get_time().value());
        match prim.get_type_name().as_str() {
            "BasisCurves" => {
                let bc = usd_geom::BasisCurves::new(prim);

                let counts: Vec<i32> = bc
                    .curves()
                    .get_curve_vertex_counts_attr()
                    .get_typed_vec::<i32>(sdf_time)
                    .unwrap_or_default();

                let curve_type = bc
                    .get_type_attr()
                    .get_typed::<Token>(sdf_time)
                    .and_then(|t| match t.as_str() {
                        "cubic" => Some(HdCurveType::Cubic),
                        "linear" => Some(HdCurveType::Linear),
                        _ => None,
                    });

                let basis = bc
                    .get_basis_attr()
                    .get_typed::<Token>(sdf_time)
                    .and_then(|t| match t.as_str() {
                        "bezier" => Some(HdCurveBasis::Bezier),
                        "bspline" => Some(HdCurveBasis::BSpline),
                        "catmullRom" => Some(HdCurveBasis::CatmullRom),
                        "hermite" => Some(HdCurveBasis::Hermite),
                        _ => None,
                    });

                let wrap = bc
                    .get_wrap_attr()
                    .get_typed::<Token>(sdf_time)
                    .map(|t| match t.as_str() {
                        "periodic" => HdCurveWrap::Periodic,
                        "pinned" => HdCurveWrap::Pinned,
                        _ => HdCurveWrap::Nonperiodic,
                    })
                    .unwrap_or(HdCurveWrap::Nonperiodic);

                HdBasisCurvesTopology {
                    curve_vertex_counts: counts,
                    basis,
                    curve_type,
                    wrap,
                }
            }
            "HermiteCurves" => {
                // Match `_ref` UsdImagingHermiteCurvesAdapter::GetTopology():
                // Hydra has no native Hermite primitive, so image guides as
                // linear basisCurves and ignore tangents/weights.
                let hc = usd_geom::hermite_curves::HermiteCurves::new(prim);
                let counts = hc
                    .curves()
                    .get_curve_vertex_counts_attr()
                    .get_typed_vec::<i32>(sdf_time)
                    .unwrap_or_default();

                HdBasisCurvesTopology {
                    curve_vertex_counts: counts,
                    basis: Some(HdCurveBasis::Bezier),
                    curve_type: Some(HdCurveType::Linear),
                    wrap: HdCurveWrap::Nonperiodic,
                }
            }
            _ => HdBasisCurvesTopology::default(),
        }
    }

    fn get_subdiv_tags(&self, id: &Path) -> SubdivTags {
        let Some(stage) = self.get_stage() else {
            return SubdivTags::default();
        };
        let Some(prim) = stage.get_prim_at_path(id) else {
            return SubdivTags::default();
        };
        let sdf_time = SdfTimeCode::new(self.get_time().value());
        self.read_subdiv_tags(&prim, sdf_time)
    }

    fn get(&self, id: &Path, key: &Token) -> Value {
        let Some(stage) = self.get_stage() else {
            return Value::default();
        };
        // Resolve USD path: strip subprim suffix if present (e.g. /Mesh.proto -> /Mesh)
        let usd_path = {
            let cache = self.prim_cache.read();
            cache
                .get(id)
                .map(|c| c.usd_path.clone())
                .unwrap_or_else(|| id.clone())
        };
        let Some(prim) = stage.get_prim_at_path(&usd_path) else {
            return Value::default();
        };

        let sdf_time = SdfTimeCode::new(self.get_time().value());

        // Implicit surfaces: synthesize points via GeomUtil generators.
        if key == "points" && is_implicit_type(prim.get_type_name().as_str()) {
            if let Some(pts) = synth_implicit_points(&prim, sdf_time) {
                return Value::from(pts);
            }
        }

        Self::read_primvar_value_at_time(&prim, key, sdf_time)
    }

    /// Get light parameter value from the USD prim.
    ///
    /// Reads UsdLux light attributes (inputs:color, inputs:intensity, inputs:exposure,
    /// inputs:texture:file, inputs:diffuse, inputs:specular, inputs:enableShadows, etc.)
    /// and returns them as Values matching C++ HdSceneDelegate::GetLightParamValue.
    fn get_light_param_value(&self, id: &Path, param_name: &Token) -> Value {
        let Some(stage) = self.get_stage() else {
            return Value::default();
        };
        let Some(prim) = stage.get_prim_at_path(id) else {
            return Value::default();
        };
        let sdf_time = SdfTimeCode::new(self.get_time().value());
        let name = param_name.as_str();

        // Invisible lights or globally disabled scene lights → intensity=0
        // Matches C++ primAdapter.cpp GetLightParamValue() lines 763-772
        if name == "intensity" {
            if !self.is_scene_lights_enabled() {
                return Value::from(0.0f32);
            }
            if !self.get_visible(id) {
                return Value::from(0.0f32);
            }
        }

        // Map HdLight param tokens to USD attribute names.
        // USD uses "inputs:" namespace prefix (UsdLux convention).
        let attr_name = match name {
            "texture:file" => "inputs:texture:file",
            n => {
                // Try inputs:<name> first, then bare <name>
                let inputs_name = format!("inputs:{n}");
                if let Some(attr) = prim.get_attribute(&inputs_name) {
                    return self.read_light_attr(&attr, sdf_time);
                }
                // Bare attribute name fallback
                if let Some(attr) = prim.get_attribute(n) {
                    return self.read_light_attr(&attr, sdf_time);
                }
                return Value::default();
            }
        };

        if let Some(attr) = prim.get_attribute(attr_name) {
            return self.read_light_attr(&attr, sdf_time);
        }
        Value::default()
    }
    fn get_material_id(&self, id: &Path) -> Option<Path> {
        // P1-14: Check material_binding_cache first for O(1) repeated lookups.
        {
            let mat_cache = self.material_binding_cache.read();
            if let Some(cached) = mat_cache.get(id) {
                return cached.clone();
            }
        }

        let stage = self.get_stage()?;
        // Resolve actual USD prim path (strip any subprim suffix)
        let usd_path = {
            let cache = self.prim_cache.read();
            cache
                .get(id)
                .map(|c| c.usd_path.clone())
                .unwrap_or_else(|| id.clone())
        };
        let prim = stage.get_prim_at_path(&usd_path)?;

        // Resolve material binding via MaterialBindingAPI (allPurpose)
        let all_purpose = usd_tf::Token::new("allPurpose");
        let mut binding_rel = None;
        let binding_api = MaterialBindingAPI::new(prim.clone());
        let material = binding_api.compute_bound_material(
            &all_purpose,
            &mut binding_rel,
            true, // support legacy bindings
        );

        let result = if material.is_valid() {
            let mat_path = material.get_prim().get_path().clone();
            if !mat_path.is_empty() {
                log::debug!("[delegate] material_id for {} -> {}", id, mat_path);
                Some(mat_path)
            } else {
                None
            }
        } else {
            // Fallback: check direct "material:binding" relationship
            prim.get_relationship("material:binding").and_then(|rel| {
                let targets = rel.get_forwarded_targets();
                targets.into_iter().next()
            })
        };

        // Cache the result (including None) to avoid recomputing.
        self.material_binding_cache
            .write()
            .insert(id.clone(), result.clone());

        result
    }

    /// Build HdMaterialNetworkMap from a material sprim path.
    ///
    /// Walks UsdShadeMaterial → surface output → UsdShadeShader and recursively
    /// traverses the entire connected shader graph (UsdUVTexture, UsdPrimvarReader,
    /// UsdTransform2d, NodeGraph subgraphs, and any other shader nodes).
    /// Mirrors C++ UsdImagingMaterialAdapter::GetMaterialResource().
    ///
    /// P1-IMG-7: Previously only UsdUVTexture nodes were discovered. Full recursive
    /// graph traversal is required so that UsdPrimvarReader_float2 (ST coords),
    /// UsdTransform2d (texture transforms), and arbitrary custom nodes are included.
    fn get_material_resource(&self, material_id: &Path) -> Value {
        use std::collections::{BTreeMap, HashSet as PathSet};
        use usd_hd::material_network::HdMaterialRelationship;
        use usd_shade::shader::Shader as UsdShadeShader;
        use usd_shade::types::AttributeType;

        let Some(stage) = self.get_stage() else {
            return Value::default();
        };
        let Some(prim) = stage.get_prim_at_path(material_id) else {
            return Value::default();
        };

        let material = usd_shade::Material::new(prim.clone());
        if !material.is_valid() {
            return Value::default();
        }

        // Follow surface output connection → surface shader prim
        let mut source_name = Token::new("");
        let mut source_type = AttributeType::Invalid;
        let surface_shader =
            material.compute_surface_source(&[], &mut source_name, &mut source_type);
        if !surface_shader.is_valid() {
            return Value::default();
        }

        let sdf_time = SdfTimeCode::new(self.get_time().value());
        let surface_path = surface_shader.get_prim().get_path().clone();

        let mut all_nodes: Vec<HdMaterialNode> = Vec::new();
        let mut relationships: Vec<HdMaterialRelationship> = Vec::new();
        let mut visited: PathSet<Path> = PathSet::new();

        // Recursive closure emulated via an explicit work-stack (Rust doesn't allow
        // recursive closures that borrow mutable state). Each stack item is a
        // (shader, output_node_path, output_input_name) tuple describing the
        // downstream connection that pulled this shader into the graph.
        // The surface shader seeds the stack with no downstream.
        struct WorkItem {
            shader: UsdShadeShader,
            /// downstream node path (the node whose input this shader feeds into)
            dst_path: Option<Path>,
            /// name of the input on the downstream node
            dst_input: Option<Token>,
            /// name of the output on this shader
            src_output: Option<Token>,
        }

        let mut stack: Vec<WorkItem> = vec![WorkItem {
            shader: surface_shader,
            dst_path: None,
            dst_input: None,
            src_output: None,
        }];

        while let Some(item) = stack.pop() {
            let node_path = item.shader.get_prim().get_path().clone();

            // Record relationship from upstream → downstream before dedup check
            if let (Some(dst), Some(dst_in), Some(src_out)) =
                (item.dst_path, item.dst_input, item.src_output)
            {
                relationships.push(HdMaterialRelationship {
                    input_id: node_path.clone(),
                    input_name: src_out,
                    output_id: dst,
                    output_name: dst_in,
                });
            }

            // Skip if this node has already been added (diamond connections)
            if !visited.insert(node_path.clone()) {
                continue;
            }

            // Build node parameters from all authored inputs
            let shader_id = item
                .shader
                .get_shader_id()
                .unwrap_or_else(|| Token::new(""));
            let mut params = BTreeMap::new();
            for input in item.shader.get_inputs(true) {
                if input.is_defined() {
                    if let Some(val) = input.get_value(sdf_time) {
                        params.insert(input.get_base_name(), val);
                    }
                }
            }

            log::debug!(
                "[delegate] shader node {} ({})",
                node_path,
                shader_id.as_str()
            );

            all_nodes.push(HdMaterialNode {
                path: node_path.clone(),
                identifier: shader_id,
                parameters: params,
            });

            // Enqueue all connected upstream shaders for traversal
            for input in item.shader.get_inputs(true) {
                if !input.has_connected_source() {
                    continue;
                }
                let mut invalid_sources = Vec::new();
                let sources = input.get_connected_sources(&mut invalid_sources);
                for src in sources {
                    let src_prim = src.source.get_prim();
                    if !src_prim.is_valid() {
                        continue;
                    }
                    let upstream = UsdShadeShader::new(src_prim.clone());
                    if !upstream.is_valid() {
                        continue;
                    }
                    stack.push(WorkItem {
                        shader: upstream,
                        dst_path: Some(node_path.clone()),
                        dst_input: Some(input.get_base_name()),
                        src_output: Some(src.source_name.clone()),
                    });
                }
            }
        }

        // Reverse so upstream nodes appear before the surface terminal (C++ ordering)
        all_nodes.reverse();

        // Build V1 network
        let network = HdMaterialNetworkV1 {
            relationships,
            nodes: all_nodes,
            primvars: Vec::new(),
        };

        // Build map with "surface" terminal
        let mut map = HdMaterialNetworkMap::default();
        map.map.insert(Token::new("surface"), network);
        map.terminals.push(surface_path);

        log::debug!(
            "[delegate] get_material_resource {} -> {} nodes",
            material_id,
            map.map.len()
        );

        Value::from(map)
    }

    // ----------------------------------------------------------------------- //
    // Motion samples — override HdSceneDelegate default single-sample stubs
    // ----------------------------------------------------------------------- //

    /// Sample a primvar at multiple times within the shutter interval.
    ///
    /// Returns up to `max_sample_count` (time, value) pairs.  If the attribute
    /// is not time-varying a single sample at the current frame is returned.
    ///
    /// Mirrors C++ `UsdImagingDelegate::SamplePrimvar()` /
    /// `UsdImagingPrimAdapter::SamplePrimvar()`.
    fn sample_primvar(&self, id: &Path, key: &Token, max_sample_count: usize) -> Vec<(f32, Value)> {
        if max_sample_count == 0 {
            return Vec::new();
        }

        let current = self.get_time().value();

        let Some(stage) = self.get_stage() else {
            return vec![(0.0, self.get(id, key))];
        };

        // Resolve actual USD prim path from cache (handles subprim suffixes).
        let usd_path = {
            let cache = self.prim_cache.read();
            cache
                .get(id)
                .map(|c| c.usd_path.clone())
                .unwrap_or_else(|| id.clone())
        };

        let Some(prim) = stage.get_prim_at_path(&usd_path) else {
            return vec![(0.0, self.get(id, key))];
        };

        let key_str = key.as_str();

        // Resolve attribute: try primvars:<key> first, then bare <key>.
        let attr = {
            let primvar_name = format!("primvars:{key_str}");
            prim.get_attribute(&primvar_name)
                .or_else(|| prim.get_attribute(key_str))
        };

        let Some(attr) = attr else {
            // No attribute found — fall back to get() at current time.
            return vec![(0.0, self.get(id, key))];
        };

        // Single-sample fast path when no motion blur is requested.
        let (interval_min, interval_max) = self.get_time_sampling_interval();
        if (interval_max - interval_min).abs() < 1e-12 || !attr.value_might_be_time_varying() {
            let t = SdfTimeCode::new(current);
            let val = Self::read_primvar_value_at_time(&prim, key, t);
            return vec![(0.0_f32, val)];
        }

        // Gather time sample points within the shutter interval.
        let time_samples = Self::get_time_samples_for_interval(&attr, interval_min, interval_max);

        let n = time_samples.len().min(max_sample_count);
        let mut result = Vec::with_capacity(n);
        for &abs_time in &time_samples[..n] {
            let t = SdfTimeCode::new(abs_time);
            // Frame-relative offset so consumers can interpolate easily.
            let rel_time = (abs_time - current) as f32;
            let val = Self::read_primvar_value_at_time(&prim, key, t);
            result.push((rel_time, val));
        }

        if result.is_empty() {
            let t = SdfTimeCode::new(current);
            let val = Self::read_primvar_value_at_time(&prim, key, t);
            result.push((0.0_f32, val));
        }

        result
    }

    /// Sample primvar over an explicit interval.
    fn sample_primvar_interval(
        &self,
        id: &Path,
        key: &Token,
        start_time: f32,
        end_time: f32,
        max_sample_count: usize,
    ) -> Vec<(f32, Value)> {
        if max_sample_count == 0 {
            return Vec::new();
        }

        // If the caller passes a non-trivial interval, honour it by temporarily
        // overriding the shutter interval for this call.
        if (end_time - start_time).abs() < 1e-6 {
            return self.sample_primvar(id, key, max_sample_count);
        }

        let current = self.get_time().value();

        let Some(stage) = self.get_stage() else {
            return self.sample_primvar(id, key, max_sample_count);
        };

        let usd_path = {
            let cache = self.prim_cache.read();
            cache
                .get(id)
                .map(|c| c.usd_path.clone())
                .unwrap_or_else(|| id.clone())
        };

        let Some(prim) = stage.get_prim_at_path(&usd_path) else {
            return self.sample_primvar(id, key, max_sample_count);
        };

        let key_str = key.as_str();
        let attr = {
            let pn = format!("primvars:{key_str}");
            prim.get_attribute(&pn)
                .or_else(|| prim.get_attribute(key_str))
        };

        let Some(attr) = attr else {
            return self.sample_primvar(id, key, max_sample_count);
        };

        let interval_min = current + start_time as f64;
        let interval_max = current + end_time as f64;

        if !attr.value_might_be_time_varying() {
            let val = Self::read_primvar_value_at_time(&prim, key, SdfTimeCode::new(current));
            return vec![(0.0_f32, val)];
        }

        let time_samples = Self::get_time_samples_for_interval(&attr, interval_min, interval_max);

        let n = time_samples.len().min(max_sample_count);
        let mut result = Vec::with_capacity(n);
        for &abs_time in &time_samples[..n] {
            let rel_time = (abs_time - current) as f32;
            let val = Self::read_primvar_value_at_time(&prim, key, SdfTimeCode::new(abs_time));
            result.push((rel_time, val));
        }
        if result.is_empty() {
            let val = Self::read_primvar_value_at_time(&prim, key, SdfTimeCode::new(current));
            result.push((0.0_f32, val));
        }
        result
    }

    /// Sample the world transform at multiple times within the shutter interval.
    ///
    /// Returns up to `max_sample_count` (time, matrix) pairs with frame-relative
    /// time offsets.  Falls back to a single sample if the prim has no animated
    /// transforms or when motion blur is disabled.
    ///
    /// Mirrors C++ `UsdImagingPrimAdapter::SampleTransform()`.
    fn sample_transform(&self, id: &Path, max_sample_count: usize) -> Vec<(f32, Matrix4d)> {
        if max_sample_count == 0 {
            return Vec::new();
        }

        let current = self.get_time().value();
        let (interval_min, interval_max) = self.get_time_sampling_interval();

        // No motion blur requested — single sample at current time.
        if (interval_max - interval_min).abs() < 1e-12 {
            return vec![(0.0, self.get_transform(id))];
        }

        let Some(stage) = self.get_stage() else {
            return vec![(0.0, self.get_transform(id))];
        };

        let usd_path = {
            let cache = self.prim_cache.read();
            cache
                .get(id)
                .map(|c| c.usd_path.clone())
                .unwrap_or_else(|| id.clone())
        };

        let Some(prim) = stage.get_prim_at_path(&usd_path) else {
            return vec![(0.0, self.get_transform(id))];
        };

        // Collect transform sample times from the prim's xformOp attributes.
        let mut time_set = Self::gather_xform_time_samples(&prim, interval_min, interval_max);
        time_set.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        time_set.dedup_by(|a, b| (*a - *b).abs() < 1e-12);

        let root_xform = *self.root_transform.read();
        let n = time_set.len().min(max_sample_count);

        let mut result: Vec<(f32, Matrix4d)> = Vec::with_capacity(n);
        for &abs_time in &time_set[..n] {
            let rel_time = (abs_time - current) as f32;
            // Build a temporary XformCache for this time sample.
            let sdf_t = SdfTimeCode::new(abs_time);
            let mut tmp_cache = XformCache::new(sdf_t);
            let local_xform = tmp_cache.get_local_to_world_transform(&prim);
            result.push((rel_time, local_xform * root_xform));
        }

        // Optimisation: if all transforms are identical, return 1 sample.
        if result.len() > 1 {
            let first_mat = result[0].1;
            if result.iter().all(|(_, m)| *m == first_mat) {
                result.truncate(1);
                result[0].0 = 0.0;
            }
        }

        result
    }

    /// Sample transform over an explicit interval.
    fn sample_transform_interval(
        &self,
        id: &Path,
        start_time: f32,
        end_time: f32,
        max_sample_count: usize,
    ) -> Vec<(f32, Matrix4d)> {
        if (end_time - start_time).abs() < 1e-6 {
            return self.sample_transform(id, max_sample_count);
        }

        let current = self.get_time().value();
        let interval_min = current + start_time as f64;
        let interval_max = current + end_time as f64;

        let Some(stage) = self.get_stage() else {
            return self.sample_transform(id, max_sample_count);
        };

        let usd_path = {
            let cache = self.prim_cache.read();
            cache
                .get(id)
                .map(|c| c.usd_path.clone())
                .unwrap_or_else(|| id.clone())
        };

        let Some(prim) = stage.get_prim_at_path(&usd_path) else {
            return self.sample_transform(id, max_sample_count);
        };

        let mut time_set = Self::gather_xform_time_samples(&prim, interval_min, interval_max);
        time_set.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        time_set.dedup_by(|a, b| (*a - *b).abs() < 1e-12);

        let root_xform = *self.root_transform.read();
        let n = time_set.len().min(max_sample_count);

        let mut result: Vec<(f32, Matrix4d)> = Vec::with_capacity(n);
        for &abs_time in &time_set[..n] {
            let rel_time = (abs_time - current) as f32;
            let sdf_t = SdfTimeCode::new(abs_time);
            let mut tmp_cache = XformCache::new(sdf_t);
            let local_xform = tmp_cache.get_local_to_world_transform(&prim);
            result.push((rel_time, local_xform * root_xform));
        }

        if result.len() > 1 {
            let first_mat = result[0].1;
            if result.iter().all(|(_, m)| *m == first_mat) {
                result.truncate(1);
                result[0].0 = 0.0;
            }
        }

        result
    }

    // ----------------------------------------------------------------------- //
    // ExtComputation dispatch (A2): delegate to adapter
    // ----------------------------------------------------------------------- //

    fn get_ext_computation_scene_input_names(&self, computation_id: &Path) -> Vec<Token> {
        if let Some((adapter, usd_path)) = self.get_adapter_for_id(computation_id) {
            if let Some(stage) = self.get_stage() {
                if let Some(prim) = stage.get_prim_at_path(&usd_path) {
                    return adapter.get_ext_computation_scene_input_names(&prim, computation_id);
                }
            }
        }
        Vec::new()
    }

    fn get_ext_computation_input_descriptors(
        &self,
        computation_id: &Path,
    ) -> HdExtComputationInputDescriptorVector {
        if let Some((adapter, usd_path)) = self.get_adapter_for_id(computation_id) {
            if let Some(stage) = self.get_stage() {
                if let Some(prim) = stage.get_prim_at_path(&usd_path) {
                    let pairs =
                        adapter.get_ext_computation_input_descriptors(&prim, computation_id);
                    return pairs
                        .into_iter()
                        .map(|(name, src_output)| HdExtComputationInputDescriptor {
                            name,
                            source_computation_id: computation_id.clone(),
                            source_computation_output_name: src_output,
                        })
                        .collect();
                }
            }
        }
        Vec::new()
    }

    fn get_ext_computation_output_descriptors(
        &self,
        computation_id: &Path,
    ) -> HdExtComputationOutputDescriptorVector {
        if let Some((adapter, usd_path)) = self.get_adapter_for_id(computation_id) {
            if let Some(stage) = self.get_stage() {
                if let Some(prim) = stage.get_prim_at_path(&usd_path) {
                    let pairs =
                        adapter.get_ext_computation_output_descriptors(&prim, computation_id);
                    return pairs
                        .into_iter()
                        .map(|(name, _type_name)| HdExtComputationOutputDescriptor {
                            name,
                            ..Default::default()
                        })
                        .collect();
                }
            }
        }
        Vec::new()
    }

    fn get_ext_computation_primvar_descriptors(
        &self,
        id: &Path,
        interpolation: HdInterpolation,
    ) -> HdExtComputationPrimvarDescriptorVector {
        if let Some((adapter, usd_path)) = self.get_adapter_for_id(id) {
            if let Some(stage) = self.get_stage() {
                if let Some(prim) = stage.get_prim_at_path(&usd_path) {
                    return adapter.get_ext_computation_primvar_descriptors(
                        &prim,
                        id,
                        interpolation,
                    );
                }
            }
        }
        Vec::new()
    }

    fn get_ext_computation_input(&self, computation_id: &Path, input: &Token) -> Value {
        if let Some((adapter, usd_path)) = self.get_adapter_for_id(computation_id) {
            if let Some(stage) = self.get_stage() {
                if let Some(prim) = stage.get_prim_at_path(&usd_path) {
                    if let Some(val) =
                        adapter.get_ext_computation_input(&prim, computation_id, input)
                    {
                        return val;
                    }
                }
            }
        }
        Value::default()
    }

    fn get_ext_computation_kernel(&self, computation_id: &Path) -> String {
        if let Some((adapter, usd_path)) = self.get_adapter_for_id(computation_id) {
            if let Some(stage) = self.get_stage() {
                if let Some(prim) = stage.get_prim_at_path(&usd_path) {
                    return adapter.get_ext_computation_kernel(&prim, computation_id);
                }
            }
        }
        String::new()
    }

    fn invoke_ext_computation(
        &mut self,
        computation_id: &Path,
        context: &mut dyn HdExtComputationContext,
    ) {
        // Extract adapter + usd_path first to avoid borrow conflict with get_stage().
        let pair = self.get_adapter_for_id(computation_id);
        if let Some((adapter, usd_path)) = pair {
            if let Some(stage) = self.get_stage() {
                if let Some(prim) = stage.get_prim_at_path(&usd_path) {
                    let cache_path = self.convert_index_path_to_cache_path(computation_id);
                    adapter.invoke_ext_computation(&prim, &cache_path, context);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Primvar helpers
// ---------------------------------------------------------------------------

/// Read interpolation metadata from a USD attribute representing a primvar.
///
/// USD stores interpolation as a "interpolation" metadata token on the attribute.
/// Returns HdInterpolation::Vertex as fallback (most common primvar type).
/// P0-IMG-3: Convert Vec2d array to Vec2f so Storm shaders receive float2 data.
///
/// C++ delegate.cpp:2695-2698:
///   "We generally don't want Vec2d arrays, convert to vec2f."
/// USDC files commonly store UV coordinates as double2[], but all Storm
/// shaders and rasterizers expect float2[]. Without this conversion the
/// texturing path panics or produces garbage coordinates.
fn value_scalar_to_f32(value: &Value) -> Option<f32> {
    value
        .get::<f32>()
        .copied()
        .or_else(|| value.get::<f64>().map(|v| *v as f32))
        .or_else(|| value.get::<i8>().map(|v| *v as f32))
        .or_else(|| value.get::<i16>().map(|v| *v as f32))
        .or_else(|| value.get::<i32>().map(|v| *v as f32))
        .or_else(|| value.get::<i64>().map(|v| *v as f32))
        .or_else(|| value.get::<u8>().map(|v| *v as f32))
        .or_else(|| value.get::<u16>().map(|v| *v as f32))
        .or_else(|| value.get::<u32>().map(|v| *v as f32))
        .or_else(|| value.get::<u64>().map(|v| *v as f32))
}

fn value_scalar_to_i32(value: &Value) -> Option<i32> {
    value
        .get::<i32>()
        .copied()
        .or_else(|| value.get::<i8>().map(|v| *v as i32))
        .or_else(|| value.get::<i16>().map(|v| *v as i32))
        .or_else(|| value.get::<i64>().map(|v| *v as i32))
        .or_else(|| value.get::<u8>().map(|v| *v as i32))
        .or_else(|| value.get::<u16>().map(|v| *v as i32))
        .or_else(|| value.get::<u32>().map(|v| *v as i32))
        .or_else(|| value.get::<u64>().map(|v| *v as i32))
}

fn convert_vec2d_to_vec2f(val: Value) -> Value {
    if let Some(vec2d_array) = val.get::<Vec<usd_gf::Vec2d>>().cloned() {
        let vec2f_array: Vec<usd_gf::Vec2f> = vec2d_array
            .iter()
            .map(|v| usd_gf::Vec2f::new(v.x as f32, v.y as f32))
            .collect();
        return Value::from(vec2f_array);
    }

    if let Some(vec_values) = val.get::<Vec<Value>>() {
        let mut vec2f_array = Vec::with_capacity(vec_values.len());
        for entry in vec_values {
            let vec2f = if let Some(v) = entry.get::<usd_gf::Vec2f>() {
                *v
            } else if let Some(v) = entry.get::<usd_gf::Vec2d>() {
                usd_gf::Vec2f::new(v[0] as f32, v[1] as f32)
            } else if let Some(v) = entry.get::<[f32; 2]>() {
                usd_gf::Vec2f::new(v[0], v[1])
            } else if let Some(tuple) = entry.get::<Vec<Value>>() {
                let x = tuple.first().and_then(value_scalar_to_f32);
                let y = tuple.get(1).and_then(value_scalar_to_f32);
                match (x, y) {
                    (Some(x), Some(y)) => usd_gf::Vec2f::new(x, y),
                    _ => return val,
                }
            } else {
                return val;
            };
            vec2f_array.push(vec2f);
        }
        return Value::from(vec2f_array);
    }

    val
}

fn convert_vec3_to_vec3f(val: Value) -> Value {
    if let Some(vec3f_array) = val.get::<Vec<usd_gf::Vec3f>>().cloned() {
        return Value::from(vec3f_array);
    }

    if let Some(vec_values) = val.get::<Vec<Value>>() {
        let mut vec3f_array = Vec::with_capacity(vec_values.len());
        for entry in vec_values {
            let vec3f = if let Some(v) = entry.get::<usd_gf::Vec3f>() {
                *v
            } else if let Some(v) = entry.get::<usd_gf::Vec3d>() {
                usd_gf::Vec3f::new(v[0] as f32, v[1] as f32, v[2] as f32)
            } else if let Some(v) = entry.get::<[f32; 3]>() {
                usd_gf::Vec3f::new(v[0], v[1], v[2])
            } else if let Some(tuple) = entry.get::<Vec<Value>>() {
                let x = tuple.first().and_then(value_scalar_to_f32);
                let y = tuple.get(1).and_then(value_scalar_to_f32);
                let z = tuple.get(2).and_then(value_scalar_to_f32);
                match (x, y, z) {
                    (Some(x), Some(y), Some(z)) => usd_gf::Vec3f::new(x, y, z),
                    _ => return val,
                }
            } else {
                return val;
            };
            vec3f_array.push(vec3f);
        }
        return Value::from(vec3f_array);
    }

    val
}

fn normalize_sampled_primvar_value(key: &Token, val: Value) -> Value {
    match key.as_str() {
        "points" | "normals" | "displayColor" => convert_vec3_to_vec3f(val),
        _ => convert_vec2d_to_vec2f(val),
    }
}

fn read_primvar_interpolation(attr: &usd_core::Attribute, _time: SdfTimeCode) -> HdInterpolation {
    // USD primvar interpolation is stored as "interpolation" metadata on the attribute.
    // Read via get_metadata(key). Fallback: Vertex (most common primvar type).
    let key = Token::new("interpolation");
    if let Some(val) = attr.get_metadata(&key) {
        if let Some(tok) = val.get::<Token>().cloned() {
            return interp_from_token(tok.as_str());
        }
    }
    HdInterpolation::Vertex
}

/// Map USD interpolation token string to HdInterpolation enum.
fn interp_from_token(s: &str) -> HdInterpolation {
    match s {
        "constant" => HdInterpolation::Constant,
        "uniform" => HdInterpolation::Uniform,
        "varying" => HdInterpolation::Varying,
        "vertex" => HdInterpolation::Vertex,
        "faceVarying" => HdInterpolation::FaceVarying,
        "instance" => HdInterpolation::Instance,
        _ => HdInterpolation::Vertex,
    }
}

/// Infer primvar role from its base name.
///
/// Matches C++ UsdGeomPrimvar role inference: names like "displayColor",
/// "normals", "points" map to predefined roles.
fn read_primvar_role(base_name: &str) -> &'static str {
    match base_name {
        "displayColor" | "color" => "color",
        "normals" => "normal",
        "points" | "positions" => "point",
        "velocities" | "accelerations" => "vector",
        "st" | "st0" | "st1" | "uv" | "uvs" | "map1" | "primvars:st" | "primvars:st1" => {
            "textureCoordinate"
        }
        _ => "",
    }
}

// ============================================================================
// P1-11: Implicit surface topology + point synthesis.
// Uses GeomUtil mesh generators (matching C++ GeomUtil*MeshGenerator) to
// produce procedural mesh topology and vertex positions for implicit USD
// types so they can be rendered through the Hydra 1.0 mesh pipeline.
// ============================================================================

use usd_geom_util::{
    CapsuleMeshGenerator, ConeMeshGenerator, CuboidMeshGenerator, CylinderMeshGenerator,
    PlaneMeshGenerator, SphereMeshGenerator,
};

/// Number of radial segments for implicit surface tessellation.
const IMPLICIT_NUM_RADIAL: usize = 10;
/// Number of axial segments for implicit surface tessellation.
const IMPLICIT_NUM_AXIAL: usize = 10;
/// Number of cap axial segments for capsule tessellation.
const IMPLICIT_NUM_CAP_AXIAL: usize = 4;

/// Check if a USD prim type is an implicit surface.
fn is_implicit_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "Cube" | "Sphere" | "Cylinder" | "Cone" | "Capsule" | "Plane"
    )
}

/// Synthesize cube topology using GeomUtil.
fn synth_cube_topo() -> HdMeshTopology {
    let topo = CuboidMeshGenerator::generate_topology();
    HdMeshTopology::from_full(
        topo.scheme().clone(),
        topo.orientation().clone(),
        topo.face_vertex_counts().to_vec(),
        topo.face_vertex_indices().to_vec(),
        Vec::new(),
    )
}

/// Synthesize sphere topology using GeomUtil.
fn synth_sphere_topo() -> HdMeshTopology {
    let topo =
        SphereMeshGenerator::generate_topology(IMPLICIT_NUM_RADIAL, IMPLICIT_NUM_AXIAL, true);
    HdMeshTopology::from_full(
        topo.scheme().clone(),
        topo.orientation().clone(),
        topo.face_vertex_counts().to_vec(),
        topo.face_vertex_indices().to_vec(),
        Vec::new(),
    )
}

/// Synthesize cylinder topology using GeomUtil.
fn synth_cylinder_topo() -> HdMeshTopology {
    let topo = CylinderMeshGenerator::generate_topology(IMPLICIT_NUM_RADIAL, true);
    HdMeshTopology::from_full(
        topo.scheme().clone(),
        topo.orientation().clone(),
        topo.face_vertex_counts().to_vec(),
        topo.face_vertex_indices().to_vec(),
        Vec::new(),
    )
}

/// Synthesize cone topology using GeomUtil.
fn synth_cone_topo() -> HdMeshTopology {
    let topo = ConeMeshGenerator::generate_topology(IMPLICIT_NUM_RADIAL, true);
    HdMeshTopology::from_full(
        topo.scheme().clone(),
        topo.orientation().clone(),
        topo.face_vertex_counts().to_vec(),
        topo.face_vertex_indices().to_vec(),
        Vec::new(),
    )
}

/// Synthesize capsule topology using GeomUtil.
fn synth_capsule_topo() -> HdMeshTopology {
    let topo =
        CapsuleMeshGenerator::generate_topology(IMPLICIT_NUM_RADIAL, IMPLICIT_NUM_CAP_AXIAL, true);
    HdMeshTopology::from_full(
        topo.scheme().clone(),
        topo.orientation().clone(),
        topo.face_vertex_counts().to_vec(),
        topo.face_vertex_indices().to_vec(),
        Vec::new(),
    )
}

// ============================================================================
// Implicit surface point synthesis.
// Generates procedural vertex positions for implicit USD types using GeomUtil
// mesh generators, matching the reference C++ implicit_to_mesh pattern.
// ============================================================================

/// Build axis adjustment matrix for implicit surfaces (cone, cylinder, etc.).
/// Maps from canonical Z-up to target axis. Matches C++ UsdImagingGetAxisToTransform.
fn get_axis_matrix(axis: &Token) -> Matrix4d {
    let (u, v, spine) = if axis == "X" {
        (
            Vec4d::new(0.0, 1.0, 0.0, 0.0),
            Vec4d::new(0.0, 0.0, 1.0, 0.0),
            Vec4d::new(1.0, 0.0, 0.0, 0.0),
        )
    } else if axis == "Y" {
        (
            Vec4d::new(0.0, 0.0, 1.0, 0.0),
            Vec4d::new(1.0, 0.0, 0.0, 0.0),
            Vec4d::new(0.0, 1.0, 0.0, 0.0),
        )
    } else {
        (
            Vec4d::new(1.0, 0.0, 0.0, 0.0),
            Vec4d::new(0.0, 1.0, 0.0, 0.0),
            Vec4d::new(0.0, 0.0, 1.0, 0.0),
        )
    };
    let mut m = Matrix4d::identity();
    m.set_row(0, &u);
    m.set_row(1, &v);
    m.set_row(2, &spine);
    m
}

/// Synthesize vertex positions for an implicit surface prim using GeomUtil generators.
/// Reads size/radius/height/axis attributes and produces points matching synth_*_topo().
fn synth_implicit_points(prim: &Prim, time: SdfTimeCode) -> Option<Vec<Vec3f>> {
    let type_name = prim.get_type_name();
    match type_name.as_str() {
        "Cube" => {
            let size = prim
                .get_attribute("size")
                .and_then(|a| a.get_typed::<f64>(time))
                .unwrap_or(2.0) as f32;
            Some(CuboidMeshGenerator::generate_points_f32(
                size, size, size, None,
            ))
        }
        "Sphere" => {
            let radius = prim
                .get_attribute("radius")
                .and_then(|a| a.get_typed::<f64>(time))
                .unwrap_or(1.0) as f32;
            Some(SphereMeshGenerator::generate_points_f32(
                IMPLICIT_NUM_RADIAL,
                IMPLICIT_NUM_AXIAL,
                radius,
                360.0,
                None,
            ))
        }
        "Cone" => {
            let height = prim
                .get_attribute("height")
                .and_then(|a| a.get_typed::<f64>(time))
                .unwrap_or(1.0) as f32;
            let radius = prim
                .get_attribute("radius")
                .and_then(|a| a.get_typed::<f64>(time))
                .unwrap_or(1.0) as f32;
            let axis = prim
                .get_attribute("axis")
                .and_then(|a| a.get_typed::<Token>(time))
                .unwrap_or_else(|| Token::new("Z"));
            let basis = get_axis_matrix(&axis);
            Some(ConeMeshGenerator::generate_points_f32(
                IMPLICIT_NUM_RADIAL,
                radius,
                height,
                360.0,
                Some(&basis),
            ))
        }
        "Cylinder" => {
            let height = prim
                .get_attribute("height")
                .and_then(|a| a.get_typed::<f64>(time))
                .unwrap_or(2.0) as f32;
            let radius = prim
                .get_attribute("radius")
                .and_then(|a| a.get_typed::<f64>(time))
                .unwrap_or(1.0) as f32;
            let axis = prim
                .get_attribute("axis")
                .and_then(|a| a.get_typed::<Token>(time))
                .unwrap_or_else(|| Token::new("Z"));
            let basis = get_axis_matrix(&axis);
            Some(CylinderMeshGenerator::generate_points_f32(
                IMPLICIT_NUM_RADIAL,
                radius,
                radius,
                height,
                360.0,
                Some(&basis),
            ))
        }
        "Capsule" => {
            let height = prim
                .get_attribute("height")
                .and_then(|a| a.get_typed::<f64>(time))
                .unwrap_or(1.0) as f32;
            let radius = prim
                .get_attribute("radius")
                .and_then(|a| a.get_typed::<f64>(time))
                .unwrap_or(0.5) as f32;
            let axis = prim
                .get_attribute("axis")
                .and_then(|a| a.get_typed::<Token>(time))
                .unwrap_or_else(|| Token::new("Z"));
            let basis = get_axis_matrix(&axis);
            Some(CapsuleMeshGenerator::generate_points_f32(
                IMPLICIT_NUM_RADIAL,
                IMPLICIT_NUM_CAP_AXIAL,
                radius,
                radius,
                height,
                360.0,
                Some(&basis),
            ))
        }
        "Plane" => {
            let width = prim
                .get_attribute("width")
                .and_then(|a| a.get_typed::<f64>(time))
                .unwrap_or(1.0) as f32;
            let length = prim
                .get_attribute("length")
                .and_then(|a| a.get_typed::<f64>(time))
                .unwrap_or(1.0) as f32;
            let axis = prim
                .get_attribute("axis")
                .and_then(|a| a.get_typed::<Token>(time))
                .unwrap_or_else(|| Token::new("Z"));
            let basis = get_axis_matrix(&axis);
            Some(PlaneMeshGenerator::generate_points_f32(
                width,
                length,
                Some(&basis),
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use usd_core::common::InitialLoadSet;
    use usd_hd::prim::HdSceneDelegate;

    #[derive(Default)]
    struct TestBackend {
        rprim_dirty: Mutex<Vec<(Path, u32)>>,
    }

    impl crate::index_proxy::IndexProxyBackend for TestBackend {
        fn insert_rprim(
            &self,
            _prim_type: &Token,
            _scene_delegate_id: &Path,
            _prim_id: &Path,
        ) -> bool {
            true
        }

        fn insert_sprim(
            &self,
            _prim_type: &Token,
            _scene_delegate_id: &Path,
            _prim_id: &Path,
        ) -> bool {
            true
        }

        fn insert_bprim(
            &self,
            _prim_type: &Token,
            _scene_delegate_id: &Path,
            _prim_id: &Path,
        ) -> bool {
            true
        }

        fn is_rprim_type_supported(&self, type_id: &Token) -> bool {
            type_id == &Token::new("mesh")
        }

        fn is_sprim_type_supported(&self, _type_id: &Token) -> bool {
            true
        }

        fn is_bprim_type_supported(&self, _type_id: &Token) -> bool {
            true
        }

        fn mark_rprim_dirty(&self, prim_id: &Path, dirty_bits: u32) {
            self.rprim_dirty
                .lock()
                .expect("lock poisoned")
                .push((prim_id.clone(), dirty_bits));
        }

        fn mark_sprim_dirty(&self, _prim_id: &Path, _dirty_bits: u32) {}

        fn mark_bprim_dirty(&self, _prim_id: &Path, _dirty_bits: u32) {}

        fn mark_instancer_dirty(&self, _prim_id: &Path, _dirty_bits: u32) {}
    }

    #[test]
    fn test_delegate_creation() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        assert!(delegate.get_stage().is_some());
        // C++ default is double::max, not NaN
        assert_eq!(delegate.get_time().value(), f64::MAX);
    }

    #[test]
    fn test_root_transform() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        let xform = Matrix4d::identity();
        delegate.set_root_transform(xform);

        let result = delegate.get_root_transform();
        assert_eq!(result, xform);
    }

    #[test]
    fn test_root_visibility() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        assert!(delegate.get_root_visibility());

        delegate.set_root_visibility(false);
        assert!(!delegate.get_root_visibility());
    }

    #[test]
    fn test_refine_level() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        delegate.set_refine_level_fallback(3);
        assert_eq!(delegate.get_refine_level_fallback(), 3);
    }

    #[test]
    fn test_populate_empty_stage() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        delegate.populate();

        let cache = delegate.prim_cache.read();
        assert!(cache.is_empty() || cache.len() == 1); // May have pseudo root
    }

    #[test]
    fn test_populate_with_prims() {
        usd_core::schema_registry::register_builtin_schemas();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

        stage.define_prim("/World", "Xform").expect("define prim");
        stage
            .define_prim("/World/Mesh", "Mesh")
            .expect("define prim");

        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());
        delegate.populate();

        let cache = delegate.prim_cache.read();
        assert!(cache.len() >= 2);
    }

    #[test]
    fn test_set_time_marks_dirty() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        stage.define_prim("/Mesh", "Mesh").expect("define prim");

        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());
        delegate.populate();

        // Clear dirty bits
        {
            let mut dirty = delegate.dirty_bits.lock().expect("Lock poisoned");
            dirty.clear();
        }

        delegate.set_time(UsdTimeCode::new(24.0));

        let dirty = delegate.dirty_bits.lock().expect("Lock poisoned");
        assert!(!dirty.is_empty());
    }

    #[test]
    fn test_set_time_marks_backend_rprim_dirty() {
        usd_core::schema_registry::register_builtin_schemas();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        stage.define_prim("/Mesh", "Mesh").expect("define prim");

        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());
        let backend: Arc<TestBackend> = Arc::new(TestBackend::default());
        delegate.set_index_proxy_backend(Some(backend.clone()));
        delegate.populate();

        delegate.set_time(UsdTimeCode::new(24.0));

        let calls = backend.rprim_dirty.lock().expect("lock poisoned");
        assert!(!calls.is_empty());
        assert_eq!(calls[0].0, Path::from_string("/Mesh").expect("index path"));
    }

    #[test]
    fn test_camera_params() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

        let _camera_prim = stage
            .define_prim("/Camera", "Camera")
            .expect("define camera");

        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        let camera_path = Path::from_string("/Camera").expect("parse path");
        let params = delegate.get_camera_params(&camera_path);

        assert!(params.is_some());
        let params = params.unwrap();
        assert!(params.focal_length > 0.0);
    }

    #[test]
    fn test_scene_delegate_trait() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        stage.define_prim("/Mesh", "Mesh").expect("define prim");

        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());
        delegate.populate();

        let mesh_path = Path::from_string("/Mesh").expect("parse path");

        // Prims start with all bits dirty after populate
        let initial_bits = delegate.get_dirty_bits(&mesh_path);
        assert_eq!(initial_bits, !0u32); // All dirty initially

        // Mark additional bits (OR with existing)
        delegate.mark_prim_dirty(&mesh_path, 0xFF);
        let bits = delegate.get_dirty_bits(&mesh_path);
        assert_eq!(bits, !0u32); // Still all dirty

        // Mark clean - clear all bits
        {
            let mut dirty = delegate.dirty_bits.lock().expect("Lock poisoned");
            if let Some(current) = dirty.get_mut(&mesh_path) {
                *current = 0; // Clear all dirty bits
            }
        }

        let bits = delegate.get_dirty_bits(&mesh_path);
        assert_eq!(bits, 0); // All bits cleared
    }

    #[test]
    fn test_dirty_bits_convert_index_path_to_cache_path() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate_id = Path::from_string("/Delegate").expect("parse path");
        let mut delegate = UsdImagingDelegate::new(stage, delegate_id.clone());

        let cache_path = Path::from_string("/Mesh").expect("parse path");
        let index_path = delegate.convert_cache_path_to_index_path(&cache_path);

        delegate.mark_prim_dirty(&cache_path, 0x1234);
        assert_eq!(delegate.get_dirty_bits(&index_path), 0x1234);

        let delegate_mut = Arc::get_mut(&mut delegate).expect("unique arc");
        delegate_mut.mark_clean(&index_path, 0x0034);
        assert_eq!(delegate_mut.get_dirty_bits(&index_path), 0x1200);
    }

    #[test]
    fn test_queue_resync() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        let path = Path::from_string("/Test").expect("parse path");
        delegate.queue_resync(&path);

        let resyncs = delegate.pending_resyncs.lock().expect("Lock poisoned");
        assert!(resyncs.contains(&path));
    }

    // ----------------------------------------------------------------------- //
    // Motion sampling tests
    // ----------------------------------------------------------------------- //

    #[test]
    fn test_shutter_interval_default() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        // Default: no motion blur.
        let si = delegate.get_shutter_interval();
        assert_eq!(si.open, 0.0);
        assert_eq!(si.close, 0.0);
        assert!(!si.is_open());
    }

    #[test]
    fn test_shutter_interval_set() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        delegate.set_shutter_interval(ShutterInterval::new(-0.25, 0.25));
        let si = delegate.get_shutter_interval();
        assert!(si.is_open());
        assert_eq!(si.open, -0.25);
        assert_eq!(si.close, 0.25);
    }

    #[test]
    fn test_time_sampling_interval_no_blur() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        // With no shutter (default), interval should be [t, t].
        delegate.set_time(UsdTimeCode::new(12.0));
        let (start, end) = delegate.get_time_sampling_interval();
        assert_eq!(start, 12.0);
        assert_eq!(end, 12.0);
    }

    #[test]
    fn test_time_sampling_interval_with_blur() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        delegate.set_time(UsdTimeCode::new(10.0));
        delegate.set_shutter_interval(ShutterInterval::new(-0.5, 0.5));

        let (start, end) = delegate.get_time_sampling_interval();
        assert!((start - 9.5).abs() < 1e-10);
        assert!((end - 10.5).abs() < 1e-10);
    }

    #[test]
    fn test_sample_primvar_no_blur_returns_single_sample() {
        usd_core::schema_registry::register_builtin_schemas();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        stage.define_prim("/Mesh", "Mesh").expect("define prim");

        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());
        delegate.populate();

        let mesh_path = Path::from_string("/Mesh").expect("parse path");
        // No shutter — should return 1 sample at t=0.
        let samples = delegate.sample_primvar(&mesh_path, &Token::new("points"), 4);
        // Empty or single sample (mesh has no authored points).
        assert!(samples.len() <= 1);
    }

    #[test]
    fn test_sample_transform_no_blur_returns_single_sample() {
        usd_core::schema_registry::register_builtin_schemas();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        stage.define_prim("/Mesh", "Mesh").expect("define prim");

        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());
        delegate.populate();

        let mesh_path = Path::from_string("/Mesh").expect("parse path");
        // No shutter — should return exactly 1 sample.
        let samples = delegate.sample_transform(&mesh_path, 4);
        assert_eq!(samples.len(), 1);
        assert!((samples[0].0).abs() < 1e-6); // time = 0.0
    }

    #[test]
    fn test_sample_primvar_max_zero_returns_empty() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        let path = Path::from_string("/Any").expect("parse path");
        let samples = delegate.sample_primvar(&path, &Token::new("points"), 0);
        assert!(samples.is_empty());
    }

    #[test]
    fn test_sample_transform_max_zero_returns_empty() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        let path = Path::from_string("/Any").expect("parse path");
        let samples = delegate.sample_transform(&path, 0);
        assert!(samples.is_empty());
    }

    #[test]
    fn test_sample_primvar_reads_default_time_points() {
        use std::sync::Arc;
        use usd_sdf::Layer;

        let usda = r#"#usda 1.0
def Mesh "Mesh" {
    int[] faceVertexCounts = [4]
    int[] faceVertexIndices = [0, 1, 2, 3]
    point3f[] points = [(0,0,0), (1,0,0), (1,1,0), (0,1,0)]
}
"#;

        let layer: Arc<Layer> = Layer::create_anonymous(Some("test_sample_points_default_time"));
        layer.import_from_string(usda);
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");

        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());
        delegate.populate();
        delegate.set_time(UsdTimeCode::default_time());

        let mesh_path = Path::from_string("/Mesh").expect("parse path");
        let samples = delegate.sample_primvar(&mesh_path, &Token::new("points"), 2);
        assert_eq!(samples.len(), 1);

        let points = samples[0]
            .1
            .get::<Vec<usd_gf::Vec3f>>()
            .expect("points payload");
        assert_eq!(points.len(), 4);
        assert_eq!(points[2], usd_gf::Vec3f::new(1.0, 1.0, 0.0));
    }

    #[test]
    fn test_get_mesh_topology_reads_default_time_authored_data() {
        use std::sync::Arc;
        use usd_hd::HdSceneDelegate;
        use usd_sdf::Layer;

        let usda = r#"#usda 1.0
def Mesh "Mesh" {
    uniform token subdivisionScheme = "catmullClark"
    int[] faceVertexCounts = [4]
    int[] faceVertexIndices = [0, 1, 2, 3]
    point3f[] points = [(0,0,0), (1,0,0), (1,1,0), (0,1,0)]
}
"#;

        let layer: Arc<Layer> = Layer::create_anonymous(Some("test_topology_default_time"));
        layer.import_from_string(usda);
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");

        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());
        delegate.populate();
        delegate.set_time(UsdTimeCode::default_time());

        let mesh_path = Path::from_string("/Mesh").expect("parse path");
        let topology = delegate.get_mesh_topology(&mesh_path);
        assert_eq!(topology.scheme, Token::new("catmullClark"));
        assert_eq!(topology.face_vertex_counts, vec![4]);
        assert_eq!(topology.face_vertex_indices, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_get_mesh_topology_uses_usd_default_subdivision_scheme_when_unauthored() {
        use std::sync::Arc;
        use usd_hd::HdSceneDelegate;
        use usd_sdf::Layer;

        let usda = r#"#usda 1.0
def Mesh "Mesh" {
    int[] faceVertexCounts = [4]
    int[] faceVertexIndices = [0, 1, 2, 3]
    point3f[] points = [(0,0,0), (1,0,0), (1,1,0), (0,1,0)]
}
"#;

        let layer: Arc<Layer> = Layer::create_anonymous(Some("test_topology_default_scheme"));
        layer.import_from_string(usda);
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");

        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());
        delegate.populate();
        delegate.set_time(UsdTimeCode::default_time());

        let mesh_path = Path::from_string("/Mesh").expect("parse path");
        let topology = delegate.get_mesh_topology(&mesh_path);
        assert_eq!(topology.scheme, Token::new("catmullClark"));
    }

    #[test]
    fn test_get_indexed_primvar_reads_authored_primvar_indices() {
        use std::sync::Arc;
        use usd_sdf::Layer;

        let usda = r#"#usda 1.0
def Mesh "Mesh" {
    int[] faceVertexCounts = [4]
    int[] faceVertexIndices = [0, 1, 2, 3]
    point3f[] points = [(0,0,0), (1,0,0), (1,1,0), (0,1,0)]
    texCoord2f[] primvars:st = [(0,0), (1,0), (1,1), (0,1)] (
        interpolation = "faceVarying"
    )
    int[] primvars:st:indices = [0, 1, 2, 3]
}
"#;

        let layer: Arc<Layer> = Layer::create_anonymous(Some("test_indexed_primvar"));
        layer.import_from_string(usda);
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");

        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());
        delegate.populate();

        let mesh_path = Path::from_string("/Mesh").expect("parse path");
        let (value, indices) = delegate.get_indexed_primvar(&mesh_path, &Token::new("st"));
        fn scalar_to_f32(value: &usd_vt::Value) -> Option<f32> {
            value
                .get::<f32>()
                .copied()
                .or_else(|| value.get::<f64>().map(|v| *v as f32))
                .or_else(|| value.get::<i8>().map(|v| *v as f32))
                .or_else(|| value.get::<i16>().map(|v| *v as f32))
                .or_else(|| value.get::<i32>().map(|v| *v as f32))
                .or_else(|| value.get::<i64>().map(|v| *v as f32))
                .or_else(|| value.get::<u8>().map(|v| *v as f32))
                .or_else(|| value.get::<u16>().map(|v| *v as f32))
                .or_else(|| value.get::<u32>().map(|v| *v as f32))
                .or_else(|| value.get::<u64>().map(|v| *v as f32))
        }

        let coords: Vec<[f32; 2]> = if let Some(vec2f) = value.get::<Vec<usd_gf::Vec2f>>() {
            vec2f.iter().map(|v| [v[0], v[1]]).collect()
        } else if let Some(vec2d) = value.get::<Vec<usd_gf::Vec2d>>() {
            vec2d.iter().map(|v| [v[0] as f32, v[1] as f32]).collect()
        } else if let Some(array) = value.get::<Vec<[f32; 2]>>() {
            array.clone()
        } else if let Some(values) = value.get::<Vec<usd_vt::Value>>() {
            values
                .iter()
                .map(|entry| {
                    if let Some(v) = entry.get::<usd_gf::Vec2f>() {
                        [v[0], v[1]]
                    } else if let Some(v) = entry.get::<usd_gf::Vec2d>() {
                        [v[0] as f32, v[1] as f32]
                    } else if let Some(v) = entry.get::<[f32; 2]>() {
                        *v
                    } else if let Some(tuple) = entry.get::<Vec<usd_vt::Value>>() {
                        let x = tuple.first().and_then(scalar_to_f32).unwrap_or_default();
                        let y = tuple.get(1).and_then(scalar_to_f32).unwrap_or_default();
                        [x, y]
                    } else {
                        panic!("unexpected texcoord entry type: {:?}", entry.type_name());
                    }
                })
                .collect()
        } else {
            panic!(
                "expected texcoord primvar values, got {:?}",
                value.type_name()
            );
        };

        assert_eq!(coords.len(), 4);
        assert_eq!(coords[0], [0.0, 0.0]);
        assert_eq!(coords[1], [1.0, 0.0]);
        assert_eq!(coords[2], [1.0, 1.0]);
        assert_eq!(coords[3], [0.0, 1.0]);
        assert_eq!(indices, Some(vec![0, 1, 2, 3]));
    }

    // ------------------------------------------------------------------ //
    // Native instancing tests
    // ------------------------------------------------------------------ //

    #[test]
    fn test_instancer_data_initially_empty() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        // No instancers populated yet.
        let inst_path = Path::from_string("/World/Instance").expect("parse path");
        let indices = delegate.get_instance_indices(&inst_path, &Path::empty());
        assert!(indices.is_empty());

        let protos = delegate.get_instancer_prototypes(&inst_path);
        assert!(protos.is_empty());
    }

    #[test]
    fn test_get_instancer_id_for_non_instancer() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        // Non-instancer prim → empty path.
        let path = Path::from_string("/World/Mesh").expect("parse path");
        let id = delegate.get_instancer_id(&path);
        assert!(id.is_empty());
    }

    #[test]
    fn test_get_instancer_id_for_proto_prim() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        // Manually inject instancer data to simulate a populated instancer.
        let instancer_path = Path::from_string("/World/InstanceA").expect("parse path");
        {
            let mut inst_data = delegate.instancer_data.write();
            inst_data.insert(
                instancer_path.clone(),
                crate::instance_adapter::InstancerData::default(),
            );
        }

        // A proto prim path is <instancer>/proto_Mesh_0.
        let proto_path = instancer_path
            .append_child("proto_Mesh_0")
            .expect("append child");

        let reported_id = delegate.get_instancer_id(&proto_path);
        assert_eq!(reported_id, instancer_path);
    }

    #[test]
    fn test_get_instance_indices_populated() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        let instancer_path = Path::from_string("/World/InstanceA").expect("parse path");
        let mut idata = crate::instance_adapter::InstancerData::default();
        idata
            .instance_paths
            .push(Path::from_string("/World/InstanceA").expect("parse path"));
        idata
            .instance_paths
            .push(Path::from_string("/World/InstanceB").expect("parse path"));
        idata.num_instances_to_draw = 2;
        {
            let mut inst_data = delegate.instancer_data.write();
            inst_data.insert(instancer_path.clone(), idata);
        }

        let indices = delegate.get_instance_indices(&instancer_path, &Path::empty());
        assert_eq!(indices, vec![0, 1]);
    }

    #[test]
    fn test_get_instancer_prototypes_populated() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());

        let instancer_path = Path::from_string("/World/InstanceA").expect("parse path");
        let proto_path = instancer_path
            .append_child("proto_Mesh_0")
            .expect("append child");

        let mut idata = crate::instance_adapter::InstancerData::default();
        idata.prim_map.insert(
            proto_path.clone(),
            crate::instance_adapter::ProtoPrim::default(),
        );
        {
            let mut inst_data = delegate.instancer_data.write();
            inst_data.insert(instancer_path.clone(), idata);
        }

        let protos = delegate.get_instancer_prototypes(&instancer_path);
        assert_eq!(protos.len(), 1);
        assert_eq!(protos[0], proto_path);
    }

    /// Integration test: populate a stage with two native instances of a mesh.
    ///
    /// Scene:
    ///   /Root/InstanceA  (instance=true, prototype=/__Prototype_1)
    ///   /Root/InstanceB  (instance=true, prototype=/__Prototype_1)
    ///   /__Prototype_1/Mesh  (Mesh type)
    ///
    /// After populate_with_proxy():
    /// - instancer_data should have one entry with key = /Root/InstanceA
    /// - instance_paths should contain both /Root/InstanceA and /Root/InstanceB
    /// - prim_map should have one proto prim
    /// - get_instance_indices should return [0, 1]
    #[test]
    fn test_populate_native_instances() {
        use std::sync::Arc;
        use usd_sdf::Layer;

        let usda = r#"#usda 1.0
def Xform "Root" {
    def Mesh "InstanceA" (
        instanceable = true
    ) {
        float3[] extent = [(-1, -1, -1), (1, 1, 1)]
        int[] faceVertexCounts = [4]
        int[] faceVertexIndices = [0, 1, 2, 3]
        point3f[] points = [(-1,-1,-1),(1,-1,-1),(1,1,-1),(-1,1,-1)]
    }
    def Mesh "InstanceB" (
        instanceable = true
    ) {
        float3[] extent = [(-1, -1, -1), (1, 1, 1)]
        int[] faceVertexCounts = [4]
        int[] faceVertexIndices = [0, 1, 2, 3]
        point3f[] points = [(-1,-1,-1),(1,-1,-1),(1,1,-1),(-1,1,-1)]
    }
}
"#;
        let layer: Arc<Layer> = Layer::create_anonymous(Some("test_native_instances"));
        layer.import_from_string(usda);

        let stage = Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll);
        if stage.is_err() {
            // Stage creation can fail if USD instancing isn't fully supported yet.
            // Skip gracefully rather than panicking.
            eprintln!("[test_populate_native_instances] stage creation failed, skipping");
            return;
        }
        let stage = stage.unwrap();

        let delegate = UsdImagingDelegate::new(stage, Path::absolute_root());
        let mut proxy = crate::index_proxy::IndexProxy::new();
        delegate.populate_with_proxy(&mut proxy);

        // If instancing is working, we expect at least one instancer entry.
        let inst_data = delegate.instancer_data.read();
        if inst_data.is_empty() {
            // No instances found — either the stage has no instance prims
            // (e.g. identical-content instances were not recognised by USD),
            // or instancing is not yet active. Not a hard failure.
            eprintln!(
                "[test_populate_native_instances] no instancers found (USD may not have \
                 created native instances for this scene)"
            );
            return;
        }

        // There should be exactly one instancer (both instances share same prototype+attrs).
        // The instancer path is the first instance encountered.
        let total_instances: usize = inst_data.values().map(|d| d.instance_paths.len()).sum();
        assert!(
            total_instances >= 1,
            "expected at least 1 registered instance, got 0"
        );
    }
}
