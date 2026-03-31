//! Transform schema for Hydra primitives.

use super::HdSchema;
use crate::data_source::HdDataSourceLocator;
use crate::data_source::{
    HdContainerDataSourceHandle, HdTypedSampledDataSource, cast_to_container,
};
use crate::flo_debug::flo_debug_enabled;
use once_cell::sync::Lazy;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use usd_gf::Matrix4d;
use usd_tf::Token;

// Schema tokens

/// Schema name token: "xform"
pub static XFORM: Lazy<Token> = Lazy::new(|| Token::new("xform"));

/// Field name token: "matrix"
pub static MATRIX: Lazy<Token> = Lazy::new(|| Token::new("matrix"));

/// Field name token: "resetXformStack"
pub static RESET_XFORM_STACK: Lazy<Token> = Lazy::new(|| Token::new("resetXformStack"));

/// Typed data source for matrix (GfMatrix4d)
pub type HdMatrixDataSource = dyn HdTypedSampledDataSource<Matrix4d> + Send + Sync;

/// Handle to matrix data source
pub type HdMatrixDataSourceHandle = Arc<HdMatrixDataSource>;

/// Typed data source for bool
pub type HdBoolDataSource = dyn HdTypedSampledDataSource<bool> + Send + Sync;

/// Handle to bool data source
pub type HdBoolDataSourceHandle = Arc<HdBoolDataSource>;

static DEBUG_GET_FROM_PARENT_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_FROM_PARENT_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static DEBUG_GET_MATRIX_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_MATRIX_DIRECT_HITS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_MATRIX_FALLBACK_HITS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_MATRIX_MISSES: AtomicUsize = AtomicUsize::new(0);
static DEBUG_GET_MATRIX_TOTAL_NS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Default)]
pub struct DebugXformSchemaStats {
    pub get_from_parent_calls: usize,
    pub get_from_parent_total_ns: u64,
    pub get_matrix_calls: usize,
    pub get_matrix_direct_hits: usize,
    pub get_matrix_fallback_hits: usize,
    pub get_matrix_misses: usize,
    pub get_matrix_total_ns: u64,
}

pub fn reset_debug_xform_schema_stats() {
    DEBUG_GET_FROM_PARENT_CALLS.store(0, Ordering::Relaxed);
    DEBUG_GET_FROM_PARENT_TOTAL_NS.store(0, Ordering::Relaxed);
    DEBUG_GET_MATRIX_CALLS.store(0, Ordering::Relaxed);
    DEBUG_GET_MATRIX_DIRECT_HITS.store(0, Ordering::Relaxed);
    DEBUG_GET_MATRIX_FALLBACK_HITS.store(0, Ordering::Relaxed);
    DEBUG_GET_MATRIX_MISSES.store(0, Ordering::Relaxed);
    DEBUG_GET_MATRIX_TOTAL_NS.store(0, Ordering::Relaxed);
}

pub fn read_debug_xform_schema_stats() -> DebugXformSchemaStats {
    DebugXformSchemaStats {
        get_from_parent_calls: DEBUG_GET_FROM_PARENT_CALLS.load(Ordering::Relaxed),
        get_from_parent_total_ns: DEBUG_GET_FROM_PARENT_TOTAL_NS.load(Ordering::Relaxed),
        get_matrix_calls: DEBUG_GET_MATRIX_CALLS.load(Ordering::Relaxed),
        get_matrix_direct_hits: DEBUG_GET_MATRIX_DIRECT_HITS.load(Ordering::Relaxed),
        get_matrix_fallback_hits: DEBUG_GET_MATRIX_FALLBACK_HITS.load(Ordering::Relaxed),
        get_matrix_misses: DEBUG_GET_MATRIX_MISSES.load(Ordering::Relaxed),
        get_matrix_total_ns: DEBUG_GET_MATRIX_TOTAL_NS.load(Ordering::Relaxed),
    }
}

fn debug_flo_dirty_enabled() -> bool {
    flo_debug_enabled()
}

/// Schema representing transform data.
///
/// Provides access to:
/// - `matrix` - 4x4 transformation matrix
/// - `resetXformStack` - flag indicating transform doesn't inherit from parent
///
/// # Location
///
/// Default locator: `xform`
#[derive(Debug, Clone)]
pub struct HdXformSchema {
    schema: HdSchema,
}

impl HdXformSchema {
    /// Create schema from container data source
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Extract xform schema from parent container
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        let debug_stats = debug_flo_dirty_enabled();
        let started = debug_stats.then(std::time::Instant::now);
        if debug_stats {
            DEBUG_GET_FROM_PARENT_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        if let Some(child) = parent.get(&XFORM) {
            if let Some(container) = cast_to_container(&child) {
                if debug_stats {
                    if let Some(started) = started {
                        DEBUG_GET_FROM_PARENT_TOTAL_NS
                            .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
                    }
                }
                return Self::new(container);
            }
        }
        if debug_stats {
            if let Some(started) = started {
                DEBUG_GET_FROM_PARENT_TOTAL_NS
                    .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Check if schema is defined (has valid container)
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get underlying container data source
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Get transformation matrix data source
    pub fn get_matrix(&self) -> Option<HdMatrixDataSourceHandle> {
        let debug_stats = debug_flo_dirty_enabled();
        let started = debug_stats.then(std::time::Instant::now);
        if debug_stats {
            DEBUG_GET_MATRIX_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        let container = self.schema.get_container()?;
        let child = container.get(&MATRIX)?;
        if let Some(matrix_ds) = child.as_matrix_data_source() {
            if debug_stats {
                DEBUG_GET_MATRIX_DIRECT_HITS.fetch_add(1, Ordering::Relaxed);
                if let Some(started) = started {
                    DEBUG_GET_MATRIX_TOTAL_NS
                        .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
                }
            }
            return Some(matrix_ds);
        }
        let result = Self::new(container.clone()).schema.get_typed_retained::<Matrix4d>(&MATRIX);
        if debug_stats {
            if result.is_some() {
                DEBUG_GET_MATRIX_FALLBACK_HITS.fetch_add(1, Ordering::Relaxed);
            } else {
                DEBUG_GET_MATRIX_MISSES.fetch_add(1, Ordering::Relaxed);
            }
            if let Some(started) = started {
                DEBUG_GET_MATRIX_TOTAL_NS
                    .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
        }
        result
    }

    /// Get reset xform stack flag data source
    ///
    /// When true, transform doesn't inherit from parent
    pub fn get_reset_xform_stack(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed_retained::<bool>(&RESET_XFORM_STACK)
    }

    /// Get schema name token
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &XFORM
    }

    /// Get default locator for xform data
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[XFORM.clone()])
    }

    /// Build retained container with xform data
    pub fn build_retained(
        matrix: Option<HdMatrixDataSourceHandle>,
        reset_xform_stack: Option<HdBoolDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(m) = matrix {
            entries.push((MATRIX.clone(), m as HdDataSourceBaseHandle));
        }
        if let Some(r) = reset_xform_stack {
            entries.push((RESET_XFORM_STACK.clone(), r as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for constructing HdXformSchema containers.
///
/// Provides fluent API for building xform schema data sources.
#[allow(dead_code)] // Ready for use when schema population is needed
#[derive(Default)]
pub struct HdXformSchemaBuilder {
    /// Optional transformation matrix
    matrix: Option<HdMatrixDataSourceHandle>,
    /// Optional reset xform stack flag
    reset_xform_stack: Option<HdBoolDataSourceHandle>,
}

#[allow(dead_code)]
impl HdXformSchemaBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set transformation matrix
    pub fn set_matrix(mut self, matrix: HdMatrixDataSourceHandle) -> Self {
        self.matrix = Some(matrix);
        self
    }

    /// Set reset xform stack flag
    pub fn set_reset_xform_stack(mut self, reset: HdBoolDataSourceHandle) -> Self {
        self.reset_xform_stack = Some(reset);
        self
    }

    /// Build container data source from configured values
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdXformSchema::build_retained(self.matrix, self.reset_xform_stack)
    }
}
