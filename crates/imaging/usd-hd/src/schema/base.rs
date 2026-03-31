//! Base schema class for all Hydra schemas.
//!
//! Schemas represent structured views of unstructured container data sources.
//! They provide typed accessors for expected fields within a container.

use crate::data_source::HdContainerDataSourceHandle;
use std::sync::Arc;

/// Base schema class providing access to underlying container data source.
///
/// Schema classes represent structured views of the inherently unstructured
/// container data source passed into the constructor. They define what fields
/// a given object is expected to have.
///
/// Note that a schema can be applied to a container which doesn't contain
/// all of the named fields; in that case, some field accessors will return
/// None, and the caller should use default values.
///
/// # Examples
///
/// ```
/// use usd_hd::schema::HdSchema;
/// use usd_hd::data_source::HdContainerDataSourceHandle;
///
/// struct MySchema {
///     base: HdSchema,
/// }
///
/// impl MySchema {
///     pub fn new(container: HdContainerDataSourceHandle) -> Self {
///         Self {
///             base: HdSchema::new(container),
///         }
///     }
///     
///     pub fn is_defined(&self) -> bool {
///         self.base.is_defined()
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct HdSchema {
    container: Option<HdContainerDataSourceHandle>,
}

impl HdSchema {
    /// Creates a new schema wrapping the given container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            container: Some(container),
        }
    }

    /// Creates an empty schema with no container.
    pub fn empty() -> Self {
        Self { container: None }
    }

    /// Returns the underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.container.as_ref()
    }

    /// Returns true if this schema is applied on top of a non-null container.
    pub fn is_defined(&self) -> bool {
        self.container.is_some()
    }

    /// Returns a typed data source for the given field name.
    ///
    /// This is a helper used by schema implementations to retrieve
    /// typed child data sources from the container.
    pub fn get_typed<T>(&self, name: &usd_tf::Token) -> Option<Arc<T>>
    where
        T: 'static + ?Sized,
    {
        let container = self.container.as_ref()?;
        let child = container.get(name)?;

        // Try to downcast to the requested type
        let any = &child as &dyn std::any::Any;
        any.downcast_ref::<Arc<T>>().cloned()
    }

    /// Returns a typed data source handle for the given field name.
    ///
    /// Mirrors the C++ schema pattern where `_GetTypedDataSource<T>(name)` uses
    /// `dynamic_pointer_cast` to obtain a `HdTypedSampledDataSource<T>::Handle`
    /// from the container's child.
    ///
    /// In Rust we can't do cross-trait downcasting on trait objects, so this
    /// method uses a two-stage approach:
    ///
    /// 1. **Fast path**: try `downcast_ref` to the concrete
    ///    [`HdRetainedTypedSampledDataSource<U>`] — works for data sources
    ///    created via `build_retained()`.
    ///
    /// 2. **Fallback**: wrap the child's [`HdSampledDataSource`] in a
    ///    [`SampledToTypedAdapter<U>`] that extracts `U` from `Value` via
    ///    [`HdValueExtract`]. This handles attribute-backed and any other
    ///    non-retained data sources — the Rust analogue of C++
    ///    `dynamic_pointer_cast`.
    pub fn get_typed_retained<U>(
        &self,
        name: &usd_tf::Token,
    ) -> Option<Arc<dyn crate::data_source::HdTypedSampledDataSource<U> + Send + Sync>>
    where
        U: crate::data_source::HdValueExtract + std::fmt::Debug,
        crate::data_source::HdRetainedTypedSampledDataSource<U>:
            crate::data_source::HdTypedSampledDataSource<U>,
    {
        let container = self.container.as_ref()?;
        let child = container.get(name)?;

        // Fast path disabled: a filter in the scene index chain wraps
        // time-varying DataSourceXformMatrix in HdRetainedTypedSampledDataSource
        // which snapshots the value at t=0, breaking animation.
        // Always go through SampledToTypedAdapter which calls get_value()
        // on every access, respecting the current stage_globals time.
        //
        // TODO: identify which filter creates the Retained wrapper and fix
        // it to preserve the sampled data source for time-varying attributes.

        // Fallback: any data source with as_sampled() — wrap in typed adapter.
        // This is the Rust equivalent of C++ dynamic_pointer_cast from base
        // to HdTypedSampledDataSource<T>.
        if child.as_sampled().is_some() {
            return Some(crate::data_source::SampledToTypedAdapter::<U>::new(
                child.clone_box(),
            ));
        }

        None
    }
}

impl Default for HdSchema {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::HdRetainedContainerDataSource;

    #[test]
    fn test_empty_schema() {
        let schema = HdSchema::empty();
        assert!(!schema.is_defined());
        assert!(schema.get_container().is_none());
    }

    #[test]
    fn test_schema_with_container() {
        let container = HdRetainedContainerDataSource::new_empty();
        let schema = HdSchema::new(container);
        assert!(schema.is_defined());
        assert!(schema.get_container().is_some());
    }

    #[test]
    fn test_default() {
        let schema = HdSchema::default();
        assert!(!schema.is_defined());
    }
}
