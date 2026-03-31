
//! Rerooting container data source - path prefix replacement.
//!
//! Port of pxr/usdImaging/usdImaging/rerootingContainerDataSource.h/cpp
//!
//! Wraps an input container and recursively replaces path prefixes in path
//! and path-array data sources. Used for binding copy path translation
//! when propagating prototypes under instancers.

use std::sync::Arc;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdRetainedSmallVectorDataSource, HdRetainedTypedSampledDataSource, cast_to_container,
};
use usd_hd::{HdTypedSampledDataSource, HdVectorDataSource};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Rerooting container that replaces path prefixes in nested data sources.
///
/// For any path or path-array data source encountered recursively,
/// applies ReplacePrefix(src_prefix, dst_prefix).
///
/// Port of UsdImagingRerootingContainerDataSource.
#[derive(Clone)]
pub struct UsdImagingRerootingContainerDataSource {
    input: HdContainerDataSourceHandle,
    src_prefix: SdfPath,
    dst_prefix: SdfPath,
}

impl UsdImagingRerootingContainerDataSource {
    /// Create a rerooting container.
    ///
    /// * `input` - The input container to wrap
    /// * `src_prefix` - Path prefix to replace (e.g. instance path)
    /// * `dst_prefix` - New prefix (e.g. prototype root for binding hash)
    pub fn new(
        input: HdContainerDataSourceHandle,
        src_prefix: SdfPath,
        dst_prefix: SdfPath,
    ) -> Arc<Self> {
        Arc::new(Self {
            input,
            src_prefix,
            dst_prefix,
        })
    }
}

impl std::fmt::Debug for UsdImagingRerootingContainerDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UsdImagingRerootingContainerDataSource")
            .field("src_prefix", &self.src_prefix)
            .field("dst_prefix", &self.dst_prefix)
            .finish()
    }
}

impl HdDataSourceBase for UsdImagingRerootingContainerDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(UsdImagingRerootingContainerDataSource {
            input: self.input.clone(),
            src_prefix: self.src_prefix.clone(),
            dst_prefix: self.dst_prefix.clone(),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for UsdImagingRerootingContainerDataSource {
    fn get_names(&self) -> Vec<TfToken> {
        self.input.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let child = self.input.get(name)?;
        Some(create_rerooting_data_source(
            child,
            &self.src_prefix,
            &self.dst_prefix,
        ))
    }
}

/// Create a data source that applies path prefix replacement.
///
/// Recursively wraps containers, vectors, path, and path-array data sources.
fn create_rerooting_data_source(
    input: HdDataSourceBaseHandle,
    src_prefix: &SdfPath,
    dst_prefix: &SdfPath,
) -> HdDataSourceBaseHandle {
    // Try container (cast_to_container works for HdRetainedContainerDataSource; others pass through)
    if let Some(cont) = cast_to_container(&input) {
        let rerooted = UsdImagingRerootingContainerDataSource::new(
            cont,
            src_prefix.clone(),
            dst_prefix.clone(),
        );
        return rerooted as HdDataSourceBaseHandle;
    }

    // Try path (HdRetainedTypedSampledDataSource<SdfPath>)
    let any = input.as_ref().as_any();
    if let Some(path_ds) = any.downcast_ref::<HdRetainedTypedSampledDataSource<SdfPath>>() {
        let path = path_ds.get_typed_value(0.0);
        let replaced = path.replace_prefix(src_prefix, dst_prefix).unwrap_or(path);
        return HdRetainedTypedSampledDataSource::new(replaced) as HdDataSourceBaseHandle;
    }

    // Try path array (HdRetainedTypedSampledDataSource<Vec<SdfPath>>)
    if let Some(arr_ds) = any.downcast_ref::<HdRetainedTypedSampledDataSource<Vec<SdfPath>>>() {
        let paths = arr_ds.get_typed_value(0.0);
        let replaced: Vec<SdfPath> = paths
            .into_iter()
            .map(|p: SdfPath| p.replace_prefix(src_prefix, dst_prefix).unwrap_or(p))
            .collect();
        return HdRetainedTypedSampledDataSource::new(replaced) as HdDataSourceBaseHandle;
    }

    // Try vector (HdRetainedSmallVectorDataSource)
    if let Some(vec_ds) = any.downcast_ref::<HdRetainedSmallVectorDataSource>() {
        let n = vec_ds.get_num_elements();
        let elements: Vec<HdDataSourceBaseHandle> = (0..n)
            .filter_map(|i| {
                vec_ds
                    .get_element(i)
                    .map(|e| create_rerooting_data_source(e, src_prefix, dst_prefix))
            })
            .collect();
        return HdRetainedSmallVectorDataSource::new(&elements) as HdDataSourceBaseHandle;
    }

    // Pass through unchanged
    input
}
