//! Container data source - hierarchical named data.

use super::base::{HdDataSourceBase, HdDataSourceBaseHandle};
use super::locator::HdDataSourceLocator;
use std::sync::Arc;
use usd_tf::Token;

/// A data source representing structured, hierarchical data.
///
/// Container data sources organize data as named children, forming a tree
/// structure similar to a filesystem or nested dictionaries. This is the
/// primary way scene data is organized in Hydra.
///
/// # Thread Safety
///
/// All methods are required to be thread-safe. Implementations should use
/// appropriate synchronization primitives (`RwLock`, `Mutex`) if needed.
///
/// # Examples
///
/// ```
/// use usd_hd::data_source::*;
/// use usd_tf::Token;
///
/// // Access child data source by name
/// // let container: Arc<dyn HdContainerDataSource> = ...;
/// // let child = container.get(&Token::new("points"));
/// ```
pub trait HdContainerDataSource: HdDataSourceBase {
    /// Returns the list of child names.
    ///
    /// This returns all names for which `get()` is expected to return
    /// a non-null value. This must be thread-safe.
    fn get_names(&self) -> Vec<Token>;

    /// Returns the child data source with the given name.
    ///
    /// Returns `None` if no child exists with that name. This must be
    /// thread-safe.
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle>;
}

/// Handle to a container data source.
pub type HdContainerDataSourceHandle = Arc<dyn HdContainerDataSource>;

/// Helper function to traverse a locator path in a container.
///
/// Given a container and a locator, returns the descendant data source
/// identified by the locator path. Returns `None` if the path doesn't exist.
///
/// # Examples
///
/// ```
/// use usd_hd::data_source::*;
/// use usd_tf::Token;
///
/// // Navigate nested containers
/// // let root: HdContainerDataSourceHandle = ...;
/// // let locator = HdDataSourceLocator::new(&[
/// //     Token::new("primvars"),
/// //     Token::new("points"),
/// // ]);
/// // let points_ds = hd_container_get(root, &locator);
/// ```
pub fn hd_container_get(
    container: HdContainerDataSourceHandle,
    locator: &HdDataSourceLocator,
) -> Option<HdDataSourceBaseHandle> {
    if locator.is_empty() {
        return Some(container as HdDataSourceBaseHandle);
    }

    let mut current = container as HdDataSourceBaseHandle;

    for element in locator.elements() {
        // Use as_container() to recover the container interface from the erased handle.
        // Direct downcast to Arc<dyn HdContainerDataSource> is impossible because the fat
        // pointer layout differs from Arc<dyn HdDataSourceBase>.
        if let Some(cont) = current.as_container() {
            match cont.get(element) {
                Some(child) => current = child,
                None => return None,
            }
        } else {
            // Not a container, can't continue
            return None;
        }
    }

    Some(current)
}

/// Attempts to cast a base data source handle to a container.
///
/// Delegates to `HdDataSourceBase::as_container()`, which every container type
/// must override. This avoids the impossible `Arc<dyn Trait>` downcast problem:
/// the fat pointer for `dyn HdContainerDataSource` differs from `dyn HdDataSourceBase`,
/// so `as_any().downcast_ref()` cannot recover an `Arc<dyn HdContainerDataSource>`.
///
/// Returns `None` if the source does not implement `HdContainerDataSource`.
pub fn cast_to_container(base: &HdDataSourceBaseHandle) -> Option<HdContainerDataSourceHandle> {
    base.as_container()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::retained::HdRetainedContainerDataSource;

    #[test]
    fn test_empty_locator() {
        let container = HdRetainedContainerDataSource::new_empty();
        let locator = HdDataSourceLocator::empty();

        let result = hd_container_get(container.clone(), &locator);
        assert!(result.is_some());
    }

    #[test]
    fn test_container_trait() {
        let container = HdRetainedContainerDataSource::new_empty();
        let names = container.get_names();
        assert_eq!(names.len(), 0);
    }
}
