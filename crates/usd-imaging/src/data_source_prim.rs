//! Base prim data source - the core USD->Hydra data pipeline.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourcePrim.h/cpp
//!
//! DataSourcePrim::Get() lazily creates sub-data-sources for:
//! - "xform" -> DataSourceXform (transform matrix from UsdGeomXformable)
//! - "primvars" -> DataSourcePrimvars (primvar enumeration)
//! - "visibility" -> DataSourceVisibility (USD bi-state to Hydra tri-state)
//! - "purpose" -> DataSourcePurpose (with fallback/inheritable)
//! - "extent" -> DataSourceExtent (from UsdGeomBoundable)
//! - "__usdPrimInfo" -> DataSourceUsdPrimInfo
//! - "primOrigin" -> DataSourcePrimOrigin

use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::types::PropertyInvalidationType;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use usd_core::Prim;
use usd_gf::{Interval, Matrix4d, Vec3d};
use usd_hd::schema::HdMatrixDataSourceHandle;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdRetainedTypedSampledDataSource,
    HdSampledDataSource, HdSampledDataSourceTime, HdTypedSampledDataSource,
};
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;
use usd_vt::Value;

static DEBUG_DATA_SOURCE_XFORM_FROM_QUERY_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_DATA_SOURCE_XFORM_FROM_QUERY_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static DEBUG_DATA_SOURCE_XFORM_MATRIX_TYPED_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_DATA_SOURCE_XFORM_MATRIX_TYPED_TOTAL_NS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Default)]
pub struct DebugDataSourcePrimXformStats {
    pub data_source_xform_from_query_calls: usize,
    pub data_source_xform_from_query_total_ns: u64,
    pub data_source_xform_matrix_typed_calls: usize,
    pub data_source_xform_matrix_typed_total_ns: u64,
}

pub fn reset_debug_data_source_prim_xform_stats() {
    DEBUG_DATA_SOURCE_XFORM_FROM_QUERY_CALLS.store(0, Ordering::Relaxed);
    DEBUG_DATA_SOURCE_XFORM_FROM_QUERY_TOTAL_NS.store(0, Ordering::Relaxed);
    DEBUG_DATA_SOURCE_XFORM_MATRIX_TYPED_CALLS.store(0, Ordering::Relaxed);
    DEBUG_DATA_SOURCE_XFORM_MATRIX_TYPED_TOTAL_NS.store(0, Ordering::Relaxed);
}

pub fn read_debug_data_source_prim_xform_stats() -> DebugDataSourcePrimXformStats {
    DebugDataSourcePrimXformStats {
        data_source_xform_from_query_calls: DEBUG_DATA_SOURCE_XFORM_FROM_QUERY_CALLS
            .load(Ordering::Relaxed),
        data_source_xform_from_query_total_ns: DEBUG_DATA_SOURCE_XFORM_FROM_QUERY_TOTAL_NS
            .load(Ordering::Relaxed),
        data_source_xform_matrix_typed_calls: DEBUG_DATA_SOURCE_XFORM_MATRIX_TYPED_CALLS
            .load(Ordering::Relaxed),
        data_source_xform_matrix_typed_total_ns: DEBUG_DATA_SOURCE_XFORM_MATRIX_TYPED_TOTAL_NS
            .load(Ordering::Relaxed),
    }
}

fn debug_time_dirty_enabled() -> bool {
    std::env::var_os("USD_RS_DEBUG_TIME_DIRTY").is_some()
}

// Lazy token constants for schema type checking and property names
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    // Schema type names for Prim::is_a() checks
    pub static IMAGEABLE: LazyLock<Token> = LazyLock::new(|| Token::new("Imageable"));
    pub static XFORMABLE: LazyLock<Token> = LazyLock::new(|| Token::new("Xformable"));
    pub static BOUNDABLE: LazyLock<Token> = LazyLock::new(|| Token::new("Boundable"));

    // Hydra schema names
    pub static XFORM: LazyLock<Token> = LazyLock::new(|| Token::new("xform"));
    pub static VISIBILITY: LazyLock<Token> = LazyLock::new(|| Token::new("visibility"));
    pub static PURPOSE: LazyLock<Token> = LazyLock::new(|| Token::new("purpose"));
    pub static EXTENT: LazyLock<Token> = LazyLock::new(|| Token::new("extent"));
    pub static PRIMVARS: LazyLock<Token> = LazyLock::new(|| Token::new("primvars"));
    pub static USD_PRIM_INFO: LazyLock<Token> = LazyLock::new(|| Token::new("__usdPrimInfo"));
    pub static PRIM_ORIGIN: LazyLock<Token> = LazyLock::new(|| Token::new("primOrigin"));

    // Sub-field tokens
    pub static MATRIX: LazyLock<Token> = LazyLock::new(|| Token::new("matrix"));
    pub static RESET_XFORM_STACK: LazyLock<Token> = LazyLock::new(|| Token::new("resetXformStack"));
    pub static MIN: LazyLock<Token> = LazyLock::new(|| Token::new("min"));
    pub static MAX: LazyLock<Token> = LazyLock::new(|| Token::new("max"));
    pub static INHERITABLE: LazyLock<Token> = LazyLock::new(|| Token::new("inheritable"));
    pub static FALLBACK: LazyLock<Token> = LazyLock::new(|| Token::new("fallback"));
    pub static SCENE_PATH: LazyLock<Token> = LazyLock::new(|| Token::new("scenePath"));

    // USD visibility values
    pub static INVISIBLE: LazyLock<Token> = LazyLock::new(|| Token::new("invisible"));

    // Purpose mapping: USD "default" -> Hydra "geometry"
    pub static DEFAULT_PURPOSE: LazyLock<Token> = LazyLock::new(|| Token::new("default"));
    pub static GEOMETRY: LazyLock<Token> = LazyLock::new(|| Token::new("geometry"));
}

// ============================================================================
// DataSourceVisibility
// ============================================================================

/// Data source for prim visibility (USD bi-state -> Hydra tri-state).
///
/// USD visibility is bi-state: "invisible" or "inherited" (default).
/// Hydra visibility is tri-state: visible(true), invisible(false), or inherited(None).
///
/// Mapping:
/// - USD "invisible" -> Hydra boolean false
/// - USD "inherited"/unset -> Hydra None (null = inherit from parent)
#[derive(Clone)]
pub struct DataSourceVisibility {
    /// The USD prim to read visibility from
    prim: Prim,
    /// Stage globals for time code
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceVisibility")
    }
}

impl DataSourceVisibility {
    /// Create new visibility data source.
    pub fn new(prim: Prim, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourceVisibility {
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

impl HdContainerDataSource for DataSourceVisibility {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::VISIBILITY.clone()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name != &*tokens::VISIBILITY {
            return None;
        }

        // Read visibility attribute from UsdGeomImageable
        let imageable = usd_geom::imageable::Imageable::new(self.prim.clone());
        if !imageable.is_valid() {
            return None;
        }

        let vis_attr = imageable.get_visibility_attr();
        if !vis_attr.is_valid() || !vis_attr.has_authored_value() {
            // Not authored -> inherited (Hydra: no data source = inherit)
            return None;
        }

        // Read the token value at current time
        let usd_time = self.stage_globals.get_time();
        let time = TimeCode::new(usd_time.value());
        if let Some(val) = vis_attr.get(time) {
            if let Some(vis_token) = val.downcast_clone::<Token>() {
                if vis_token == *tokens::INVISIBLE {
                    // "invisible" -> Hydra false
                    return Some(HdRetainedTypedSampledDataSource::new(false));
                }
            }
        }

        // "inherited" or any other value -> no data source = inherit
        None
    }
}

// ============================================================================
// DataSourcePurpose
// ============================================================================

/// Data source for prim purpose (geometry, render, proxy, guide).
///
/// Returns purpose/inheritable/fallback following C++ semantics:
/// - "purpose": authored value (with default->geometry remapping)
/// - "fallback": non-authored fallback value
/// - "inheritable": true if the value was authored (and thus inheritable)
#[derive(Clone)]
pub struct DataSourcePurpose {
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourcePurpose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourcePurpose")
    }
}

impl DataSourcePurpose {
    /// Create new purpose data source for the given prim.
    pub fn new(prim: Prim, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
        })
    }

    /// Map USD purpose token to Hydra: "default" -> "geometry", others pass through
    fn to_hydra_purpose(purpose: &Token) -> Token {
        if purpose == &*tokens::DEFAULT_PURPOSE {
            tokens::GEOMETRY.clone()
        } else {
            purpose.clone()
        }
    }
}

impl HdDataSourceBase for DataSourcePurpose {
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

impl HdContainerDataSource for DataSourcePurpose {
    fn get_names(&self) -> Vec<Token> {
        vec![
            tokens::PURPOSE.clone(),
            tokens::INHERITABLE.clone(),
            tokens::FALLBACK.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let imageable = usd_geom::imageable::Imageable::new(self.prim.clone());
        if !imageable.is_valid() {
            return None;
        }

        let purpose_attr = imageable.get_purpose_attr();
        let is_authored = purpose_attr.is_valid() && purpose_attr.has_authored_value();

        if name == &*tokens::PURPOSE {
            // Only return authored purpose value
            if !is_authored {
                return None;
            }
            let usd_time = self.stage_globals.get_time();
            let time = TimeCode::new(usd_time.value());
            if let Some(val) = purpose_attr.get(time) {
                if let Some(purpose_token) = val.downcast_clone::<Token>() {
                    let hydra_purpose = Self::to_hydra_purpose(&purpose_token);
                    return Some(HdRetainedTypedSampledDataSource::new(hydra_purpose));
                }
            }
            None
        } else if name == &*tokens::FALLBACK {
            // Return fallback only when NOT authored
            if is_authored {
                return None;
            }
            let usd_time = self.stage_globals.get_time();
            let time = TimeCode::new(usd_time.value());
            if let Some(val) = purpose_attr.get(time) {
                if let Some(purpose_token) = val.downcast_clone::<Token>() {
                    let hydra_purpose = Self::to_hydra_purpose(&purpose_token);
                    return Some(HdRetainedTypedSampledDataSource::new(hydra_purpose));
                }
            }
            None
        } else if name == &*tokens::INHERITABLE {
            // Authored purpose is inheritable
            if is_authored {
                return Some(HdRetainedTypedSampledDataSource::new(true));
            }
            None
        } else {
            None
        }
    }
}

// ============================================================================
// DataSourceXformMatrix
// ============================================================================

/// Sampled data source for the LOCAL transform matrix.
///
/// Stores an XformQuery (cached xformOps) for efficient per-frame evaluation.
/// Returns local-to-parent transform only. World transform composition is
/// handled by HdFlatteningSceneIndex via its xform provider.
/// Matches C++ UsdImagingDataSourceXformMatrix.
#[derive(Clone)]
pub struct DataSourceXformMatrix {
    xform_query: usd_geom::xformable::XformQuery,
    stage_globals: DataSourceStageGlobalsHandle,
    static_value: Option<Matrix4d>,
}

impl std::fmt::Debug for DataSourceXformMatrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceXformMatrix")
    }
}

impl DataSourceXformMatrix {
    /// Create new xform matrix sampled data source.
    /// Reuses the caller-provided `XformQuery`, matching OpenUSD's shared
    /// `UsdGeomXformable::XformQuery` ownership between the xform container and
    /// its child sampled data sources.
    pub fn new(
        xform_query: usd_geom::xformable::XformQuery,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        let static_value = if xform_query.transform_might_be_time_varying() {
            None
        } else {
            let base_time = stage_globals.get_time();
            let time = TimeCode::new(base_time.value());
            Some(
                xform_query
                    .get_local_transformation(time)
                    .unwrap_or_else(usd_gf::Matrix4d::identity),
            )
        };
        Arc::new(Self {
            xform_query,
            stage_globals,
            static_value,
        })
    }
}

impl HdDataSourceBase for DataSourceXformMatrix {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }

    fn as_matrix_data_source(&self) -> Option<HdMatrixDataSourceHandle> {
        Some(Arc::new(self.clone()) as HdMatrixDataSourceHandle)
    }

    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(self.get_value(0.0))
    }
}

impl HdSampledDataSource for DataSourceXformMatrix {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        if let Some(matrix) = self.static_value {
            return Value::from(matrix);
        }
        let base_time = self.stage_globals.get_time();
        let time = if base_time.is_numeric() {
            TimeCode::new(base_time.value() + shutter_offset as f64)
        } else {
            TimeCode::new(base_time.value())
        };
        let matrix = self
            .xform_query
            .get_local_transformation(time)
            .unwrap_or_else(usd_gf::Matrix4d::identity);
        Value::from(matrix)
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        let base_time = self.stage_globals.get_time();
        if !self.xform_query.transform_might_be_time_varying() || !base_time.is_numeric() {
            return false;
        }

        let interval = Interval::closed(
            base_time.value() + start_time as f64,
            base_time.value() + end_time as f64,
        );
        let mut time_samples = self.xform_query.get_time_samples_in_interval(&interval);

        if time_samples.is_empty() || time_samples[0] > interval.get_min() {
            time_samples.insert(0, interval.get_min());
        }
        if time_samples.last().copied().unwrap_or(interval.get_min()) < interval.get_max() {
            time_samples.push(interval.get_max());
        }

        out_sample_times.clear();
        out_sample_times.extend(
            time_samples
                .into_iter()
                .map(|t| (t - base_time.value()) as HdSampledDataSourceTime),
        );
        true
    }
}

impl HdTypedSampledDataSource<Matrix4d> for DataSourceXformMatrix {
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> Matrix4d {
        let debug_stats = debug_time_dirty_enabled();
        let started = debug_stats.then(std::time::Instant::now);
        if debug_stats {
            DEBUG_DATA_SOURCE_XFORM_MATRIX_TYPED_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(matrix) = self.static_value {
            if debug_stats {
                if let Some(started) = started {
                    DEBUG_DATA_SOURCE_XFORM_MATRIX_TYPED_TOTAL_NS
                        .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
                }
            }
            return matrix;
        }
        let base_time = self.stage_globals.get_time();
        let time = if base_time.is_numeric() {
            TimeCode::new(base_time.value() + shutter_offset as f64)
        } else {
            TimeCode::new(base_time.value())
        };
        let result = self
            .xform_query
            .get_local_transformation(time)
            .unwrap_or_else(usd_gf::Matrix4d::identity);
        if debug_stats {
            if let Some(started) = started {
                DEBUG_DATA_SOURCE_XFORM_MATRIX_TYPED_TOTAL_NS
                    .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
        }
        result
    }
}

// ============================================================================
// DataSourceXform
// ============================================================================

/// Container data source for transform, exposing "matrix" and "resetXformStack".
#[derive(Clone)]
pub struct DataSourceXform {
    /// Retained child handle for `xform:matrix`.
    ///
    /// OpenUSD returns a fresh child datasource per query. Rust pays materially
    /// more for repeated Arc allocation on the flattening hot path, so we retain
    /// the child handle once while preserving the same live sampled semantics.
    matrix_ds: HdDataSourceBaseHandle,
    /// Retained child handle for `xform:resetXformStack`.
    reset_xform_stack_ds: HdDataSourceBaseHandle,
}

impl std::fmt::Debug for DataSourceXform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceXform")
    }
}

impl DataSourceXform {
    /// Create new transform container data source.
    ///
    /// This caches the prim's local `XformQuery` and flags time-varying state
    /// only for the prim's own local xform locator, matching
    /// `UsdImagingDataSourceXform`.
    pub fn new(
        prim: Prim,
        scene_index_path: Path,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        let xform_query = usd_geom::xformable::XformQuery::from_xformable(
            &usd_geom::xformable::Xformable::new(prim.clone()),
        );
        Self::from_query(xform_query, scene_index_path, stage_globals)
    }

    pub fn from_query(
        xform_query: usd_geom::xformable::XformQuery,
        scene_index_path: Path,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        let debug_stats = debug_time_dirty_enabled();
        let started = debug_stats.then(std::time::Instant::now);
        if debug_stats {
            DEBUG_DATA_SOURCE_XFORM_FROM_QUERY_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        if xform_query.transform_might_be_time_varying() {
            stage_globals.flag_as_time_varying(
                &scene_index_path,
                &HdDataSourceLocator::from_token(tokens::XFORM.clone()),
            );
        }
        let matrix_ds = DataSourceXformMatrix::new(xform_query.clone(), stage_globals.clone())
            as HdDataSourceBaseHandle;
        let reset_xform_stack_ds =
            HdRetainedTypedSampledDataSource::new(xform_query.get_reset_xform_stack())
                as HdDataSourceBaseHandle;
        let result = Arc::new(Self {
            matrix_ds,
            reset_xform_stack_ds,
        });
        if debug_stats {
            if let Some(started) = started {
                DEBUG_DATA_SOURCE_XFORM_FROM_QUERY_TOTAL_NS
                    .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
        }
        result
    }
}

impl HdDataSourceBase for DataSourceXform {
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

impl HdContainerDataSource for DataSourceXform {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::MATRIX.clone(), tokens::RESET_XFORM_STACK.clone()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::MATRIX {
            Some(self.matrix_ds.clone())
        } else if name == &*tokens::RESET_XFORM_STACK {
            Some(self.reset_xform_stack_ds.clone())
        } else {
            None
        }
    }
}

// ============================================================================
// DataSourceExtentCoordinate
// ============================================================================

/// Sampled data source returning a single Vec3d from extent array (min or max).
///
/// Reads the extent float3[] attribute and extracts either index 0 (min)
/// or index 1 (max), converting from Vec3f to Vec3d for Hydra.
#[derive(Clone)]
pub struct DataSourceExtentCoordinate {
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
    /// 0 = min, 1 = max
    index: usize,
}

impl std::fmt::Debug for DataSourceExtentCoordinate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceExtentCoordinate")
    }
}

impl DataSourceExtentCoordinate {
    /// Create new extent coordinate data source (index 0=min, 1=max).
    pub fn new(prim: Prim, stage_globals: DataSourceStageGlobalsHandle, index: usize) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
            index,
        })
    }
}

impl HdDataSourceBase for DataSourceExtentCoordinate {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
    fn sample_at_zero(&self) -> Option<usd_vt::Value> {
        Some(self.get_value(0.0))
    }
}

impl HdSampledDataSource for DataSourceExtentCoordinate {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        let boundable = usd_geom::boundable::Boundable::new(self.prim.clone());
        let base_time = self.stage_globals.get_time();
        let time_val = base_time.value() + shutter_offset as f64;
        let time = TimeCode::new(time_val);

        let extent_attr = boundable.get_extent_attr();
        if !extent_attr.is_valid() {
            return Value::from(Vec3d::new(0.0, 0.0, 0.0));
        }

        if let Some(val) = extent_attr.get(time) {
            // Extent is Vec3f[], need to extract and upcast to Vec3d
            if let Some(extent_array) = val.as_vec_clone::<usd_gf::vec3::Vec3f>() {
                if self.index < extent_array.len() {
                    let v = &extent_array[self.index];
                    return Value::from(Vec3d::new(v.x as f64, v.y as f64, v.z as f64));
                }
            }
        }

        Value::from(Vec3d::new(0.0, 0.0, 0.0))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        let boundable = usd_geom::boundable::Boundable::new(self.prim.clone());
        let extent_attr = boundable.get_extent_attr();
        if !extent_attr.is_valid() {
            return false;
        }

        let samples = extent_attr.get_time_samples();
        if samples.is_empty() {
            return false;
        }

        let base_time = self.stage_globals.get_time().value();
        let window_start = base_time + start_time as f64;
        let window_end = base_time + end_time as f64;

        out_sample_times.clear();
        for sample_time in &samples {
            if *sample_time >= window_start && *sample_time <= window_end {
                out_sample_times.push((*sample_time - base_time) as HdSampledDataSourceTime);
            }
        }

        let mut prev_sample: Option<f64> = None;
        let mut next_sample: Option<f64> = None;
        for sample_time in &samples {
            if *sample_time < window_start {
                prev_sample = Some(*sample_time);
            }
            if *sample_time > window_end {
                next_sample = Some(*sample_time);
                break;
            }
        }

        if let Some(prev) = prev_sample {
            let rel = (prev - base_time) as HdSampledDataSourceTime;
            if !out_sample_times.contains(&rel) {
                out_sample_times.insert(0, rel);
            }
        }
        if let Some(next) = next_sample {
            let rel = (next - base_time) as HdSampledDataSourceTime;
            if !out_sample_times.contains(&rel) {
                out_sample_times.push(rel);
            }
        }

        out_sample_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        !out_sample_times.is_empty()
    }
}

// ============================================================================
// DataSourceExtent
// ============================================================================

/// Container data source for extent: "min" and "max" coordinate data sources.
#[derive(Clone)]
pub struct DataSourceExtent {
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceExtent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceExtent")
    }
}

impl DataSourceExtent {
    /// Create new extent container data source for min/max bounds.
    pub fn new(prim: Prim, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourceExtent {
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

impl HdContainerDataSource for DataSourceExtent {
    fn get_names(&self) -> Vec<Token> {
        // Only return names if the extent attribute actually exists
        let boundable = usd_geom::boundable::Boundable::new(self.prim.clone());
        let extent_attr = boundable.get_extent_attr();
        if extent_attr.is_valid() && extent_attr.has_value() {
            vec![tokens::MIN.clone(), tokens::MAX.clone()]
        } else {
            vec![]
        }
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::MIN {
            Some(DataSourceExtentCoordinate::new(
                self.prim.clone(),
                self.stage_globals.clone(),
                0,
            ))
        } else if name == &*tokens::MAX {
            Some(DataSourceExtentCoordinate::new(
                self.prim.clone(),
                self.stage_globals.clone(),
                1,
            ))
        } else {
            None
        }
    }
}

// ============================================================================
// DataSourcePrimOrigin
// ============================================================================

/// Data source providing the original USD prim path for scene path tracking.
///
/// Returns the scene path under "scenePath", handling prototype-relative
/// paths for USD native instancing.
#[derive(Clone, Debug)]
pub struct DataSourcePrimOrigin {
    prim: Prim,
}

impl DataSourcePrimOrigin {
    /// Create new prim origin data source for scene path tracking.
    pub fn new(prim: Prim) -> Arc<Self> {
        Arc::new(Self { prim })
    }
}

impl HdDataSourceBase for DataSourcePrimOrigin {
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

impl HdContainerDataSource for DataSourcePrimOrigin {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::SCENE_PATH.clone()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name != &*tokens::SCENE_PATH {
            return None;
        }
        if !self.prim.is_valid() {
            return None;
        }
        // Return the prim's path as a path data source
        let path = self.prim.get_path().clone();
        Some(HdRetainedTypedSampledDataSource::new(path))
    }
}

// ============================================================================
// DataSourcePrim
// ============================================================================

/// Base data source for USD prims.
///
/// This provides a container data source that wraps a USD prim and
/// lazily creates typed sub-data-sources on Get(). This is THE core
/// class that routes USD prim data into the Hydra pipeline.
///
/// Get() dynamically creates sub-data-sources based on the prim's schema
/// type (IsA<UsdGeomImageable>, IsA<UsdGeomXformable>, etc.), matching
/// the C++ UsdImagingDataSourcePrim behavior.
///
/// The "children" HashMap provides an override mechanism: explicitly added
/// children take priority over the dynamic Get() logic. This is used by
/// derived prim adapters (mesh, camera, etc.) to add type-specific data.
#[derive(Clone)]
pub struct DataSourcePrim {
    /// USD prim being wrapped
    prim: Prim,
    /// Hydra path (may differ from USD path)
    hydra_path: Path,
    /// Stage globals context
    stage_globals: DataSourceStageGlobalsHandle,
    /// Override child data sources by name (from adapters)
    children: HashMap<Token, HdDataSourceBaseHandle>,
    /// Cached xform data source — avoids recreating XformQuery per get("xform") call.
    /// XformQuery construction resolves all xformOps + creates AttributeQueries,
    /// which is expensive (~0.3ms/prim). Cache persists for the DataSourcePrim lifetime.
    cached_xform: std::sync::OnceLock<Option<HdDataSourceBaseHandle>>,
}

impl std::fmt::Debug for DataSourcePrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourcePrim")
            .field("prim", &self.prim)
            .field("hydra_path", &self.hydra_path)
            .field("stage_globals", &"<DataSourceStageGlobals>")
            .field("children_count", &self.children.len())
            .finish()
    }
}

impl DataSourcePrim {
    /// Create new prim data source.
    ///
    /// # Arguments
    ///
    /// * `prim` - USD prim to wrap
    /// * `hydra_path` - Hydra prim path (may differ from USD path)
    /// * `stage_globals` - Stage globals context
    pub fn new(prim: Prim, hydra_path: Path, stage_globals: DataSourceStageGlobalsHandle) -> Self {
        Self {
            prim,
            hydra_path,
            stage_globals,
            children: HashMap::new(),
            cached_xform: std::sync::OnceLock::new(),
        }
    }

    /// Add an override child data source (used by derived adapters).
    pub fn add_child(&mut self, name: Token, data_source: HdDataSourceBaseHandle) {
        self.children.insert(name, data_source);
    }

    /// Get the USD prim.
    pub fn prim(&self) -> &Prim {
        &self.prim
    }

    /// Get the Hydra path.
    pub fn hydra_path(&self) -> &Path {
        &self.hydra_path
    }

    /// Get the stage globals.
    pub fn stage_globals(&self) -> &DataSourceStageGlobalsHandle {
        &self.stage_globals
    }

    /// Check if prim is a UsdGeomImageable (has visibility, purpose).
    fn is_imageable(&self) -> bool {
        self.prim.is_a(&tokens::IMAGEABLE)
    }

    /// Check if prim is a UsdGeomXformable (has transform).
    fn is_xformable(&self) -> bool {
        self.prim.is_a(&tokens::XFORMABLE)
    }

    /// Check if prim is a UsdGeomBoundable (has extent).
    fn is_boundable(&self) -> bool {
        self.prim.is_a(&tokens::BOUNDABLE)
    }

    /// Check if this prim has authored xform attributes.
    /// C++ parity: _HasAuthoredXform — used for GeomXformVectorsSchema.
    #[allow(dead_code)] // will be used when GeomXformVectorsSchema is implemented
    fn has_xform_ops(&self) -> bool {
        let xformable = usd_geom::xformable::Xformable::new(self.prim.clone());
        if !xformable.is_valid() {
            return false;
        }
        let mut resets = false;
        let ops = xformable.get_ordered_xform_ops_with_reset(&mut resets);
        !ops.is_empty() || resets
    }

    /// Compute invalidation locators for property changes.
    ///
    /// Maps USD property names to Hydra data source locators that
    /// need to be invalidated (dirtied) when those properties change.
    pub fn invalidate(
        _prim: &Prim,
        _subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators = HdDataSourceLocatorSet::empty();

        for prop in properties {
            let prop_str = prop.as_str();

            // Transform changes: xformOpOrder or any xformOp:* attribute
            if prop_str == "xformOpOrder" || prop_str.starts_with("xformOp:") {
                locators.insert(HdDataSourceLocator::from_token(Token::new("xform")));
                continue;
            }

            // Common property -> locator mappings
            match prop_str {
                "visibility" => {
                    locators.insert(HdDataSourceLocator::from_token(Token::new("visibility")));
                }
                "purpose" => {
                    locators.insert(HdDataSourceLocator::from_token(Token::new("purpose")));
                }
                "extent" => {
                    locators.insert(HdDataSourceLocator::from_token(Token::new("extent")));
                }
                "extentsHint" => {
                    locators.insert(HdDataSourceLocator::from_token(Token::new("extentsHint")));
                }
                "proxyPrim" => {
                    locators.insert(HdDataSourceLocator::from_token(Token::new("model")));
                }
                _ => {
                    // Primvar changes: "primvars:*"
                    if prop_str.starts_with("primvars:") {
                        match invalidation_type {
                            PropertyInvalidationType::Resync => {
                                locators.insert(HdDataSourceLocator::from_token(Token::new(
                                    "primvars",
                                )));
                            }
                            PropertyInvalidationType::PropertyChanged => {
                                // Append specific primvar name to locator
                                let primvar_name = &prop_str[9..]; // skip "primvars:"
                                locators.insert(HdDataSourceLocator::from_tokens_2(
                                    Token::new("primvars"),
                                    Token::new(primvar_name),
                                ));
                            }
                        }
                    }
                }
            }
        }

        locators
    }
}

impl HdContainerDataSource for DataSourcePrim {
    /// Lazily create and return sub-data-sources by name.
    ///
    /// This is the critical Get() method that routes USD data into Hydra.
    /// Override children (from adapters) take priority, then dynamic
    /// creation based on schema type.
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // Priority 1: explicit override children from adapters
        if let Some(child) = self.children.get(name) {
            return Some(child.clone());
        }

        // __usdPrimInfo and primOrigin are always available (per C++ reference)
        if name == &*tokens::USD_PRIM_INFO {
            return Some(Arc::new(
                crate::data_source_usd_prim_info::DataSourceUsdPrimInfo::new(self.prim.clone()),
            ));
        }
        if name == &*tokens::PRIM_ORIGIN {
            return Some(DataSourcePrimOrigin::new(self.prim.clone()));
        }

        // Schema-dependent data sources require a valid prim path
        if !self.hydra_path.is_prim_path() {
            return None;
        }

        // "xform" -> DataSourceXform (C++ parity: only for UsdGeomXformable)
        // Cached: XformQuery creation is expensive (~0.3ms) — resolves all xformOps
        // and creates AttributeQueries. Cache the result for the DataSourcePrim lifetime.
        if name == &*tokens::XFORM {
            return self
                .cached_xform
                .get_or_init(|| {
                    if !self.is_xformable() {
                        return None;
                    }
                    let xformable = usd_geom::xformable::Xformable::new(self.prim.clone());
                    if !xformable.is_valid() {
                        return None;
                    }
                    let xform_query = usd_geom::xformable::XformQuery::from_xformable(&xformable);
                    if !xform_query.transform_might_have_effect() {
                        return None;
                    }
                    Some(DataSourceXform::from_query(
                        xform_query,
                        self.hydra_path.clone(),
                        self.stage_globals.clone(),
                    ))
                })
                .clone();
        }

        // "visibility" -> DataSourceVisibility (for UsdGeomImageable prims)
        if name == &*tokens::VISIBILITY {
            if !self.is_imageable() {
                return None;
            }
            // Only return if visibility is authored (otherwise inherit)
            let imageable = usd_geom::imageable::Imageable::new(self.prim.clone());
            let vis_attr = imageable.get_visibility_attr();
            if !vis_attr.is_valid() || !vis_attr.has_authored_value() {
                return None;
            }
            return Some(DataSourceVisibility::new(
                self.prim.clone(),
                self.stage_globals.clone(),
            ));
        }

        // "purpose" -> DataSourcePurpose (for UsdGeomImageable prims)
        if name == &*tokens::PURPOSE {
            if !self.is_imageable() {
                return None;
            }
            return Some(DataSourcePurpose::new(
                self.prim.clone(),
                self.stage_globals.clone(),
            ));
        }

        // "extent" -> DataSourceExtent (for UsdGeomBoundable prims)
        if name == &*tokens::EXTENT {
            if !self.is_boundable() {
                return None;
            }
            // Only return if extent is authored
            let boundable = usd_geom::boundable::Boundable::new(self.prim.clone());
            let extent_attr = boundable.get_extent_attr();
            if !extent_attr.is_valid() || !extent_attr.has_authored_value() {
                return None;
            }
            return Some(DataSourceExtent::new(
                self.prim.clone(),
                self.stage_globals.clone(),
            ));
        }

        // "primvars" -> DataSourcePrimvars
        if name == &*tokens::PRIMVARS {
            return Some(Arc::new(
                crate::data_source_primvars::DataSourcePrimvars::new(
                    self.hydra_path.clone(),
                    self.prim.clone(),
                    self.stage_globals.clone(),
                ),
            ));
        }

        None
    }

    /// Return names of available sub-data-sources, conditioned on prim type.
    fn get_names(&self) -> Vec<Token> {
        // __usdPrimInfo, primOrigin, and primvars always available (per C++ reference)
        let mut names = vec![
            tokens::USD_PRIM_INFO.clone(),
            tokens::PRIM_ORIGIN.clone(),
            tokens::PRIMVARS.clone(),
        ];

        if !self.hydra_path.is_prim_path() {
            return names;
        }

        // Add override children names
        for key in self.children.keys() {
            if !names.contains(key) {
                names.push(key.clone());
            }
        }

        // Conditional names based on schema type
        if self.is_imageable() {
            names.push(tokens::VISIBILITY.clone());
            names.push(tokens::PURPOSE.clone());
        }

        // C++ parity: only add xform for UsdGeomXformable prims
        if self.is_xformable() {
            names.push(tokens::XFORM.clone());
        }

        if self.is_boundable() {
            names.push(tokens::EXTENT.clone());
        }

        names
    }
}

impl HdDataSourceBase for DataSourcePrim {
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

/// Builder for constructing prim data sources with override children.
pub struct DataSourcePrimBuilder {
    prim: Prim,
    hydra_path: Path,
    stage_globals: DataSourceStageGlobalsHandle,
    children: HashMap<Token, HdDataSourceBaseHandle>,
}

impl DataSourcePrimBuilder {
    /// Create new builder.
    pub fn new(prim: Prim, hydra_path: Path, stage_globals: DataSourceStageGlobalsHandle) -> Self {
        Self {
            prim,
            hydra_path,
            stage_globals,
            children: HashMap::new(),
        }
    }

    /// Add a child data source override.
    pub fn add(mut self, name: Token, data_source: HdDataSourceBaseHandle) -> Self {
        self.children.insert(name, data_source);
        self
    }

    /// Build the prim data source.
    pub fn build(self) -> DataSourcePrim {
        DataSourcePrim {
            prim: self.prim,
            hydra_path: self.hydra_path,
            stage_globals: self.stage_globals,
            children: self.children,
            cached_xform: std::sync::OnceLock::new(),
        }
    }

    /// Build and wrap in Arc.
    pub fn build_handle(self) -> HdContainerDataSourceHandle {
        Arc::new(self.build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::{DataSourceStageGlobals, NoOpStageGlobals};
    use std::sync::Mutex;
    use usd_core::Stage;
    use usd_hd::HdRetainedContainerDataSource;

    fn make_stage() -> Arc<Stage> {
        usd_core::schema_registry::register_builtin_schemas();
        Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("Failed to create stage")
    }

    fn make_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[derive(Default)]
    struct RecordingStageGlobals {
        flagged: Mutex<Vec<(Path, HdDataSourceLocator)>>,
    }

    impl DataSourceStageGlobals for RecordingStageGlobals {
        fn get_time(&self) -> usd_core::TimeCode {
            usd_core::TimeCode::default_time()
        }

        fn flag_as_time_varying(&self, hydra_path: &Path, locator: &HdDataSourceLocator) {
            self.flagged
                .lock()
                .expect("recording lock")
                .push((hydra_path.clone(), locator.clone()));
        }

        fn flag_as_asset_path_dependent(&self, _usd_path: &Path) {}
    }

    #[test]
    fn test_data_source_prim_new() {
        let stage = make_stage();
        let prim = stage.get_pseudo_root();
        let path = Path::absolute_root();
        let globals = make_globals();

        let ds = DataSourcePrim::new(prim.clone(), path.clone(), globals);

        assert_eq!(ds.prim().get_path(), prim.get_path());
        assert_eq!(ds.hydra_path(), &path);
    }

    #[test]
    fn test_data_source_prim_children() {
        let stage = make_stage();
        let prim = stage.get_pseudo_root();
        let path = Path::absolute_root();
        let globals = make_globals();

        let mut ds = DataSourcePrim::new(prim, path, globals);

        // Add override child
        let vis_ds = HdRetainedContainerDataSource::new_empty();
        ds.add_child(Token::new("visibility"), vis_ds);

        // Override child should be returned by get()
        let child = ds.get(&Token::new("visibility"));
        assert!(child.is_some());
    }

    #[test]
    fn test_data_source_prim_builder() {
        let stage = make_stage();
        let prim = stage.get_pseudo_root();
        let path = Path::absolute_root();
        let globals = make_globals();

        let vis_ds = HdRetainedContainerDataSource::new_empty();

        let ds = DataSourcePrimBuilder::new(prim, path, globals)
            .add(Token::new("visibility"), vis_ds)
            .build();

        assert!(ds.get(&Token::new("visibility")).is_some());
    }

    #[test]
    fn test_data_source_prim_builder_handle() {
        let stage = make_stage();
        let prim = stage.get_pseudo_root();
        let path = Path::absolute_root();
        let globals = make_globals();

        let _ds_handle = DataSourcePrimBuilder::new(prim, path, globals).build_handle();
    }

    #[test]
    fn test_get_always_returns_prim_info() {
        let stage = make_stage();
        let prim = stage.get_pseudo_root();
        let path = Path::absolute_root();
        let globals = make_globals();

        let ds = DataSourcePrim::new(prim, path, globals);

        // __usdPrimInfo and primOrigin should always be available
        let info = ds.get(&Token::new("__usdPrimInfo"));
        assert!(info.is_some(), "get('__usdPrimInfo') must return Some");

        let origin = ds.get(&Token::new("primOrigin"));
        assert!(origin.is_some(), "get('primOrigin') must return Some");
    }

    #[test]
    fn test_get_names_includes_base_names() {
        let stage = make_stage();
        let prim = stage.get_pseudo_root();
        let path = Path::absolute_root();
        let globals = make_globals();

        let ds = DataSourcePrim::new(prim, path, globals);
        let names = ds.get_names();

        // Base names always present
        assert!(names.contains(&Token::new("__usdPrimInfo")));
        assert!(names.contains(&Token::new("primOrigin")));
        assert!(names.contains(&Token::new("primvars")));
    }

    #[test]
    fn test_get_returns_none_for_unknown() {
        let stage = make_stage();
        let prim = stage.get_pseudo_root();
        let path = Path::absolute_root();
        let globals = make_globals();

        let ds = DataSourcePrim::new(prim, path, globals);
        assert!(ds.get(&Token::new("nonexistent")).is_none());
    }

    #[test]
    fn test_visibility_data_source_invisible() {
        let stage = make_stage();
        let globals = make_globals();

        // Create a Mesh prim with visibility set to "invisible"
        let prim_path = Path::from_string("/TestMesh").unwrap();
        stage
            .define_prim(prim_path.as_str(), "Mesh")
            .expect("define prim");

        if let Some(prim) = stage.get_prim_at_path(&prim_path) {
            let imageable = usd_geom::imageable::Imageable::new(prim.clone());
            let vis_attr = imageable.create_visibility_attr();
            vis_attr.set(
                Value::from(Token::new("invisible")),
                usd_sdf::TimeCode::default_time(), // NaN = set default value (not time sample)
            );

            let vis_ds = DataSourceVisibility::new(prim, globals);

            // Should return a boolean false data source
            let result = vis_ds.get(&Token::new("visibility"));
            assert!(result.is_some(), "invisible prim should return Some");
        }
    }

    #[test]
    fn test_purpose_data_source() {
        let stage = make_stage();
        let globals = make_globals();

        let prim_path = Path::from_string("/TestMesh").unwrap();
        stage
            .define_prim(prim_path.as_str(), "Mesh")
            .expect("define prim");

        if let Some(prim) = stage.get_prim_at_path(&prim_path) {
            let purpose_ds = DataSourcePurpose::new(prim, globals);
            let names = purpose_ds.get_names();
            assert_eq!(names.len(), 3);
            assert!(names.contains(&Token::new("purpose")));
            assert!(names.contains(&Token::new("inheritable")));
            assert!(names.contains(&Token::new("fallback")));
        }
    }

    #[test]
    fn test_xform_data_source_names() {
        let stage = make_stage();
        let globals = make_globals();

        let prim_path = Path::from_string("/TestXform").unwrap();
        stage
            .define_prim(prim_path.as_str(), "Xform")
            .expect("define prim");

        if let Some(prim) = stage.get_prim_at_path(&prim_path) {
            let xform_ds = DataSourceXform::new(prim, prim_path.clone(), globals);
            let names = xform_ds.get_names();
            assert!(names.contains(&Token::new("matrix")));
            assert!(names.contains(&Token::new("resetXformStack")));
        }
    }

    #[test]
    fn test_xform_flags_only_locally_time_varying_prim() {
        let stage = make_stage();
        let root_path = Path::from_string("/Root").unwrap();
        let child_path = Path::from_string("/Root/Child").unwrap();
        let root = stage
            .define_prim(root_path.as_str(), "Xform")
            .expect("define root");
        let child = stage
            .define_prim(child_path.as_str(), "Xform")
            .expect("define child");

        let translate_op = usd_geom::xformable::Xformable::new(root.clone()).add_translate_op(
            usd_geom::XformOpPrecision::Double,
            None,
            false,
        );
        translate_op.set(
            usd_gf::Vec3d::new(0.0, 0.0, 0.0),
            usd_vt::TimeCode::new(1.0),
        );
        translate_op.set(
            usd_gf::Vec3d::new(1.0, 0.0, 0.0),
            usd_vt::TimeCode::new(2.0),
        );

        let globals_impl = Arc::new(RecordingStageGlobals::default());
        let globals: DataSourceStageGlobalsHandle = globals_impl.clone();

        let _root_xform = DataSourceXform::new(root, root_path.clone(), globals.clone());
        let _child_xform = DataSourceXform::new(child, child_path.clone(), globals);

        let flagged = globals_impl.flagged.lock().expect("recording lock");
        assert_eq!(flagged.len(), 1);
        assert_eq!(flagged[0].0, root_path);
        assert_eq!(
            flagged[0].1,
            HdDataSourceLocator::from_token(tokens::XFORM.clone())
        );
    }

    #[test]
    fn test_extent_data_source_names() {
        let stage = make_stage();
        let globals = make_globals();

        let prim_path = Path::from_string("/TestMesh").unwrap();
        stage
            .define_prim(prim_path.as_str(), "Mesh")
            .expect("define prim");

        if let Some(prim) = stage.get_prim_at_path(&prim_path) {
            let extent_ds = DataSourceExtent::new(prim, globals);
            // Without authored extent, names should be empty
            let names = extent_ds.get_names();
            assert!(names.is_empty(), "No authored extent -> empty names");
        }
    }

    #[test]
    fn test_prim_origin_returns_path() {
        let stage = make_stage();

        let prim_path = Path::from_string("/TestPrim").unwrap();
        stage
            .define_prim(prim_path.as_str(), "Xform")
            .expect("define prim");

        if let Some(prim) = stage.get_prim_at_path(&prim_path) {
            let origin_ds = DataSourcePrimOrigin::new(prim);
            let names = origin_ds.get_names();
            assert!(names.contains(&Token::new("scenePath")));

            let scene_path = origin_ds.get(&Token::new("scenePath"));
            assert!(scene_path.is_some(), "scenePath should be returned");
        }
    }

    #[test]
    fn test_data_source_prim_get_xform_for_xformable() {
        let stage = make_stage();
        let globals = make_globals();

        let prim_path = Path::from_string("/TestXform").unwrap();
        stage
            .define_prim(prim_path.as_str(), "Xform")
            .expect("define prim");

        if let Some(prim) = stage.get_prim_at_path(&prim_path) {
            let ds = DataSourcePrim::new(prim, prim_path, globals);
            let names = ds.get_names();

            // Xform prim should have xform, visibility, purpose in names
            assert!(names.contains(&Token::new("xform")));
            assert!(names.contains(&Token::new("visibility")));
            assert!(names.contains(&Token::new("purpose")));
        }
    }

    #[test]
    fn test_invalidate_xform_properties() {
        let locators = DataSourcePrim::invalidate(
            &Prim::invalid(),
            &Token::new(""),
            &[Token::new("xformOpOrder"), Token::new("xformOp:translate")],
            PropertyInvalidationType::PropertyChanged,
        );
        assert!(!locators.is_empty());
    }

    #[test]
    fn test_invalidate_primvar_properties() {
        let locators = DataSourcePrim::invalidate(
            &Prim::invalid(),
            &Token::new(""),
            &[Token::new("primvars:displayColor")],
            PropertyInvalidationType::PropertyChanged,
        );
        assert!(!locators.is_empty());

        let locators_resync = DataSourcePrim::invalidate(
            &Prim::invalid(),
            &Token::new(""),
            &[Token::new("primvars:displayColor")],
            PropertyInvalidationType::Resync,
        );
        assert!(!locators_resync.is_empty());
    }
}
