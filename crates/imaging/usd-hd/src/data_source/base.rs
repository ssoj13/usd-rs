//! Base trait for all data sources.

use super::container::HdContainerDataSourceHandle;
use super::invalidatable_container::HdInvalidatableContainerDataSource;
use super::sampled::HdSampledDataSource;
use super::typed::HdTypedSampledDataSource;
use super::vector::HdVectorDataSourceHandle;
use std::fmt::Debug;
use std::sync::Arc;
use usd_gf::Matrix4d;
use usd_vt::Value;

/// Base trait for all Hydra data sources.
///
/// This is the root of the data source hierarchy. All data sources implement
/// this trait, which provides common functionality for polymorphic handling.
///
/// Data sources are the fundamental building blocks of Hydra's scene index
/// architecture, representing scene data that can be queried at various times
/// and locations in the scene hierarchy.
///
/// # Type Aliases
///
/// - `HdDataSourceBaseHandle` = `Arc<dyn HdDataSourceBase>`
///
/// # Implementations
///
/// See:
/// - [`HdContainerDataSource`](super::HdContainerDataSource) - Hierarchical named data
/// - [`HdSampledDataSource`](super::HdSampledDataSource) - Time-sampled values
/// - [`HdVectorDataSource`](super::HdVectorDataSource) - Indexed arrays
pub trait HdDataSourceBase: Send + Sync + Debug {
    /// Returns a type-erased clone of this data source.
    ///
    /// This allows cloning through trait objects.
    fn clone_box(&self) -> HdDataSourceBaseHandle;

    /// Returns a reference to Any for downcasting.
    ///
    /// This allows safe downcasting to concrete types.
    fn as_any(&self) -> &dyn std::any::Any;

    /// If this is a sampled data source, returns its value at t=0.
    ///
    /// Used by hd_make_static_copy to snapshot sampled values.
    /// Default returns None (not a sampled data source).
    fn sample_at_zero(&self) -> Option<Value> {
        None
    }

    /// If this data source also implements HdSampledDataSource, returns a reference.
    ///
    /// Used to recover the sampled interface from a type-erased handle.
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        None
    }

    /// If this data source already implements `HdTypedSampledDataSource<Matrix4d>`,
    /// return it directly.
    ///
    /// This mirrors OpenUSD's `_GetTypedDataSource<HdMatrixDataSource>()` fast
    /// path for xform access and avoids wrapping matrix sources in a
    /// `SampledToTypedAdapter<Value>` on every query.
    fn as_matrix_data_source(
        &self,
    ) -> Option<Arc<dyn HdTypedSampledDataSource<Matrix4d> + Send + Sync>> {
        None
    }

    /// If this data source also implements HdContainerDataSource, returns it as a handle.
    ///
    /// Used to recover the container interface from a type-erased `HdDataSourceBaseHandle`
    /// without requiring the caller to enumerate every concrete container type.
    /// Each concrete container type must override this to return `Some(Arc::new(...))`,
    /// because the Arc fat pointer for `dyn HdContainerDataSource` cannot be recovered
    /// from `&dyn Any` alone.
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        None
    }

    /// If this data source also implements HdInvalidatableContainerDataSource, returns it.
    ///
    /// This mirrors the C++ `HdInvalidatableContainerDataSource::Cast` path used by
    /// `HdFlatteningSceneIndex` to invalidate cached flattened containers in place.
    fn as_invalidatable_container(&self) -> Option<&dyn HdInvalidatableContainerDataSource> {
        None
    }

    /// If this data source also implements HdVectorDataSource, returns it as a handle.
    ///
    /// This is the vector analogue of `as_container()`. Without it, `cast_to_vector()`
    /// can only succeed for hard-coded concrete types and scene index wrappers that
    /// produce custom vector data sources lose the vector interface after type erasure.
    fn as_vector(&self) -> Option<HdVectorDataSourceHandle> {
        None
    }
}

/// Handle to a data source base.
///
/// This is the primary way to store and pass data sources polymorphically.
/// Uses `Arc` for efficient sharing across threads.
pub type HdDataSourceBaseHandle = Arc<dyn HdDataSourceBase>;

/// Atomic handle for thread-safe data source storage.
///
/// In Rust, `Arc` already provides atomic reference counting, so this is
/// just an alias to the standard handle type.
pub type HdDataSourceBaseAtomicHandle = HdDataSourceBaseHandle;

/// Special block data source indicating absence of data.
///
/// A block data source represents the explicit absence of data, which is
/// different from null. When composing containers, a block data source can
/// shadow underlying data, making it "not present" at a given locator.
///
/// This is useful for masking or removing data during composition without
/// needing to rebuild entire container hierarchies.
///
/// # Examples
///
/// ```
/// use usd_hd::data_source::*;
///
/// let block = HdBlockDataSource::new();
/// // This explicitly marks data as absent
/// ```
#[derive(Debug, Clone)]
pub struct HdBlockDataSource;

impl HdBlockDataSource {
    /// Creates a new block data source.
    pub fn new() -> Arc<Self> {
        Arc::new(HdBlockDataSource)
    }
}

impl Default for HdBlockDataSource {
    fn default() -> Self {
        Self::new();
        HdBlockDataSource
    }
}

impl HdDataSourceBase for HdBlockDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Helper macro to implement clone_box and as_any for data source types.
///
/// This reduces boilerplate when implementing HdDataSourceBase.
#[macro_export]
macro_rules! impl_datasource_base {
    ($type:ty) => {
        impl $crate::data_source::HdDataSourceBase for $type {
            fn clone_box(&self) -> $crate::data_source::HdDataSourceBaseHandle {
                std::sync::Arc::new(self.clone())
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    };
}

/// Helper macro for container data source types.
///
/// Like `impl_datasource_base!` but also overrides `as_container()` to return
/// `Some(Arc::new(self.clone()))`. This is required for any type that implements
/// `HdContainerDataSource` — without it, `cast_to_container()` returns `None`
/// and schema accessors (HdPrimvarsSchema, HdMeshSchema, etc.) can't find
/// nested containers in the scene index prim hierarchy.
///
/// In C++, `dynamic_cast<HdContainerDataSource*>` handles this automatically
/// via RTTI. In Rust, each container must explicitly advertise itself.
#[macro_export]
macro_rules! impl_container_datasource_base {
    ($type:ty) => {
        impl $crate::data_source::HdDataSourceBase for $type {
            fn clone_box(&self) -> $crate::data_source::HdDataSourceBaseHandle {
                std::sync::Arc::new(self.clone())
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_container(&self) -> Option<$crate::data_source::HdContainerDataSourceHandle> {
                Some(std::sync::Arc::new(self.clone()))
            }
        }
    };
}

/// Helper macro for vector data source types.
///
/// Like `impl_datasource_base!` but also overrides `as_vector()` to return
/// `Some(Arc::new(self.clone()))`. This is required for any type that implements
/// `HdVectorDataSource` — without it, `cast_to_vector()` falls back to
/// concrete-type checks and misses wrapped/custom vector data sources.
#[macro_export]
macro_rules! impl_vector_datasource_base {
    ($type:ty) => {
        impl $crate::data_source::HdDataSourceBase for $type {
            fn clone_box(&self) -> $crate::data_source::HdDataSourceBaseHandle {
                std::sync::Arc::new(self.clone())
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_vector(&self) -> Option<$crate::data_source::HdVectorDataSourceHandle> {
                Some(std::sync::Arc::new(self.clone()))
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_data_source() {
        let block = HdBlockDataSource::new();
        assert!(Arc::strong_count(&block) == 1);

        let _clone = block.clone();
        assert!(Arc::strong_count(&block) == 2);
    }

    #[test]
    fn test_block_default() {
        let _block = HdBlockDataSource::default();
    }
}
