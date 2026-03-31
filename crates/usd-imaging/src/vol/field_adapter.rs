//! Base adapter trait for volume field primitives.
//!
//! This module provides the `FieldAdapter` trait which serves as the base
//! abstraction for imaging adapters that handle volume field data.
//!
//! # C++ Reference
//!
//! Analogous to `UsdImagingFieldAdapter` from `pxr/usdImaging/usdImaging/fieldAdapter.h`

use std::sync::Arc;
use usd_core::Prim;
use usd_hd::data_source::HdContainerDataSourceHandle;
use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_tf::Token;
use usd_vt::Value;

/// Invalidation type for properties on imaging subprims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyInvalidationType {
    /// Property value changed
    Resync,
    /// Property metadata changed
    DirtyBits,
}

/// Set of data source locators for invalidation.
///
/// Placeholder for HdDataSourceLocatorSet from Hydra.
pub type DataSourceLocatorSet = Vec<Token>;

/// Base trait for volume field adapters.
///
/// Field adapters handle the translation of USD field primitives
/// (OpenVDBAsset, Field3DAsset) into Hydra imaging primitives.
pub trait FieldAdapter: Send + Sync {
    /// Returns the list of imaging subprims for a given USD prim.
    ///
    /// Typically returns a single empty token for the prim itself.
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token>;

    /// Returns the imaging subprim type for a given USD prim and subprim.
    ///
    /// Returns the token identifying the type (e.g., "field3dAsset", "openvdbAsset").
    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Option<Token>;

    /// Returns the imaging subprim data source for a given USD prim.
    ///
    /// This provides the container data source that scene indices will query
    /// for field properties.
    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        scene_index_path: &Path,
    ) -> Option<HdContainerDataSourceHandle>;

    /// Returns the set of locators to invalidate for property changes.
    ///
    /// Called when USD properties change to determine what imaging data
    /// needs to be updated.
    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> DataSourceLocatorSet;

    /// Gets a value for a specific key at a given time.
    ///
    /// Legacy API for retrieving field properties. Returns the value for
    /// attributes like filePath, fieldName, fieldIndex, etc.
    fn get(&self, prim: &Prim, cache_path: &Path, key: &Token, time: TimeCode) -> Option<Value>;

    /// Returns the prim type token this adapter handles.
    ///
    /// e.g., "Field3DAsset" or "OpenVDBAsset"
    fn get_prim_type_token(&self) -> Token;
}

/// Type alias for field adapter handles.
pub type FieldAdapterHandle = Arc<dyn FieldAdapter>;
