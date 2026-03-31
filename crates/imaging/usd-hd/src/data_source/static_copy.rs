//! Static copy of data sources.
//!
//! Port of HdMakeStaticCopy from pxr/imaging/hd/retainedDataSource.cpp.
//! Creates a deep copy of a data source that snapshot current values,
//! disconnecting from any live backing data.

use super::retained::{
    HdOverlayContainerDataSource, HdRetainedContainerDataSource, HdRetainedSampledDataSource,
    HdRetainedSmallVectorDataSource,
};
use super::vector::HdVectorDataSource;
// UsdImagingRerootingContainerDataSource is intentionally gated behind the `usd-imaging` feature
// to break a circular dependency: usd-hd -> usd-imaging -> usd-hd.
// Enable by adding `usd-imaging` to the feature flags in Cargo.toml when the crate
// reorganization allows usd-imaging to depend on usd-hd without a cycle.
use std::collections::HashMap;
use std::sync::Arc;
#[cfg(feature = "usd-imaging")]
use usd_imaging::rerooting_container_data_source::UsdImagingRerootingContainerDataSource;

use super::base::HdDataSourceBaseHandle;
use super::container::HdContainerDataSourceHandle;

/// Creates a static (deep) copy of a data source.
///
/// Recursively copies container hierarchy, vector elements, and samples
/// sampled data sources at t=0 into retained storage. The result is
/// disconnected from any live scene data.
///
/// Port of HdMakeStaticCopy.
///
/// # Returns
///
/// - `Some(handle)` - Successfully created static copy
/// - `None` - Unsupported or invalid data source type
pub fn hd_make_static_copy(base: &HdDataSourceBaseHandle) -> Option<HdDataSourceBaseHandle> {
    let any = base.as_any();

    if let Some(c) = any.downcast_ref::<HdRetainedContainerDataSource>() {
        let handle: HdContainerDataSourceHandle = Arc::new(c.clone());
        return Some(make_static_copy_container(&handle));
    }
    if let Some(c) = any.downcast_ref::<HdOverlayContainerDataSource>() {
        let handle: HdContainerDataSourceHandle = Arc::new(c.clone());
        return Some(make_static_copy_container(&handle));
    }
    #[cfg(feature = "usd-imaging")]
    if let Some(c) = any.downcast_ref::<UsdImagingRerootingContainerDataSource>() {
        let handle: HdContainerDataSourceHandle = Arc::new(c.clone());
        return Some(make_static_copy_container(&handle));
    }

    if let Some(val) = base.sample_at_zero() {
        return Some(HdRetainedSampledDataSource::new(val) as HdDataSourceBaseHandle);
    }

    if let Some(vec) = any.downcast_ref::<HdRetainedSmallVectorDataSource>() {
        return Some(make_static_copy_vector_from_slice(vec));
    }

    None
}

fn make_static_copy_container(container: &HdContainerDataSourceHandle) -> HdDataSourceBaseHandle {
    let names = container.get_names();
    let mut children = HashMap::new();
    for name in &names {
        if let Some(child) = container.get(name) {
            if let Some(static_child) = hd_make_static_copy(&child) {
                children.insert(name.clone(), static_child);
            }
        }
    }
    HdRetainedContainerDataSource::new(children) as HdDataSourceBaseHandle
}

fn make_static_copy_vector_from_slice(
    vector: &HdRetainedSmallVectorDataSource,
) -> HdDataSourceBaseHandle {
    let n = vector.get_num_elements();
    let mut elements = Vec::with_capacity(n);
    for i in 0..n {
        if let Some(el) = vector.get_element(i) {
            if let Some(static_el) = hd_make_static_copy(&el) {
                elements.push(static_el);
            }
        }
    }
    HdRetainedSmallVectorDataSource::new(&elements) as HdDataSourceBaseHandle
}
