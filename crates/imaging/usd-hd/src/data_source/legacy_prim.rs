//! HdDataSourceLegacyPrim - Bridge HdSceneDelegate to data source (legacy emulation).
//!
//! Corresponds to pxr/imaging/hd/dataSourceLegacyPrim.h/cpp.
//! This is the central piece of legacy emulation: it wraps an HdSceneDelegate
//! and presents its data as schema-conforming container data sources.

use super::base::HdDataSourceBase;
use super::retained::{HdRetainedContainerDataSource, HdRetainedSampledDataSource};
use super::sampled::{HdSampledDataSource, HdSampledDataSourceTime};
use super::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBaseHandle,
    HdDataSourceLocatorSet,
};
use crate::data_source::locator::HdDataSourceLocator;
use crate::enums::{HdCullStyle, HdInterpolation};
use crate::prim::HdSceneDelegate;
use crate::tokens::{hd_prim_type_is_gprim, hd_prim_type_is_light};
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

// ---------------------------------------------------------------------------
// Token modules
// ---------------------------------------------------------------------------

/// Legacy prim type tokens.
pub mod tokens {
    use super::*;
    /// Token for OpenVDB volume field asset type.
    pub fn openvdb_asset() -> Token {
        Token::new("openvdbAsset")
    }
    /// Token for Field3D volume field asset type.
    pub fn field3d_asset() -> Token {
        Token::new("field3dAsset")
    }
}

/// Legacy flag tokens.
pub mod flag_tokens {
    use super::*;
    /// Flag token indicating a legacy instancer prim.
    pub fn is_legacy_instancer() -> Token {
        Token::new("isLegacyInstancer")
    }
}

/// Returns true if `prim_type` is a volume field type (openvdbAsset, field3dAsset).
pub fn hd_legacy_prim_type_is_volume_field(prim_type: &Token) -> bool {
    let s = prim_type.as_str();
    s == "openvdbAsset" || s == "field3dAsset"
}

// ---------------------------------------------------------------------------
// Schema token constants (mirrors C++ HdFooSchemaTokens->foo)
// ---------------------------------------------------------------------------

static TOK_MESH: Lazy<Token> = Lazy::new(|| Token::new("mesh"));
static TOK_BASIS_CURVES: Lazy<Token> = Lazy::new(|| Token::new("basisCurves"));
static TOK_PRIMVARS: Lazy<Token> = Lazy::new(|| Token::new("primvars"));
static TOK_EXT_COMP_PRIMVARS: Lazy<Token> = Lazy::new(|| Token::new("extComputationPrimvars"));
static TOK_MATERIAL_BINDINGS: Lazy<Token> = Lazy::new(|| Token::new("materialBindings"));
static TOK_DISPLAY_STYLE: Lazy<Token> = Lazy::new(|| Token::new("displayStyle"));
static TOK_COORD_SYS_BINDING: Lazy<Token> = Lazy::new(|| Token::new("coordSysBinding"));
static TOK_PURPOSE: Lazy<Token> = Lazy::new(|| Token::new("purpose"));
static TOK_VISIBILITY: Lazy<Token> = Lazy::new(|| Token::new("visibility"));
static TOK_CATEGORIES: Lazy<Token> = Lazy::new(|| Token::new("categories"));
static TOK_XFORM: Lazy<Token> = Lazy::new(|| Token::new("xform"));
static TOK_EXTENT: Lazy<Token> = Lazy::new(|| Token::new("extent"));
static TOK_MATERIAL: Lazy<Token> = Lazy::new(|| Token::new("material"));
static TOK_LIGHT: Lazy<Token> = Lazy::new(|| Token::new("light"));
static TOK_COLLECTIONS: Lazy<Token> = Lazy::new(|| Token::new("collections"));
static TOK_DEPENDENCIES: Lazy<Token> = Lazy::new(|| Token::new("__dependencies"));
static TOK_INSTANCED_BY: Lazy<Token> = Lazy::new(|| Token::new("instancedBy"));
static TOK_INSTANCER_TOPOLOGY: Lazy<Token> = Lazy::new(|| Token::new("instancerTopology"));
static TOK_INSTANCE_CATEGORIES: Lazy<Token> = Lazy::new(|| Token::new("instanceCategories"));
static TOK_CAMERA: Lazy<Token> = Lazy::new(|| Token::new("camera"));
static TOK_RENDER_BUFFER: Lazy<Token> = Lazy::new(|| Token::new("renderBuffer"));
static TOK_RENDER_SETTINGS: Lazy<Token> = Lazy::new(|| Token::new("renderSettings"));
static TOK_INTEGRATOR: Lazy<Token> = Lazy::new(|| Token::new("integrator"));
static TOK_SAMPLE_FILTER: Lazy<Token> = Lazy::new(|| Token::new("sampleFilter"));
static TOK_DISPLAY_FILTER: Lazy<Token> = Lazy::new(|| Token::new("displayFilter"));
static TOK_VOLUME_FIELD: Lazy<Token> = Lazy::new(|| Token::new("volumeField"));
static TOK_VOLUME_FIELD_BINDING: Lazy<Token> = Lazy::new(|| Token::new("volumeFieldBinding"));
static TOK_EXT_COMPUTATION: Lazy<Token> = Lazy::new(|| Token::new("extComputation"));
static TOK_DRAW_TARGET: Lazy<Token> = Lazy::new(|| Token::new("drawTarget"));
static TOK_IMAGE_SHADER: Lazy<Token> = Lazy::new(|| Token::new("imageShader"));
pub(crate) static TOK_SCENE_DELEGATE: Lazy<Token> = Lazy::new(|| Token::new("sceneDelegate"));

// Prim type tokens
static TYPE_MESH: Lazy<Token> = Lazy::new(|| Token::new("mesh"));
static TYPE_BASIS_CURVES: Lazy<Token> = Lazy::new(|| Token::new("basisCurves"));
static TYPE_MATERIAL: Lazy<Token> = Lazy::new(|| Token::new("material"));
static TYPE_INSTANCER: Lazy<Token> = Lazy::new(|| Token::new("instancer"));
static TYPE_CAMERA: Lazy<Token> = Lazy::new(|| Token::new("camera"));
static TYPE_RENDER_BUFFER: Lazy<Token> = Lazy::new(|| Token::new("renderBuffer"));
static TYPE_RENDER_SETTINGS: Lazy<Token> = Lazy::new(|| Token::new("renderSettings"));
static TYPE_INTEGRATOR: Lazy<Token> = Lazy::new(|| Token::new("integrator"));
static TYPE_SAMPLE_FILTER: Lazy<Token> = Lazy::new(|| Token::new("sampleFilter"));
static TYPE_DISPLAY_FILTER: Lazy<Token> = Lazy::new(|| Token::new("displayFilter"));
static TYPE_VOLUME: Lazy<Token> = Lazy::new(|| Token::new("volume"));
static TYPE_EXT_COMPUTATION: Lazy<Token> = Lazy::new(|| Token::new("extComputation"));
static TYPE_COORD_SYS: Lazy<Token> = Lazy::new(|| Token::new("coordSys"));
static TYPE_DRAW_TARGET: Lazy<Token> = Lazy::new(|| Token::new("drawTarget"));
static TYPE_IMAGE_SHADER: Lazy<Token> = Lazy::new(|| Token::new("imageShader"));
static TYPE_LIGHT_FILTER: Lazy<Token> = Lazy::new(|| Token::new("lightFilter"));

// Prevent runaway recursion in Get() when a delegate path loops back into the
// same (prim, token) lookup through scene-index emulation layers.
std::thread_local! {
    static LEGACY_PRIM_GET_REENTRY: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

struct LegacyPrimGetReentryGuard {
    key: String,
}

impl LegacyPrimGetReentryGuard {
    fn enter(id: &SdfPath, name: &str) -> Option<Self> {
        let key = format!("{}::{}", id, name);
        let inserted =
            LEGACY_PRIM_GET_REENTRY.with(|active| active.borrow_mut().insert(key.clone()));
        if inserted { Some(Self { key }) } else { None }
    }
}

impl Drop for LegacyPrimGetReentryGuard {
    fn drop(&mut self) {
        LEGACY_PRIM_GET_REENTRY.with(|active| {
            active.borrow_mut().remove(&self.key);
        });
    }
}

/// Marker data source carrying the originating legacy scene delegate.
///
/// `_ref` stores `HdSceneDelegate*` in the `sceneDelegate` data source so the
/// scene-index adapter can forward `Sync()` / `PostSyncCleanup()` to the
/// underlying delegates. The earlier Rust port downgraded this to a boolean
/// marker, which destroyed that contract.
#[derive(Clone)]
pub(crate) struct LegacySceneDelegateDs {
    scene_delegate: Arc<dyn HdSceneDelegate + Send + Sync>,
}

impl LegacySceneDelegateDs {
    pub(crate) fn new(
        scene_delegate: Arc<dyn HdSceneDelegate + Send + Sync>,
    ) -> HdDataSourceBaseHandle {
        Arc::new(Self { scene_delegate })
    }

    pub(crate) fn get_scene_delegate(&self) -> Arc<dyn HdSceneDelegate + Send + Sync> {
        self.scene_delegate.clone()
    }
}

impl fmt::Debug for LegacySceneDelegateDs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacySceneDelegateDs").finish()
    }
}

impl HdDataSourceBase for LegacySceneDelegateDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<Value> {
        Some(Value::from(true))
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for LegacySceneDelegateDs {
    fn get_value(&self, _shutter_offset: HdSampledDataSourceTime) -> Value {
        Value::from(true)
    }

    fn get_contributing_sample_times(
        &self,
        _start_time: HdSampledDataSourceTime,
        _end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        out_sample_times.clear();
        false
    }
}

/// Recover the legacy scene delegate carried by a `sceneDelegate` data source.
pub(crate) fn extract_scene_delegate_handle(
    ds: &HdDataSourceBaseHandle,
) -> Option<Arc<dyn HdSceneDelegate + Send + Sync>> {
    ds.as_any()
        .downcast_ref::<LegacySceneDelegateDs>()
        .map(LegacySceneDelegateDs::get_scene_delegate)
}

// ---------------------------------------------------------------------------
// Inner data sources that wrap delegate calls
// ---------------------------------------------------------------------------

/// Xform data source wrapping HdSceneDelegate::GetTransform.
struct LegacyXformDs {
    id: SdfPath,
    scene_delegate: Arc<dyn HdSceneDelegate + Send + Sync>,
}

impl LegacyXformDs {
    fn new(id: SdfPath, sd: Arc<dyn HdSceneDelegate + Send + Sync>) -> Arc<Self> {
        Arc::new(Self {
            id,
            scene_delegate: sd,
        })
    }
}

impl fmt::Debug for LegacyXformDs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyXformDs")
            .field("id", &self.id)
            .finish()
    }
}

impl HdDataSourceBase for LegacyXformDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        }))
    }
}

impl HdContainerDataSource for LegacyXformDs {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("matrix"), Token::new("resetXformStack")]
    }
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        match name.as_str() {
            "matrix" => {
                let m = self.scene_delegate.get_transform(&self.id);
                Some(HdRetainedSampledDataSource::new(Value::from(m)))
            }
            "resetXformStack" => Some(HdRetainedSampledDataSource::new(Value::from(false))),
            _ => None,
        }
    }
}

/// Visibility data source wrapping HdSceneDelegate::GetVisible.
struct LegacyVisibilityDs {
    id: SdfPath,
    scene_delegate: Arc<dyn HdSceneDelegate + Send + Sync>,
}

impl LegacyVisibilityDs {
    fn new(id: SdfPath, sd: Arc<dyn HdSceneDelegate + Send + Sync>) -> Arc<Self> {
        Arc::new(Self {
            id,
            scene_delegate: sd,
        })
    }
}

impl fmt::Debug for LegacyVisibilityDs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyVisibilityDs")
            .field("id", &self.id)
            .finish()
    }
}

impl HdDataSourceBase for LegacyVisibilityDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        }))
    }
}

impl HdContainerDataSource for LegacyVisibilityDs {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("visibility")]
    }
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == "visibility" {
            let vis = self.scene_delegate.get_visible(&self.id);
            Some(HdRetainedSampledDataSource::new(Value::from(vis)))
        } else {
            None
        }
    }
}

/// Purpose data source wrapping HdSceneDelegate::GetRenderTag.
struct LegacyPurposeDs {
    id: SdfPath,
    scene_delegate: Arc<dyn HdSceneDelegate + Send + Sync>,
}

impl LegacyPurposeDs {
    fn new(id: SdfPath, sd: Arc<dyn HdSceneDelegate + Send + Sync>) -> Arc<Self> {
        Arc::new(Self {
            id,
            scene_delegate: sd,
        })
    }
}

impl fmt::Debug for LegacyPurposeDs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyPurposeDs")
            .field("id", &self.id)
            .finish()
    }
}

impl HdDataSourceBase for LegacyPurposeDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        }))
    }
}

impl HdContainerDataSource for LegacyPurposeDs {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("purpose")]
    }
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == "purpose" {
            // C++: _sceneDelegate->GetRenderTag(_id)
            let purpose = self.scene_delegate.get_render_tag(&self.id);
            Some(HdRetainedSampledDataSource::new(Value::from(
                purpose.as_str().to_string(),
            )))
        } else {
            None
        }
    }
}

/// Extent data source wrapping HdSceneDelegate::GetExtent.
struct LegacyExtentDs {
    id: SdfPath,
    scene_delegate: Arc<dyn HdSceneDelegate + Send + Sync>,
}

impl LegacyExtentDs {
    fn new(id: SdfPath, sd: Arc<dyn HdSceneDelegate + Send + Sync>) -> Arc<Self> {
        Arc::new(Self {
            id,
            scene_delegate: sd,
        })
    }
}

impl fmt::Debug for LegacyExtentDs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyExtentDs")
            .field("id", &self.id)
            .finish()
    }
}

impl HdDataSourceBase for LegacyExtentDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        }))
    }
}

impl HdContainerDataSource for LegacyExtentDs {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("min"), Token::new("max")]
    }
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let extent = self.scene_delegate.get_extent(&self.id);
        match name.as_str() {
            "min" => Some(HdRetainedSampledDataSource::new(Value::from(
                extent.min().clone(),
            ))),
            "max" => Some(HdRetainedSampledDataSource::new(Value::from(
                extent.max().clone(),
            ))),
            _ => None,
        }
    }
}

/// Mesh data source wrapping HdSceneDelegate::GetMeshTopology + GetDoubleSided.
struct LegacyMeshDs {
    id: SdfPath,
    scene_delegate: Arc<dyn HdSceneDelegate + Send + Sync>,
}

impl LegacyMeshDs {
    fn new(id: SdfPath, sd: Arc<dyn HdSceneDelegate + Send + Sync>) -> Arc<Self> {
        Arc::new(Self {
            id,
            scene_delegate: sd,
        })
    }
}

impl fmt::Debug for LegacyMeshDs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyMeshDs")
            .field("id", &self.id)
            .finish()
    }
}

impl HdDataSourceBase for LegacyMeshDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        }))
    }
}

impl HdContainerDataSource for LegacyMeshDs {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("topology"), Token::new("doubleSided")]
    }
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        match name.as_str() {
            "topology" => {
                let topo = self.scene_delegate.get_mesh_topology(&self.id);
                Some(LegacyMeshTopologyDs::new(topo))
            }
            "doubleSided" => {
                let ds = self.scene_delegate.get_double_sided(&self.id);
                Some(HdRetainedSampledDataSource::new(Value::from(ds)))
            }
            _ => None,
        }
    }
}

/// Mesh topology container (faceVertexCounts, faceVertexIndices, orientation).
#[derive(Debug)]
struct LegacyMeshTopologyDs {
    topo: crate::prim::mesh::HdMeshTopology,
}

impl LegacyMeshTopologyDs {
    fn new(topo: crate::prim::mesh::HdMeshTopology) -> Arc<Self> {
        Arc::new(Self { topo })
    }
}

impl HdDataSourceBase for LegacyMeshTopologyDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            topo: self.topo.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            topo: self.topo.clone(),
        }))
    }
}

impl HdContainerDataSource for LegacyMeshTopologyDs {
    fn get_names(&self) -> Vec<Token> {
        // P1-8: include "orientation" and "scheme" as required by HdMeshTopologySchema.
        vec![
            Token::new("faceVertexCounts"),
            Token::new("faceVertexIndices"),
            Token::new("holeIndices"),
            Token::new("orientation"),
            Token::new("scheme"),
        ]
    }
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        match name.as_str() {
            "faceVertexCounts" => Some(HdRetainedSampledDataSource::new(Value::from(
                self.topo.face_vertex_counts.clone(),
            ))),
            "faceVertexIndices" => Some(HdRetainedSampledDataSource::new(Value::from(
                self.topo.face_vertex_indices.clone(),
            ))),
            "holeIndices" => {
                // C++: meshTopology->GetHoleIndices()
                if self.topo.hole_indices.is_empty() {
                    None
                } else {
                    Some(HdRetainedSampledDataSource::new(Value::from(
                        self.topo.hole_indices.clone(),
                    )))
                }
            }
            // P1-8: orientation token (e.g. "rightHanded" or "leftHanded")
            "orientation" => Some(HdRetainedSampledDataSource::new(Value::from(
                self.topo.orientation.as_str().to_string(),
            ))),
            // P1-8: subdivision scheme token (e.g. "none", "catmullClark")
            "scheme" => Some(HdRetainedSampledDataSource::new(Value::from(
                self.topo.scheme.as_str().to_string(),
            ))),
            _ => None,
        }
    }
}

/// Basis curves data source wrapping HdSceneDelegate::GetBasisCurvesTopology.
struct LegacyBasisCurvesDs {
    id: SdfPath,
    scene_delegate: Arc<dyn HdSceneDelegate + Send + Sync>,
}

impl LegacyBasisCurvesDs {
    fn new(id: SdfPath, sd: Arc<dyn HdSceneDelegate + Send + Sync>) -> Arc<Self> {
        Arc::new(Self {
            id,
            scene_delegate: sd,
        })
    }
}

impl fmt::Debug for LegacyBasisCurvesDs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyBasisCurvesDs")
            .field("id", &self.id)
            .finish()
    }
}

impl HdDataSourceBase for LegacyBasisCurvesDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        }))
    }
}

impl HdContainerDataSource for LegacyBasisCurvesDs {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("topology")]
    }
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == "topology" {
            let topo = self.scene_delegate.get_basis_curves_topology(&self.id);
            let mut children = HashMap::new();
            children.insert(
                Token::new("curveVertexCounts"),
                HdRetainedSampledDataSource::new(Value::from(topo.curve_vertex_counts.clone()))
                    as HdDataSourceBaseHandle,
            );
            if let Some(ref ct) = topo.curve_type {
                children.insert(
                    Token::new("type"),
                    HdRetainedSampledDataSource::new(Value::from(ct.as_token_str().to_string()))
                        as HdDataSourceBaseHandle,
                );
            }
            if let Some(ref basis) = topo.basis {
                children.insert(
                    Token::new("basis"),
                    HdRetainedSampledDataSource::new(Value::from(basis.as_token_str().to_string()))
                        as HdDataSourceBaseHandle,
                );
            }
            children.insert(
                Token::new("wrap"),
                HdRetainedSampledDataSource::new(Value::from(topo.wrap.as_token_str().to_string()))
                    as HdDataSourceBaseHandle,
            );
            Some(HdRetainedContainerDataSource::new(children))
        } else {
            None
        }
    }
}

/// Display style data source wrapping HdSceneDelegate::GetDisplayStyle.
struct LegacyDisplayStyleDs {
    id: SdfPath,
    scene_delegate: Arc<dyn HdSceneDelegate + Send + Sync>,
}

impl LegacyDisplayStyleDs {
    fn new(id: SdfPath, sd: Arc<dyn HdSceneDelegate + Send + Sync>) -> Arc<Self> {
        Arc::new(Self {
            id,
            scene_delegate: sd,
        })
    }
}

impl fmt::Debug for LegacyDisplayStyleDs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyDisplayStyleDs")
            .field("id", &self.id)
            .finish()
    }
}

impl HdDataSourceBase for LegacyDisplayStyleDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        }))
    }
}

impl HdContainerDataSource for LegacyDisplayStyleDs {
    fn get_names(&self) -> Vec<Token> {
        // C++ returns all 10 fields from HdLegacyDisplayStyleSchema
        vec![
            Token::new("refineLevel"),
            Token::new("flatShadingEnabled"),
            Token::new("displacementEnabled"),
            Token::new("displayInOverlay"),
            Token::new("occludedSelectionShowsThrough"),
            Token::new("pointsShadingEnabled"),
            Token::new("materialIsFinal"),
            Token::new("shadingStyle"),
            Token::new("reprSelector"),
            Token::new("cullStyle"),
        ]
    }
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        match name.as_str() {
            // Fields from HdDisplayStyle struct (lazy-read in C++)
            "refineLevel" => {
                let style = self.scene_delegate.get_display_style(&self.id);
                // C++: returns nullptr if refineLevel == 0
                if style.refine_level != 0 {
                    Some(HdRetainedSampledDataSource::new(Value::from(
                        style.refine_level as i32,
                    )))
                } else {
                    None
                }
            }
            "flatShadingEnabled" => {
                let style = self.scene_delegate.get_display_style(&self.id);
                Some(HdRetainedSampledDataSource::new(Value::from(
                    style.flat_shading_enabled,
                )))
            }
            "displacementEnabled" => {
                let style = self.scene_delegate.get_display_style(&self.id);
                Some(HdRetainedSampledDataSource::new(Value::from(
                    style.displacement_enabled,
                )))
            }
            "displayInOverlay" => {
                let style = self.scene_delegate.get_display_style(&self.id);
                Some(HdRetainedSampledDataSource::new(Value::from(
                    style.display_in_overlay,
                )))
            }
            "occludedSelectionShowsThrough" => {
                let style = self.scene_delegate.get_display_style(&self.id);
                Some(HdRetainedSampledDataSource::new(Value::from(
                    style.occluded_selection_shows_through,
                )))
            }
            "pointsShadingEnabled" => {
                let style = self.scene_delegate.get_display_style(&self.id);
                Some(HdRetainedSampledDataSource::new(Value::from(
                    style.points_shading_enabled,
                )))
            }
            "materialIsFinal" => {
                let style = self.scene_delegate.get_display_style(&self.id);
                Some(HdRetainedSampledDataSource::new(Value::from(
                    style.material_is_final,
                )))
            }
            // shadingStyle: from separate delegate call GetShadingStyle
            "shadingStyle" => {
                let val = self.scene_delegate.get_shading_style(&self.id);
                // C++: returns nullptr if token is empty
                if val.is_empty() {
                    None
                } else {
                    Some(HdRetainedSampledDataSource::new(val))
                }
            }
            // reprSelector: from GetReprSelector, returns token array
            "reprSelector" => {
                use crate::prim::HdReprSelector;
                let repr = self.scene_delegate.get_repr_selector(&self.id);
                // C++: check if any repr is non-empty
                let any_active =
                    (0..HdReprSelector::MAX_TOPOLOGY_REPRS).any(|i| repr.is_active_repr(i));
                if any_active {
                    let tokens: Vec<String> = (0..HdReprSelector::MAX_TOPOLOGY_REPRS)
                        .map(|i| repr.get_token(i).as_str().to_string())
                        .collect();
                    Some(HdRetainedSampledDataSource::new(Value::from(tokens)))
                } else {
                    None
                }
            }
            // cullStyle: from GetCullStyle, returns token
            "cullStyle" => {
                let cull = self.scene_delegate.get_cull_style(&self.id);
                if cull == HdCullStyle::DontCare {
                    return None;
                }
                let tok = match cull {
                    HdCullStyle::Nothing => "nothing",
                    HdCullStyle::Back => "back",
                    HdCullStyle::Front => "front",
                    HdCullStyle::BackUnlessDoubleSided => "backUnlessDoubleSided",
                    HdCullStyle::FrontUnlessDoubleSided => "frontUnlessDoubleSided",
                    _ => "dontCare",
                };
                Some(HdRetainedSampledDataSource::new(Value::from(
                    tok.to_string(),
                )))
            }
            _ => None,
        }
    }
}

/// Material bindings data source wrapping HdSceneDelegate::GetMaterialId.
struct LegacyMaterialBindingsDs {
    id: SdfPath,
    scene_delegate: Arc<dyn HdSceneDelegate + Send + Sync>,
}

impl LegacyMaterialBindingsDs {
    fn new(id: SdfPath, sd: Arc<dyn HdSceneDelegate + Send + Sync>) -> Arc<Self> {
        Arc::new(Self {
            id,
            scene_delegate: sd,
        })
    }
}

impl fmt::Debug for LegacyMaterialBindingsDs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyMaterialBindingsDs")
            .field("id", &self.id)
            .finish()
    }
}

impl HdDataSourceBase for LegacyMaterialBindingsDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        }))
    }
}

impl HdContainerDataSource for LegacyMaterialBindingsDs {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("")]
    }
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name.as_str().is_empty() {
            if let Some(mat_id) = self.scene_delegate.get_material_id(&self.id) {
                let mut binding = HashMap::new();
                binding.insert(
                    Token::new("path"),
                    HdRetainedSampledDataSource::new(Value::from(mat_id.to_string()))
                        as HdDataSourceBaseHandle,
                );
                Some(HdRetainedContainerDataSource::new(binding))
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Instanced-by data source (for instanceable prims).
struct LegacyInstancedByDs {
    id: SdfPath,
    scene_delegate: Arc<dyn HdSceneDelegate + Send + Sync>,
}

impl LegacyInstancedByDs {
    fn new(id: SdfPath, sd: Arc<dyn HdSceneDelegate + Send + Sync>) -> Arc<Self> {
        Arc::new(Self {
            id,
            scene_delegate: sd,
        })
    }
}

impl fmt::Debug for LegacyInstancedByDs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyInstancedByDs")
            .field("id", &self.id)
            .finish()
    }
}

impl HdDataSourceBase for LegacyInstancedByDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        }))
    }
}

impl HdContainerDataSource for LegacyInstancedByDs {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("paths"), Token::new("prototypeRoots")]
    }
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        match name.as_str() {
            "paths" => {
                let instancer_id = self.scene_delegate.get_instancer_id(&self.id);
                if !instancer_id.is_empty() {
                    Some(HdRetainedSampledDataSource::new(Value::from(vec![
                        instancer_id.to_string(),
                    ])))
                } else {
                    None
                }
            }
            // P1-9: return empty array for prototypeRoots (field exists but is empty)
            // rather than None (which would signal "field doesn't exist").
            // Prototype root computation: C++ HdSceneIndexAdapterSceneDelegate
            // calls GetScenePrimPath(rprimId, instancerIndex) to reroute the
            // instancer path to the prototype sub-tree root. This requires
            // per-prim HdInstancerContext support which is not yet wired in;
            // wire up when instancing beyond point instancers is needed.
            "prototypeRoots" => Some(HdRetainedSampledDataSource::new(Value::from(
                Vec::<String>::new(),
            ))),
            _ => None,
        }
    }
}

/// Categories data source wrapping HdSceneDelegate::GetCategories.
struct LegacyCategoriesDs {
    id: SdfPath,
    scene_delegate: Arc<dyn HdSceneDelegate + Send + Sync>,
}

impl LegacyCategoriesDs {
    fn new(id: SdfPath, sd: Arc<dyn HdSceneDelegate + Send + Sync>) -> Arc<Self> {
        Arc::new(Self {
            id,
            scene_delegate: sd,
        })
    }
}

impl fmt::Debug for LegacyCategoriesDs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyCategoriesDs")
            .field("id", &self.id)
            .finish()
    }
}

impl HdDataSourceBase for LegacyCategoriesDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            id: self.id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        }))
    }
}

impl HdContainerDataSource for LegacyCategoriesDs {
    fn get_names(&self) -> Vec<Token> {
        self.scene_delegate.get_categories(&self.id)
    }
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let cats = self.scene_delegate.get_categories(&self.id);
        if cats.contains(name) {
            Some(HdRetainedSampledDataSource::new(Value::from(true)))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Primvar value data sources (lazy delegate access)
// ---------------------------------------------------------------------------

/// Lazy primvar value data source: calls delegate.get(id, name) on access.
/// C++ equivalent: Hd_DataSourceLegacyPrimvarValue.
struct LegacyPrimvarValueDs {
    primvar_name: Token,
    prim_id: SdfPath,
    scene_delegate: Arc<dyn HdSceneDelegate + Send + Sync>,
}

impl LegacyPrimvarValueDs {
    fn new(
        primvar_name: Token,
        prim_id: SdfPath,
        sd: Arc<dyn HdSceneDelegate + Send + Sync>,
    ) -> Arc<Self> {
        Arc::new(Self {
            primvar_name,
            prim_id,
            scene_delegate: sd,
        })
    }
}

impl fmt::Debug for LegacyPrimvarValueDs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyPrimvarValueDs")
            .field("primvar_name", &self.primvar_name)
            .field("prim_id", &self.prim_id)
            .finish()
    }
}

impl HdDataSourceBase for LegacyPrimvarValueDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            primvar_name: self.primvar_name.clone(),
            prim_id: self.prim_id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            primvar_name: self.primvar_name.clone(),
            prim_id: self.prim_id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        }))
    }
}

impl HdContainerDataSource for LegacyPrimvarValueDs {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("value")]
    }
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == "value" {
            let val = self.scene_delegate.get(&self.prim_id, &self.primvar_name);
            if val.is_empty() {
                // Fallback: try SamplePrimvar (for lights without primvars: namespace)
                let samples =
                    self.scene_delegate
                        .sample_primvar(&self.prim_id, &self.primvar_name, 1);
                if let Some((_, v)) = samples.into_iter().next() {
                    if !v.is_empty() {
                        return Some(HdRetainedSampledDataSource::new(v));
                    }
                }
                None
            } else {
                Some(HdRetainedSampledDataSource::new(val))
            }
        } else {
            None
        }
    }
}

/// Lazy indexed primvar value data source: calls delegate.get_indexed_primvar.
struct LegacyIndexedPrimvarValueDs {
    primvar_name: Token,
    prim_id: SdfPath,
    scene_delegate: Arc<dyn HdSceneDelegate + Send + Sync>,
}

impl LegacyIndexedPrimvarValueDs {
    fn new(
        primvar_name: Token,
        prim_id: SdfPath,
        sd: Arc<dyn HdSceneDelegate + Send + Sync>,
    ) -> Arc<Self> {
        Arc::new(Self {
            primvar_name,
            prim_id,
            scene_delegate: sd,
        })
    }
}

impl fmt::Debug for LegacyIndexedPrimvarValueDs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyIndexedPrimvarValueDs")
            .field("primvar_name", &self.primvar_name)
            .field("prim_id", &self.prim_id)
            .finish()
    }
}

impl HdDataSourceBase for LegacyIndexedPrimvarValueDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            primvar_name: self.primvar_name.clone(),
            prim_id: self.prim_id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            primvar_name: self.primvar_name.clone(),
            prim_id: self.prim_id.clone(),
            scene_delegate: self.scene_delegate.clone(),
        }))
    }
}

impl HdContainerDataSource for LegacyIndexedPrimvarValueDs {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("value"), Token::new("indices")]
    }
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let (val, indices) = self
            .scene_delegate
            .get_indexed_primvar(&self.prim_id, &self.primvar_name);
        match name.as_str() {
            "value" => {
                if val.is_empty() {
                    None
                } else {
                    Some(HdRetainedSampledDataSource::new(val))
                }
            }
            "indices" => indices.map(|idx| {
                HdRetainedSampledDataSource::new(Value::from(idx)) as HdDataSourceBaseHandle
            }),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Cached locator set for PrimDirtied
// ---------------------------------------------------------------------------

static CACHED_LOCATORS: Lazy<HdDataSourceLocatorSet> = Lazy::new(|| {
    let mut set = HdDataSourceLocatorSet::new();
    set.insert(HdDataSourceLocator::new(&[Token::new("primvars")]));
    set.insert(HdDataSourceLocator::new(&[Token::new("instancerTopology")]));
    set
});

// ---------------------------------------------------------------------------
// HdDataSourceLegacyPrim
// ---------------------------------------------------------------------------

/// Container data source that adapts HdSceneDelegate into schema form.
///
/// Used during legacy scene delegate emulation. For each prim, instantiated
/// by HdLegacyPrimSceneIndex::AddLegacyPrim. Its Get() method dispatches to
/// inner data sources that call through to the delegate.
///
/// Corresponds to C++ `HdDataSourceLegacyPrim`.
pub struct HdDataSourceLegacyPrim {
    id: SdfPath,
    prim_type: Token,
    scene_delegate: Option<Arc<dyn HdSceneDelegate + Send + Sync>>,
    // Cached primvars data source (atomically guarded)
    primvars_built: AtomicBool,
    primvars: Mutex<Option<HdDataSourceBaseHandle>>,
    // Cached instancer topology
    instancer_topology: Mutex<Option<HdDataSourceBaseHandle>>,
}

impl HdDataSourceLegacyPrim {
    /// Create new legacy prim data source.
    pub fn new(
        id: SdfPath,
        prim_type: Token,
        scene_delegate: Option<Arc<dyn HdSceneDelegate + Send + Sync>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            id,
            prim_type,
            scene_delegate,
            primvars_built: AtomicBool::new(false),
            primvars: Mutex::new(None),
            instancer_topology: Mutex::new(None),
        })
    }

    /// Clear cached values in response to DirtyPrims. Called by
    /// HdLegacyPrimSceneIndex::DirtyPrims.
    pub fn prim_dirtied(&self, locators: &HdDataSourceLocatorSet) {
        let primvars_loc = HdDataSourceLocator::new(&[Token::new("primvars")]);
        if locators.intersects_locator(&primvars_loc) {
            let mut guard = self.primvars.lock().unwrap();
            *guard = None;
            self.primvars_built.store(false, Ordering::Release);
        }

        let topo_loc = HdDataSourceLocator::new(&[Token::new("instancerTopology")]);
        if locators.intersects_locator(&topo_loc) {
            let mut guard = self.instancer_topology.lock().unwrap();
            *guard = None;
        }
    }

    /// Get locators that PrimDirtied responds to.
    pub fn get_cached_locators() -> &'static HdDataSourceLocatorSet {
        &CACHED_LOCATORS
    }

    /// Is this prim a light?
    fn is_light(&self) -> bool {
        hd_prim_type_is_light(&self.prim_type)
    }

    /// Is this prim instanceable? (gprim, light, or instancer)
    fn is_instanceable(&self) -> bool {
        hd_prim_type_is_gprim(&self.prim_type)
            || self.is_light()
            || self.prim_type == *TYPE_INSTANCER
    }

    /// Build primvars data source (cached, thread-safe).
    /// Iterates all 6 interpolation modes via GetPrimvarDescriptors.
    fn get_primvars_ds(&self) -> Option<HdDataSourceBaseHandle> {
        if self.primvars_built.load(Ordering::Acquire) {
            return self.primvars.lock().unwrap().clone();
        }
        let mut guard = self.primvars.lock().unwrap();
        if self.primvars_built.load(Ordering::Acquire) {
            return guard.clone();
        }

        let sd = match self.scene_delegate.as_ref() {
            Some(sd) => sd,
            None => {
                let ds: HdDataSourceBaseHandle = HdRetainedContainerDataSource::new_empty();
                *guard = Some(ds.clone());
                self.primvars_built.store(true, Ordering::Release);
                return Some(ds);
            }
        };

        // All interpolation modes to query
        const ALL_INTERPS: [HdInterpolation; 6] = [
            HdInterpolation::Constant,
            HdInterpolation::Uniform,
            HdInterpolation::Varying,
            HdInterpolation::Vertex,
            HdInterpolation::FaceVarying,
            HdInterpolation::Instance,
        ];

        let mut primvar_children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();

        for &interp in &ALL_INTERPS {
            let descriptors = sd.get_primvar_descriptors(&self.id, interp);
            let interp_token = Token::new(interp.as_str());

            for desc in &descriptors {
                // Build a per-primvar container with {primvarValue, interpolation, role}
                let mut pv_fields: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();

                // Lazy primvar value: calls delegate.get(id, name) at access time
                let pv_val_ds =
                    LegacyPrimvarValueDs::new(desc.name.clone(), self.id.clone(), sd.clone());
                pv_fields.insert(
                    Token::new("primvarValue"),
                    pv_val_ds as HdDataSourceBaseHandle,
                );
                pv_fields.insert(
                    Token::new("interpolation"),
                    HdRetainedSampledDataSource::new(Value::from(interp_token.as_str().to_string()))
                        as HdDataSourceBaseHandle,
                );
                if !desc.role.is_empty() {
                    pv_fields.insert(
                        Token::new("role"),
                        HdRetainedSampledDataSource::new(Value::from(
                            desc.role.as_str().to_string(),
                        )) as HdDataSourceBaseHandle,
                    );
                }
                if desc.indexed {
                    // Indexed primvar: add indexedPrimvarValue marker
                    let idx_ds = LegacyIndexedPrimvarValueDs::new(
                        desc.name.clone(),
                        self.id.clone(),
                        sd.clone(),
                    );
                    pv_fields.insert(
                        Token::new("indexedPrimvarValue"),
                        idx_ds as HdDataSourceBaseHandle,
                    );
                }

                primvar_children.insert(
                    desc.name.clone(),
                    HdRetainedContainerDataSource::new(pv_fields),
                );
            }
        }

        let ds: HdDataSourceBaseHandle = if primvar_children.is_empty() {
            HdRetainedContainerDataSource::new_empty()
        } else {
            HdRetainedContainerDataSource::new(primvar_children)
        };
        *guard = Some(ds.clone());
        self.primvars_built.store(true, Ordering::Release);
        Some(ds)
    }

    fn get_xform_ds(&self) -> Option<HdDataSourceBaseHandle> {
        self.scene_delegate
            .as_ref()
            .map(|sd| LegacyXformDs::new(self.id.clone(), sd.clone()) as HdDataSourceBaseHandle)
    }

    fn get_visibility_ds(&self) -> Option<HdDataSourceBaseHandle> {
        self.scene_delegate.as_ref().map(|sd| {
            LegacyVisibilityDs::new(self.id.clone(), sd.clone()) as HdDataSourceBaseHandle
        })
    }

    fn get_purpose_ds(&self) -> Option<HdDataSourceBaseHandle> {
        self.scene_delegate
            .as_ref()
            .map(|sd| LegacyPurposeDs::new(self.id.clone(), sd.clone()) as HdDataSourceBaseHandle)
    }

    fn get_extent_ds(&self) -> Option<HdDataSourceBaseHandle> {
        self.scene_delegate
            .as_ref()
            .map(|sd| LegacyExtentDs::new(self.id.clone(), sd.clone()) as HdDataSourceBaseHandle)
    }

    fn get_material_bindings_ds(&self) -> Option<HdDataSourceBaseHandle> {
        self.scene_delegate.as_ref().map(|sd| {
            LegacyMaterialBindingsDs::new(self.id.clone(), sd.clone()) as HdDataSourceBaseHandle
        })
    }

    fn get_display_style_ds(&self) -> Option<HdDataSourceBaseHandle> {
        self.scene_delegate.as_ref().map(|sd| {
            LegacyDisplayStyleDs::new(self.id.clone(), sd.clone()) as HdDataSourceBaseHandle
        })
    }

    fn get_instanced_by_ds(&self) -> Option<HdDataSourceBaseHandle> {
        self.scene_delegate.as_ref().map(|sd| {
            LegacyInstancedByDs::new(self.id.clone(), sd.clone()) as HdDataSourceBaseHandle
        })
    }

    fn get_categories_ds(&self) -> Option<HdDataSourceBaseHandle> {
        self.scene_delegate.as_ref().map(|sd| {
            LegacyCategoriesDs::new(self.id.clone(), sd.clone()) as HdDataSourceBaseHandle
        })
    }

    // ----------------------------------------------------------------------- //
    // Fix 5: Additional data source getters for Get() dispatch
    // ----------------------------------------------------------------------- //

    /// Camera data source: wraps GetCameraParamValue.
    /// C++: Hd_DataSourceCamera - returns container with camera params.
    fn get_camera_ds(&self) -> Option<HdDataSourceBaseHandle> {
        let sd = self.scene_delegate.as_ref()?;
        // Return a generic container that reads camera params on demand
        let mut children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
        // Standard camera params: projection, focalLength, horizontalAperture, etc.
        for param_name in &[
            "projection",
            "focalLength",
            "horizontalAperture",
            "verticalAperture",
            "horizontalApertureOffset",
            "verticalApertureOffset",
            "clippingRange",
            "shutterOpen",
            "shutterClose",
        ] {
            let tok = Token::new(param_name);
            let val = sd.get_camera_param_value(&self.id, &tok);
            if !val.is_empty() {
                children.insert(
                    tok,
                    HdRetainedSampledDataSource::new(val) as HdDataSourceBaseHandle,
                );
            }
        }
        Some(HdRetainedContainerDataSource::new(children))
    }

    /// Light data source: wraps GetLightParamValue.
    /// C++: Hd_DataSourceLight.
    fn get_light_ds(&self) -> Option<HdDataSourceBaseHandle> {
        let sd = self.scene_delegate.as_ref()?;
        let mut children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
        // Standard light params
        for param_name in &[
            "color",
            "intensity",
            "exposure",
            "diffuse",
            "specular",
            "normalize",
            "colorTemperature",
            "enableColorTemperature",
            "shapingFocus",
            "shapingFocusTint",
            "shapingConeAngle",
            "shapingConeSoftness",
        ] {
            let tok = Token::new(param_name);
            let val = sd.get_light_param_value(&self.id, &tok);
            if !val.is_empty() {
                children.insert(
                    tok,
                    HdRetainedSampledDataSource::new(val) as HdDataSourceBaseHandle,
                );
            }
        }
        Some(HdRetainedContainerDataSource::new(children))
    }

    /// Material data source: wraps GetMaterialResource.
    /// C++: calls _sceneDelegate->GetMaterialResource(_id) and converts.
    fn get_material_ds(&self) -> Option<HdDataSourceBaseHandle> {
        let sd = self.scene_delegate.as_ref()?;
        let mat_val = sd.get_material_resource(&self.id);
        if mat_val.is_empty() {
            return None;
        }
        // Wrap the material resource value in a container
        let mut children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
        children.insert(
            Token::new("resource"),
            HdRetainedSampledDataSource::new(mat_val) as HdDataSourceBaseHandle,
        );
        Some(HdRetainedContainerDataSource::new(children))
    }

    /// Instancer topology data source (cached).
    /// C++: Hd_InstancerTopologyDataSource.
    fn get_instancer_topology_ds(&self) -> Option<HdDataSourceBaseHandle> {
        // Check cache first
        {
            let guard = self.instancer_topology.lock().unwrap();
            if let Some(ref ds) = *guard {
                return Some(ds.clone());
            }
        }
        let sd = self.scene_delegate.as_ref()?;
        let mut children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
        // instanceIndices, prototypes, mask, etc.
        // Simplified: store prototype paths and instance indices
        let instancer_id = sd.get_instancer_id(&self.id);
        if !instancer_id.is_empty() {
            children.insert(
                Token::new("instancerTopologyParentId"),
                HdRetainedSampledDataSource::new(Value::from(instancer_id.to_string()))
                    as HdDataSourceBaseHandle,
            );
        }
        let ds: HdDataSourceBaseHandle = HdRetainedContainerDataSource::new(children);
        let mut guard = self.instancer_topology.lock().unwrap();
        *guard = Some(ds.clone());
        Some(ds)
    }

    /// Coord sys binding data source.
    /// C++: _GetCoordSysBindingDataSource builds name->path pairs.
    fn get_coord_sys_binding_ds(&self) -> Option<HdDataSourceBaseHandle> {
        let sd = self.scene_delegate.as_ref()?;
        let bindings = sd.get_coord_sys_bindings(&self.id)?;
        if bindings.is_empty() {
            return None;
        }
        let mut children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
        for path in bindings.iter() {
            // Extract name from path: /path/to/object.coordSys:foo:binding -> foo
            let name_str = path.get_name();
            // Strip ":binding" suffix if present
            let name = if let Some(stripped) = name_str.strip_suffix(":binding") {
                stripped
            } else {
                &name_str
            };
            // Strip "coordSys:" prefix if present
            let name = if let Some(stripped) = name.strip_prefix("coordSys:") {
                stripped
            } else {
                name
            };
            children.insert(
                Token::new(name),
                HdRetainedSampledDataSource::new(Value::from(path.to_string()))
                    as HdDataSourceBaseHandle,
            );
        }
        Some(HdRetainedContainerDataSource::new(children))
    }

    /// Volume field binding data source.
    /// C++: _GetVolumeFieldBindingDataSource.
    fn get_volume_field_binding_ds(&self) -> Option<HdDataSourceBaseHandle> {
        let sd = self.scene_delegate.as_ref()?;
        let fields = sd.get_volume_field_descriptors(&self.id);
        if fields.is_empty() {
            return None;
        }
        let mut children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
        for desc in &fields {
            children.insert(
                desc.field_name.clone(),
                HdRetainedSampledDataSource::new(Value::from(desc.field_id.to_string()))
                    as HdDataSourceBaseHandle,
            );
        }
        Some(HdRetainedContainerDataSource::new(children))
    }

    /// Volume field data source (for volume field prims).
    /// C++: Hd_DataSourceVolumeField.
    fn get_volume_field_ds(&self) -> Option<HdDataSourceBaseHandle> {
        let sd = self.scene_delegate.as_ref()?;
        let mut children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
        // Retrieve generic volume field params
        for key in &[
            "filePath",
            "fieldName",
            "fieldIndex",
            "fieldDataType",
            "vectorDataRoleHint",
        ] {
            let tok = Token::new(key);
            let val = sd.get(&self.id, &tok);
            if !val.is_empty() {
                children.insert(
                    tok,
                    HdRetainedSampledDataSource::new(val) as HdDataSourceBaseHandle,
                );
            }
        }
        Some(HdRetainedContainerDataSource::new(children))
    }

    /// Ext computation data source.
    /// C++: Hd_DataSourceLegacyExtComputation.
    fn get_ext_computation_ds(&self) -> Option<HdDataSourceBaseHandle> {
        let sd = self.scene_delegate.as_ref()?;
        let mut children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();

        // Input names
        let input_names = sd.get_ext_computation_scene_input_names(&self.id);
        if !input_names.is_empty() {
            let names_strs: Vec<String> =
                input_names.iter().map(|t| t.as_str().to_string()).collect();
            children.insert(
                Token::new("inputNames"),
                HdRetainedSampledDataSource::new(Value::from(names_strs)) as HdDataSourceBaseHandle,
            );
        }

        // Input descriptors
        let input_descs = sd.get_ext_computation_input_descriptors(&self.id);
        if !input_descs.is_empty() {
            let mut input_children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
            for desc in &input_descs {
                let mut desc_fields: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
                desc_fields.insert(
                    Token::new("name"),
                    HdRetainedSampledDataSource::new(Value::from(desc.name.as_str().to_string()))
                        as HdDataSourceBaseHandle,
                );
                desc_fields.insert(
                    Token::new("sourceComputation"),
                    HdRetainedSampledDataSource::new(Value::from(
                        desc.source_computation_id.to_string(),
                    )) as HdDataSourceBaseHandle,
                );
                desc_fields.insert(
                    Token::new("sourceComputationOutputName"),
                    HdRetainedSampledDataSource::new(Value::from(
                        desc.source_computation_output_name.as_str().to_string(),
                    )) as HdDataSourceBaseHandle,
                );
                input_children.insert(
                    desc.name.clone(),
                    HdRetainedContainerDataSource::new(desc_fields),
                );
            }
            children.insert(
                Token::new("inputComputations"),
                HdRetainedContainerDataSource::new(input_children),
            );
        }

        // Output descriptors
        let output_descs = sd.get_ext_computation_output_descriptors(&self.id);
        if !output_descs.is_empty() {
            let mut output_children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
            for desc in &output_descs {
                let mut desc_fields: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
                desc_fields.insert(
                    Token::new("name"),
                    HdRetainedSampledDataSource::new(Value::from(desc.name.as_str().to_string()))
                        as HdDataSourceBaseHandle,
                );
                output_children.insert(
                    desc.name.clone(),
                    HdRetainedContainerDataSource::new(desc_fields),
                );
            }
            children.insert(
                Token::new("outputs"),
                HdRetainedContainerDataSource::new(output_children),
            );
        }

        // Kernel source
        let kernel = sd.get_ext_computation_kernel(&self.id);
        if !kernel.is_empty() {
            children.insert(
                Token::new("glslKernel"),
                HdRetainedSampledDataSource::new(Value::from(kernel)) as HdDataSourceBaseHandle,
            );
        }

        Some(HdRetainedContainerDataSource::new(children))
    }

    /// Ext computation primvars data source.
    fn get_ext_computation_primvars_ds(&self) -> Option<HdDataSourceBaseHandle> {
        let sd = self.scene_delegate.as_ref()?;

        const ALL_INTERPS: [HdInterpolation; 6] = [
            HdInterpolation::Constant,
            HdInterpolation::Uniform,
            HdInterpolation::Varying,
            HdInterpolation::Vertex,
            HdInterpolation::FaceVarying,
            HdInterpolation::Instance,
        ];

        let mut children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
        for &interp in &ALL_INTERPS {
            let descs = sd.get_ext_computation_primvar_descriptors(&self.id, interp);
            let interp_token = Token::new(interp.as_str());
            for desc in &descs {
                let mut pv_fields: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
                pv_fields.insert(
                    Token::new("interpolation"),
                    HdRetainedSampledDataSource::new(Value::from(interp_token.as_str().to_string()))
                        as HdDataSourceBaseHandle,
                );
                if !desc.role.is_empty() {
                    pv_fields.insert(
                        Token::new("role"),
                        HdRetainedSampledDataSource::new(Value::from(
                            desc.role.as_str().to_string(),
                        )) as HdDataSourceBaseHandle,
                    );
                }
                pv_fields.insert(
                    Token::new("sourceComputation"),
                    HdRetainedSampledDataSource::new(Value::from(
                        desc.source_computation_id.to_string(),
                    )) as HdDataSourceBaseHandle,
                );
                pv_fields.insert(
                    Token::new("sourceComputationOutputName"),
                    HdRetainedSampledDataSource::new(Value::from(
                        desc.source_computation_output_name.as_str().to_string(),
                    )) as HdDataSourceBaseHandle,
                );
                children.insert(
                    desc.name.clone(),
                    HdRetainedContainerDataSource::new(pv_fields),
                );
            }
        }

        if children.is_empty() {
            None
        } else {
            Some(HdRetainedContainerDataSource::new(children))
        }
    }

    /// Render buffer data source.
    fn get_render_buffer_ds(&self) -> Option<HdDataSourceBaseHandle> {
        let sd = self.scene_delegate.as_ref()?;
        let desc = sd.get_render_buffer_descriptor(&self.id);
        let mut children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
        children.insert(
            Token::new("dimensions"),
            HdRetainedSampledDataSource::new(Value::from(vec![
                desc.dimensions[0],
                desc.dimensions[1],
                desc.dimensions[2],
            ])) as HdDataSourceBaseHandle,
        );
        Some(HdRetainedContainerDataSource::new(children))
    }

    /// Render settings data source.
    fn get_render_settings_ds(&self) -> Option<HdDataSourceBaseHandle> {
        // C++: Hd_DataSourceRenderSettings - reads various render settings params
        // Placeholder: return generic delegate.Get()-based container
        self.get_generic_prim_ds(&Token::new("renderSettings"))
    }

    /// Integrator data source.
    fn get_integrator_ds(&self) -> Option<HdDataSourceBaseHandle> {
        let sd = self.scene_delegate.as_ref()?;
        let resource = sd.get(&self.id, &Token::new("resource"));
        if resource.is_empty() {
            return None;
        }
        let mut children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
        children.insert(
            Token::new("resource"),
            HdRetainedSampledDataSource::new(resource) as HdDataSourceBaseHandle,
        );
        Some(HdRetainedContainerDataSource::new(children))
    }

    /// Sample filter data source.
    fn get_sample_filter_ds(&self) -> Option<HdDataSourceBaseHandle> {
        self.get_integrator_ds() // Same pattern: read "resource" key
    }

    /// Display filter data source.
    fn get_display_filter_ds(&self) -> Option<HdDataSourceBaseHandle> {
        self.get_integrator_ds() // Same pattern: read "resource" key
    }

    /// Instance categories data source.
    fn get_instance_categories_ds(&self) -> Option<HdDataSourceBaseHandle> {
        let sd = self.scene_delegate.as_ref()?;
        let cats = sd.get_instance_categories(&self.id);
        if cats.is_empty() {
            return None;
        }
        // Store as array of arrays of tokens
        let cat_strings: Vec<Vec<String>> = cats
            .iter()
            .map(|c| c.iter().map(|t| t.as_str().to_string()).collect())
            .collect();
        let mut children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
        children.insert(
            Token::new("categoriesValues"),
            HdRetainedSampledDataSource::new(Value::from(
                cat_strings.iter().map(|c| c.join(",")).collect::<Vec<_>>(),
            )) as HdDataSourceBaseHandle,
        );
        Some(HdRetainedContainerDataSource::new(children))
    }

    /// Generic prim data source: returns delegate.Get(id, key) wrapped.
    fn get_generic_prim_ds(&self, _key: &Token) -> Option<HdDataSourceBaseHandle> {
        // C++ uses Hd_GenericGetSampledDataSource. We return an empty container.
        // Full implementation would enumerate params from the delegate.
        Some(HdRetainedContainerDataSource::new_empty())
    }
}

impl fmt::Debug for HdDataSourceLegacyPrim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HdDataSourceLegacyPrim")
            .field("id", &self.id)
            .field("prim_type", &self.prim_type)
            .finish()
    }
}

impl HdDataSourceBase for HdDataSourceLegacyPrim {
    /// P1-7: Clone intentionally resets cached state (primvars_built = false, caches = None).
    /// This is correct: the clone is a fresh handle that will lazily rebuild its caches
    /// from the same underlying scene_delegate. Do not carry over cached state to avoid
    /// stale data in the clone after subsequent delegate changes.
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(HdDataSourceLegacyPrim {
            id: self.id.clone(),
            prim_type: self.prim_type.clone(),
            scene_delegate: self.scene_delegate.clone(),
            primvars_built: AtomicBool::new(false),
            primvars: Mutex::new(None),
            instancer_topology: Mutex::new(None),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(HdDataSourceLegacyPrim {
            id: self.id.clone(),
            prim_type: self.prim_type.clone(),
            scene_delegate: self.scene_delegate.clone(),
            primvars_built: AtomicBool::new(false),
            primvars: Mutex::new(None),
            instancer_topology: Mutex::new(None),
        }))
    }
}

impl HdContainerDataSource for HdDataSourceLegacyPrim {
    /// Return available data source names based on prim type.
    /// Matches C++ HdDataSourceLegacyPrim::GetNames exactly.
    fn get_names(&self) -> Vec<Token> {
        let mut result = Vec::new();

        if self.prim_type == *TYPE_MESH {
            result.push(TOK_MESH.clone());
        }
        if self.prim_type == *TYPE_BASIS_CURVES {
            result.push(TOK_BASIS_CURVES.clone());
        }

        // All legacy prims can provide primvars
        result.push(TOK_PRIMVARS.clone());

        if hd_prim_type_is_gprim(&self.prim_type) {
            result.push(TOK_EXT_COMP_PRIMVARS.clone());
            result.push(TOK_MATERIAL_BINDINGS.clone());
            result.push(TOK_DISPLAY_STYLE.clone());
            result.push(TOK_COORD_SYS_BINDING.clone());
            result.push(TOK_PURPOSE.clone());
            result.push(TOK_VISIBILITY.clone());
            result.push(TOK_CATEGORIES.clone());
            result.push(TOK_XFORM.clone());
            result.push(TOK_EXTENT.clone());
        }

        if self.is_light() || self.prim_type == *TYPE_LIGHT_FILTER {
            result.push(TOK_MATERIAL.clone());
            result.push(TOK_XFORM.clone());
            result.push(TOK_LIGHT.clone());
            result.push(TOK_COLLECTIONS.clone());
            result.push(TOK_DEPENDENCIES.clone());
        }

        if self.prim_type == *TYPE_MATERIAL {
            result.push(TOK_MATERIAL.clone());
        }

        if self.prim_type == *TYPE_INSTANCER {
            result.push(TOK_XFORM.clone());
            result.push(TOK_INSTANCER_TOPOLOGY.clone());
            result.push(TOK_INSTANCE_CATEGORIES.clone());
            result.push(TOK_CATEGORIES.clone());
        }

        if self.is_instanceable() {
            result.push(TOK_INSTANCED_BY.clone());
        }

        if self.prim_type == *TYPE_CAMERA {
            result.push(TOK_CAMERA.clone());
            result.push(TOK_XFORM.clone());
        }

        if self.prim_type == *TYPE_RENDER_BUFFER {
            result.push(TOK_RENDER_BUFFER.clone());
        }
        if self.prim_type == *TYPE_RENDER_SETTINGS {
            result.push(TOK_RENDER_SETTINGS.clone());
        }
        if self.prim_type == *TYPE_INTEGRATOR {
            result.push(TOK_INTEGRATOR.clone());
        }
        if self.prim_type == *TYPE_SAMPLE_FILTER {
            result.push(TOK_SAMPLE_FILTER.clone());
        }
        if self.prim_type == *TYPE_DISPLAY_FILTER {
            result.push(TOK_DISPLAY_FILTER.clone());
        }

        if hd_legacy_prim_type_is_volume_field(&self.prim_type) {
            result.push(TOK_VOLUME_FIELD.clone());
        }
        if self.prim_type == *TYPE_VOLUME {
            result.push(TOK_VOLUME_FIELD_BINDING.clone());
        }
        if self.prim_type == *TYPE_EXT_COMPUTATION {
            result.push(TOK_EXT_COMPUTATION.clone());
        }
        if self.prim_type == *TYPE_COORD_SYS {
            result.push(TOK_XFORM.clone());
        }
        if self.prim_type == *TYPE_DRAW_TARGET {
            result.push(TOK_DRAW_TARGET.clone());
        }
        if self.prim_type == *TYPE_IMAGE_SHADER {
            result.push(TOK_IMAGE_SHADER.clone());
        }

        // All legacy prims advertise their scene delegate
        result.push(TOK_SCENE_DELEGATE.clone());

        result
    }

    /// Get child data source by schema name. Dispatches to inner data source
    /// constructors. Matches C++ HdDataSourceLegacyPrim::Get dispatch.
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let s = name.as_str();
        let _reentry = match LegacyPrimGetReentryGuard::enter(&self.id, s) {
            Some(g) => g,
            None => return Some(HdRetainedContainerDataSource::new_empty()),
        };

        if *name == *TOK_MESH {
            if self.prim_type == *TYPE_MESH {
                return self.scene_delegate.as_ref().map(|sd| {
                    LegacyMeshDs::new(self.id.clone(), sd.clone()) as HdDataSourceBaseHandle
                });
            }
        } else if *name == *TOK_BASIS_CURVES {
            if self.prim_type == *TYPE_BASIS_CURVES {
                return self.scene_delegate.as_ref().map(|sd| {
                    LegacyBasisCurvesDs::new(self.id.clone(), sd.clone()) as HdDataSourceBaseHandle
                });
            }
        } else if *name == *TOK_PRIMVARS {
            return self.get_primvars_ds();
        } else if *name == *TOK_MATERIAL_BINDINGS {
            return self.get_material_bindings_ds();
        } else if *name == *TOK_XFORM {
            return self.get_xform_ds();
        } else if *name == *TOK_DISPLAY_STYLE {
            return self.get_display_style_ds();
        } else if *name == *TOK_SCENE_DELEGATE {
            return self
                .scene_delegate
                .as_ref()
                .map(|sd| LegacySceneDelegateDs::new(sd.clone()));
        } else if *name == *TOK_INSTANCED_BY {
            return self.get_instanced_by_ds();
        } else if *name == *TOK_VISIBILITY {
            return self.get_visibility_ds();
        } else if *name == *TOK_PURPOSE {
            return self.get_purpose_ds();
        } else if *name == *TOK_EXTENT {
            return self.get_extent_ds();
        } else if *name == *TOK_CATEGORIES {
            return self.get_categories_ds();
        // --- Camera: generic param container from delegate ---
        } else if *name == *TOK_CAMERA {
            return self.get_camera_ds();

        // --- Light: generic param container from delegate ---
        } else if *name == *TOK_LIGHT {
            return self.get_light_ds();

        // --- Material: material network from delegate ---
        } else if *name == *TOK_MATERIAL {
            return self.get_material_ds();

        // --- Instancer topology ---
        } else if *name == *TOK_INSTANCER_TOPOLOGY {
            return self.get_instancer_topology_ds();

        // --- Coord sys binding ---
        } else if *name == *TOK_COORD_SYS_BINDING {
            return self.get_coord_sys_binding_ds();

        // --- Volume field binding (for "volume" type prims) ---
        } else if *name == *TOK_VOLUME_FIELD_BINDING {
            return self.get_volume_field_binding_ds();

        // --- Volume field (for volume field prims themselves) ---
        } else if *name == *TOK_VOLUME_FIELD {
            return self.get_volume_field_ds();

        // --- Ext computation ---
        } else if *name == *TOK_EXT_COMPUTATION {
            return self.get_ext_computation_ds();

        // --- Ext computation primvars ---
        } else if *name == *TOK_EXT_COMP_PRIMVARS {
            return self.get_ext_computation_primvars_ds();

        // --- Render buffer ---
        } else if *name == *TOK_RENDER_BUFFER {
            return self.get_render_buffer_ds();

        // --- Render settings ---
        } else if *name == *TOK_RENDER_SETTINGS {
            return self.get_render_settings_ds();

        // --- Integrator ---
        } else if *name == *TOK_INTEGRATOR {
            return self.get_integrator_ds();

        // --- Sample filter ---
        } else if *name == *TOK_SAMPLE_FILTER {
            return self.get_sample_filter_ds();

        // --- Display filter ---
        } else if *name == *TOK_DISPLAY_FILTER {
            return self.get_display_filter_ds();

        // --- Instance categories ---
        } else if *name == *TOK_INSTANCE_CATEGORIES {
            return self.get_instance_categories_ds();

        // --- Collections (light linking) ---
        } else if *name == *TOK_COLLECTIONS {
            // C++: only lights and lightFilters provide collections
            if self.is_light() || self.prim_type == *TYPE_LIGHT_FILTER {
                // Placeholder: light linking collections need full membership
                // expression support. Return empty container for now.
                return Some(HdRetainedContainerDataSource::new_empty());
            }
        // --- Dependencies ---
        } else if *name == *TOK_DEPENDENCIES {
            // Placeholder: dependency data sources for lights
            return Some(HdRetainedContainerDataSource::new_empty());

        // --- Draw target ---
        } else if *name == *TOK_DRAW_TARGET {
            // Generic get-based container
            return self.get_generic_prim_ds(&Token::new("drawTarget"));

        // --- Image shader ---
        } else if *name == *TOK_IMAGE_SHADER {
            return self.get_generic_prim_ds(&Token::new("imageShader"));
        }

        None
    }
}
