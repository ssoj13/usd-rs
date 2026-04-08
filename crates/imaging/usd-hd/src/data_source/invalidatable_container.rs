//! HdInvalidatableContainerDataSource - Container with invalidation support.
//!
//! Corresponds to pxr/imaging/hd/invalidatableContainerDataSource.h.

use super::HdContainerDataSource;
use super::locator::HdDataSourceLocatorSet;

/// Base trait for container data sources that cache data and support invalidation.
///
/// Corresponds to C++ `HdInvalidatableContainerDataSource`.
pub trait HdInvalidatableContainerDataSource: HdContainerDataSource + Send + Sync {
    /// Invalidate cached data for the given locators.
    fn invalidate(&self, dirty_locators: &HdDataSourceLocatorSet) -> bool;
}
