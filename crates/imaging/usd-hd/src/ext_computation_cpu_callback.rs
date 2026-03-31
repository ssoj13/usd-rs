//! HdExtComputationCpuCallback - CPU callback for ext computations.
//!
//! Port of pxr/imaging/hd/extComputationCpuCallback.h
//!
//! Callback that fills ext computation outputs given input values.

use super::ext_computation_context::HdExtComputationContext;
use std::fmt;
use std::sync::Arc;

/// Callback for an ext computation that fills outputs given input values.
///
/// Matches C++ `HdExtComputationCpuCallback`.
pub trait HdExtComputationCpuCallback: Send + Sync {
    /// Run the computation.
    fn compute(&self, ctx: &mut dyn HdExtComputationContext);
}

/// Handle type for CPU callback (Arc<dyn HdExtComputationCpuCallback>).
///
/// Matches C++ `HdExtComputationCpuCallbackSharedPtr`.
pub type HdExtComputationCpuCallbackHandle = Arc<dyn HdExtComputationCpuCallback>;

/// Wrapper to allow Value storage of CPU callback handle.
///
/// Arc<dyn Trait> cannot easily impl PartialEq/Hash; this wrapper provides
/// pointer-equality semantics for Value storage.
#[derive(Clone)]
pub struct HdExtComputationCpuCallbackValue(Arc<dyn HdExtComputationCpuCallback>);

impl HdExtComputationCpuCallbackValue {
    /// Wraps a CPU callback for Value storage.
    pub fn new(cb: Arc<dyn HdExtComputationCpuCallback>) -> Self {
        Self(cb)
    }
    /// Returns the underlying callback handle.
    pub fn get(&self) -> &Arc<dyn HdExtComputationCpuCallback> {
        &self.0
    }
}

impl PartialEq for HdExtComputationCpuCallbackValue {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for HdExtComputationCpuCallbackValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HdExtComputationCpuCallbackHandle")
    }
}

impl From<HdExtComputationCpuCallbackHandle> for HdExtComputationCpuCallbackValue {
    fn from(h: HdExtComputationCpuCallbackHandle) -> Self {
        Self(h)
    }
}

// Value::from for HdRetainedTypedSampledDataSource::get_value
impl From<HdExtComputationCpuCallbackValue> for usd_vt::Value {
    fn from(v: HdExtComputationCpuCallbackValue) -> Self {
        Self::from_no_hash(v)
    }
}
