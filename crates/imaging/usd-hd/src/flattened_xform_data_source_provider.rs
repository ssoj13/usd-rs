
//! HdFlattenedXformDataSourceProvider - Flattens xform by concatenating parent * local matrices.
//!
//! Port of pxr/imaging/hd/flattenedXformDataSourceProvider.cpp.
//!
//! When resetXformStack is true, the local transform is used as-is.
//! Otherwise, the flattened result is local * parent (row-vector convention).

use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocatorSet, HdRetainedTypedSampledDataSource, HdSampledDataSource,
    HdSampledDataSourceTime, HdTypedSampledDataSource,
};
use crate::flattened_data_source_provider::{
    HdFlattenedDataSourceProvider, HdFlattenedDataSourceProviderContext,
};
use crate::flo_debug::flo_debug_enabled;
use crate::schema::{HdMatrixDataSourceHandle, HdXformSchema};
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use usd_gf::Matrix4d;
use usd_vt::Value;

static DEBUG_PARENT_MATRIX_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_PARENT_MATRIX_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static DEBUG_MATRIX_CACHE_HITS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_MATRIX_CACHE_MISSES: AtomicUsize = AtomicUsize::new(0);
static DEBUG_TYPED_VALUE_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_TYPED_VALUE_CACHE_HITS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_TYPED_VALUE_CACHE_MISSES: AtomicUsize = AtomicUsize::new(0);
static DEBUG_TYPED_VALUE_TOTAL_NS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Default)]
pub struct DebugFlattenedXformStats {
    pub parent_matrix_calls: usize,
    pub parent_matrix_total_ns: u64,
    pub matrix_cache_hits: usize,
    pub matrix_cache_misses: usize,
    pub typed_value_calls: usize,
    pub typed_value_cache_hits: usize,
    pub typed_value_cache_misses: usize,
    pub typed_value_total_ns: u64,
}

pub fn reset_debug_flattened_xform_stats() {
    DEBUG_PARENT_MATRIX_CALLS.store(0, Ordering::Relaxed);
    DEBUG_PARENT_MATRIX_TOTAL_NS.store(0, Ordering::Relaxed);
    DEBUG_MATRIX_CACHE_HITS.store(0, Ordering::Relaxed);
    DEBUG_MATRIX_CACHE_MISSES.store(0, Ordering::Relaxed);
    DEBUG_TYPED_VALUE_CALLS.store(0, Ordering::Relaxed);
    DEBUG_TYPED_VALUE_CACHE_HITS.store(0, Ordering::Relaxed);
    DEBUG_TYPED_VALUE_CACHE_MISSES.store(0, Ordering::Relaxed);
    DEBUG_TYPED_VALUE_TOTAL_NS.store(0, Ordering::Relaxed);
}

pub fn read_debug_flattened_xform_stats() -> DebugFlattenedXformStats {
    DebugFlattenedXformStats {
        parent_matrix_calls: DEBUG_PARENT_MATRIX_CALLS.load(Ordering::Relaxed),
        parent_matrix_total_ns: DEBUG_PARENT_MATRIX_TOTAL_NS.load(Ordering::Relaxed),
        matrix_cache_hits: DEBUG_MATRIX_CACHE_HITS.load(Ordering::Relaxed),
        matrix_cache_misses: DEBUG_MATRIX_CACHE_MISSES.load(Ordering::Relaxed),
        typed_value_calls: DEBUG_TYPED_VALUE_CALLS.load(Ordering::Relaxed),
        typed_value_cache_hits: DEBUG_TYPED_VALUE_CACHE_HITS.load(Ordering::Relaxed),
        typed_value_cache_misses: DEBUG_TYPED_VALUE_CACHE_MISSES.load(Ordering::Relaxed),
        typed_value_total_ns: DEBUG_TYPED_VALUE_TOTAL_NS.load(Ordering::Relaxed),
    }
}

fn debug_flo_dirty_enabled() -> bool {
    flo_debug_enabled()
}

static IDENTITY_XFORM: Lazy<HdContainerDataSourceHandle> = Lazy::new(|| {
    HdXformSchema::build_retained(
        Some(HdRetainedTypedSampledDataSource::new(Matrix4d::identity())),
        Some(HdRetainedTypedSampledDataSource::new(true)),
    )
});

#[derive(Clone)]
struct MatrixCombinerDataSource {
    parent: HdMatrixDataSourceHandle,
    local: HdMatrixDataSourceHandle,
    /// C++ _cachedResultAt0: precomputed flattened matrix at shutter offset 0.
    /// Avoids recursive parent chain traversal on every get_typed_value(0) call.
    cached_result_at_0: Matrix4d,
}

impl MatrixCombinerDataSource {
    fn new(parent: HdMatrixDataSourceHandle, local: HdMatrixDataSourceHandle) -> Arc<Self> {
        // C++ parity: cache result at time 0 in constructor.
        let cached = local.get_typed_value(0.0) * parent.get_typed_value(0.0);
        Arc::new(Self {
            parent,
            local,
            cached_result_at_0: cached,
        })
    }
}

impl std::fmt::Debug for MatrixCombinerDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MatrixCombinerDataSource").finish()
    }
}

impl HdDataSourceBase for MatrixCombinerDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<Value> {
        Some(Value::from(self.get_typed_value(0.0)))
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }

    fn as_matrix_data_source(&self) -> Option<HdMatrixDataSourceHandle> {
        Some(Arc::new(self.clone()) as HdMatrixDataSourceHandle)
    }
}

impl HdSampledDataSource for MatrixCombinerDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        Value::from(self.get_typed_value(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        let mut all_times = Vec::new();
        let mut varying = false;

        let mut parent_times = Vec::new();
        if self
            .parent
            .get_contributing_sample_times(start_time, end_time, &mut parent_times)
        {
            varying = true;
            all_times.extend(parent_times);
        }

        let mut local_times = Vec::new();
        if self
            .local
            .get_contributing_sample_times(start_time, end_time, &mut local_times)
        {
            varying = true;
            all_times.extend(local_times);
        }

        if !varying {
            out_sample_times.clear();
            return false;
        }

        all_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        all_times.dedup_by(|a, b| (*a - *b).abs() < 0.0001);
        *out_sample_times = all_times;
        true
    }
}

impl HdTypedSampledDataSource<Matrix4d> for MatrixCombinerDataSource {
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> Matrix4d {
        let debug_stats = debug_flo_dirty_enabled();
        let started = debug_stats.then(std::time::Instant::now);
        if debug_stats {
            DEBUG_TYPED_VALUE_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        // Match OpenUSD directly: cache only the shutter-0 composition.
        if shutter_offset == 0.0 {
            if debug_stats {
                DEBUG_TYPED_VALUE_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
                if let Some(started) = started {
                    DEBUG_TYPED_VALUE_TOTAL_NS
                        .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
                }
            }
            return self.cached_result_at_0;
        }
        let value =
            self.local.get_typed_value(shutter_offset) * self.parent.get_typed_value(shutter_offset);
        if debug_stats {
            if let Some(started) = started {
                DEBUG_TYPED_VALUE_TOTAL_NS
                    .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
        }
        value
    }
}

/// Provider that flattens xform data by concatenating parent and local transforms.
///
/// Corresponds to C++ HdFlattenedXformDataSourceProvider.
#[derive(Debug, Default)]
pub struct HdFlattenedXformDataSourceProvider;

impl HdFlattenedXformDataSourceProvider {
    /// Create new xform flattening provider.
    pub fn new() -> Self {
        Self
    }
}

impl HdFlattenedDataSourceProvider for HdFlattenedXformDataSourceProvider {
    fn get_flattened_data_source(
        &self,
        ctx: &HdFlattenedDataSourceProviderContext<'_>,
    ) -> Option<HdContainerDataSourceHandle> {
        // Get input xform schema (may be absent).
        let input_container = ctx.get_input_data_source();
        let input_matrix_ds = input_container
            .as_ref()
            .and_then(|c| HdXformSchema::new(c.clone()).get_matrix());

        // Check resetXformStack before composing with parent.
        if let Some(ref c) = input_container {
            let schema = HdXformSchema::new(c.clone());
            if let Some(reset_ds) = schema.get_reset_xform_stack() {
                if reset_ds.get_typed_value(0.0f32) {
                    return if schema.get_matrix().is_some() {
                        schema.get_container().cloned()
                    } else {
                        Some(IDENTITY_XFORM.clone())
                    };
                }
            }
        }

        // Get parent's flattened xform.
        let parent_container = ctx.get_flattened_data_source_from_parent_prim();
        let parent_matrix_ds = parent_container
            .as_ref()
            .and_then(|c| HdXformSchema::new(c.clone()).get_matrix());

        match (input_matrix_ds, parent_matrix_ds) {
            (None, None) => {
                // No local or parent matrix -> identity.
                Some(IDENTITY_XFORM.clone())
            }
            (None, Some(_)) => {
                // Parent only -> return parent (already flattened).
                parent_container
            }
            (Some(local_ds), None) => {
                // Local only (root prim) -> mark as fully composed.
                Some(HdXformSchema::build_retained(
                    Some(local_ds),
                    Some(HdRetainedTypedSampledDataSource::new(true)),
                ))
            }
            (Some(local_ds), Some(parent_ds)) => {
                // Match OpenUSD directly: concatenate the matrices and return a
                // fully-composed retained xform container.
                Some(HdXformSchema::build_retained(
                    Some(MatrixCombinerDataSource::new(parent_ds, local_ds)),
                    Some(HdRetainedTypedSampledDataSource::new(true)),
                ))
            }
        }
    }

    fn compute_dirty_locators_for_descendants(&self, locators: &mut HdDataSourceLocatorSet) {
        // Xform changes affect all descendants.
        *locators = HdDataSourceLocatorSet::universal();
    }
}
