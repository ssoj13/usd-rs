//! Typed sampled data source - type-safe wrapper.

use super::base::HdDataSourceBase;
use super::sampled::{HdSampledDataSource, HdSampledDataSourceTime};
use std::sync::Arc;

/// A type-safe wrapper around sampled data sources.
///
/// This trait extends `HdSampledDataSource` (C++ parity: HdTypedSampledDataSource
/// inherits from HdSampledDataSource) with strongly-typed access methods.
///
/// # Type Parameter
///
/// * `T` - The Rust type of the sampled value (must be `Clone + Send + Sync + 'static`)
///
/// # Examples
///
/// ```
/// use usd_hd::data_source::*;
///
/// // Type-safe access for known types
/// // let typed: Arc<dyn HdTypedSampledDataSource<f64>> = ...;
/// // let value: f64 = typed.get_typed_value(0.0);
/// ```
pub trait HdTypedSampledDataSource<T>:
    HdSampledDataSource + HdDataSourceBase + Send + Sync
where
    T: Clone + Send + Sync + 'static,
{
    /// Returns the typed value at the given frame-relative time.
    ///
    /// This is a type-safe alternative to `get_value()` that returns the
    /// concrete type directly without needing to extract from `Value`.
    ///
    /// # Arguments
    ///
    /// * `shutter_offset` - Time relative to current frame
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> T;
}

/// Handle to a typed sampled data source.
pub type HdTypedSampledDataSourceHandle<T> = Arc<dyn HdTypedSampledDataSource<T>>;

// Note: cast_to_typed and get_typed_value_or_extract removed
// Use downcast_ref on concrete types if needed

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::retained::HdRetainedTypedSampledDataSource;

    #[test]
    fn test_typed_i32() {
        let ds = HdRetainedTypedSampledDataSource::new(42i32);
        let value = ds.get_typed_value(0.0);
        assert_eq!(value, 42);
    }

    #[test]
    fn test_typed_f64() {
        let ds = HdRetainedTypedSampledDataSource::new(3.14f64);
        let value = ds.get_typed_value(0.0);
        assert!((value - 3.14).abs() < 0.001);
    }

    #[test]
    fn test_typed_bool() {
        let ds = HdRetainedTypedSampledDataSource::new(true);
        let value = ds.get_typed_value(0.0);
        assert!(value);
    }
}
