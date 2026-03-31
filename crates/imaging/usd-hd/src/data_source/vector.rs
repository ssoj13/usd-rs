//! Vector data source - indexed arrays of data sources.

use super::base::{HdDataSourceBase, HdDataSourceBaseHandle};
use std::sync::Arc;

/// A data source representing indexed data.
///
/// Vector data sources provide array-like access to multiple data sources.
/// This should be used when a scene index needs to manipulate indexing.
/// For simple array-valued data (like a list of numbers), use
/// [`HdSampledDataSource`](super::HdSampledDataSource) with a `VtArray` instead.
///
/// # Thread Safety
///
/// All methods must be thread-safe.
///
/// # Use Cases
///
/// - Instancer prototype indices
/// - Subsets and groups
/// - Varying-length data that needs scene index manipulation
///
/// # Examples
///
/// ```
/// use usd_hd::data_source::*;
///
/// // Access elements by index
/// // let vector: Arc<dyn HdVectorDataSource> = ...;
/// // let count = vector.get_num_elements();
/// // for i in 0..count {
/// //     let element = vector.get_element(i);
/// // }
/// ```
pub trait HdVectorDataSource: HdDataSourceBase {
    /// Returns the number of elements.
    ///
    /// This must be thread-safe.
    fn get_num_elements(&self) -> usize;

    /// Returns the element at the given index.
    ///
    /// Returns `None` if index is out of bounds. For valid indices
    /// (0..get_num_elements()), this should return `Some`.
    ///
    /// This must be thread-safe.
    ///
    /// # Arguments
    ///
    /// * `element` - Zero-based index
    fn get_element(&self, element: usize) -> Option<HdDataSourceBaseHandle>;
}

/// Handle to a vector data source.
pub type HdVectorDataSourceHandle = Arc<dyn HdVectorDataSource>;

/// Attempts to cast a base data source handle to a vector.
///
/// Returns `None` if the cast fails.
/// Uses the concrete HdRetainedSmallVectorDataSource type for the downcast.
pub fn cast_to_vector(base: &HdDataSourceBaseHandle) -> Option<HdVectorDataSourceHandle> {
    if let Some(vector) = base.as_vector() {
        return Some(vector);
    }

    use super::retained::HdRetainedSmallVectorDataSource;

    let any_ref = base.as_any();
    any_ref
        .downcast_ref::<HdRetainedSmallVectorDataSource>()
        .map(|retained| Arc::new(retained.clone()) as HdVectorDataSourceHandle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::retained::HdRetainedSmallVectorDataSource;
    use std::sync::Arc;

    #[derive(Debug, Clone)]
    struct TestVectorDataSource;

    impl HdDataSourceBase for TestVectorDataSource {
        fn clone_box(&self) -> HdDataSourceBaseHandle {
            Arc::new(self.clone())
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_vector(&self) -> Option<HdVectorDataSourceHandle> {
            Some(Arc::new(self.clone()))
        }
    }

    impl HdVectorDataSource for TestVectorDataSource {
        fn get_num_elements(&self) -> usize {
            0
        }

        fn get_element(&self, _element: usize) -> Option<HdDataSourceBaseHandle> {
            None
        }
    }

    #[test]
    fn test_empty_vector() {
        let vec_ds = HdRetainedSmallVectorDataSource::new(&[]);
        assert_eq!(vec_ds.get_num_elements(), 0);
        assert!(vec_ds.get_element(0).is_none());
    }

    #[test]
    fn test_vector_trait() {
        let elements = vec![];
        let vec_ds = HdRetainedSmallVectorDataSource::new(&elements);
        assert_eq!(vec_ds.get_num_elements(), 0);
    }

    #[test]
    fn test_cast_to_vector_uses_base_interface() {
        let ds: HdDataSourceBaseHandle = Arc::new(TestVectorDataSource);
        let vector = cast_to_vector(&ds).expect("custom vector data source should cast");
        assert_eq!(vector.get_num_elements(), 0);
    }
}
