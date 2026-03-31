//! UsdVolImaging - Volume imaging adapters for USD.
//!
//! This module provides imaging adapters that translate USD volume primitives
//! into Hydra representations for rendering volumetric data.
//!
//! # Overview
//!
//! UsdVolImaging handles the conversion of volume field primitives from USD's
//! scene description into data sources consumable by Hydra render delegates.
//! It supports two primary field asset types:
//!
//! - **Field3DAsset** - Field3D format volumetric data
//! - **OpenVDBAsset** - OpenVDB format volumetric data
//!
//! # Architecture
//!
//! The module uses an adapter pattern where each field type has a dedicated
//! adapter implementing the [`FieldAdapter`] trait. These adapters:
//!
//! 1. Identify imaging subprims for a given USD prim
//! 2. Provide data sources for scene index queries
//! 3. Handle property invalidation for incremental updates
//! 4. Access field attributes (filePath, fieldName, etc.)
//!
//! # Field Adapters
//!
//! - [`Field3DAssetAdapter`] - Handles UsdVolField3DAsset primitives
//! - [`OpenVDBAssetAdapter`] - Handles UsdVolOpenVDBAsset primitives
//!
//! Both adapters expose field properties through container data sources
//! that integrate with Hydra's scene index system.
//!
//! # Data Sources
//!
//! - [`DataSourceFieldAsset`] - Container for field asset attributes
//! - [`DataSourceFieldAssetPrim`] - Prim-level data source with volumeField container
//!
//! # Tokens
//!
//! The module defines imaging-specific tokens in [`tokens`]:
//! - `field3dAsset` - Imaging type for Field3D
//! - `openvdbAsset` - Imaging type for OpenVDB
//!
//! # Examples
//!
//! ```ignore
//! use usd_imaging::vol::*;
//! use usd_tf::Token;
//!
//! // Create an OpenVDB adapter
//! let adapter = OpenVDBAssetAdapter::new();
//!
//! // Get the prim type it handles
//! let prim_type = adapter.get_prim_type_token();
//! assert_eq!(prim_type.as_str(), "OpenVDBAsset");
//! ```
//!
//! # C++ Reference
//!
//! Port of `pxr/usdImaging/usdVolImaging/` module from OpenUSD.
//!
//! ## Differences from C++
//!
//! - Uses trait-based `FieldAdapter` instead of base class inheritance
//! - Simplified data source implementation (no UsdImaging base infrastructure yet)
//! - Direct Token usage instead of TfToken macros
//!
//! # Implementation Status
//!
//! - [x] Tokens (field3dAsset, openvdbAsset)
//! - [x] FieldAdapter trait
//! - [x] Field3DAssetAdapter
//! - [x] OpenVDBAssetAdapter
//! - [x] DataSourceFieldAsset
//! - [x] DataSourceFieldAssetPrim
//! - [ ] Full UsdImaging base adapter infrastructure
//! - [ ] Plugin registration system
//! - [ ] Complete attribute data source wrappers

mod data_source_field_asset;
mod field3d_asset_adapter;
mod field_adapter;
mod openvdb_asset_adapter;
mod tokens;

#[cfg(test)]
mod tests;

// Public exports - Tokens
pub use tokens::{USD_VOL_IMAGING_TOKENS, UsdVolImagingTokensType};

// Public exports - Adapters
pub use field_adapter::{
    DataSourceLocatorSet, FieldAdapter, FieldAdapterHandle, PropertyInvalidationType,
};
pub use field3d_asset_adapter::Field3DAssetAdapter;
pub use openvdb_asset_adapter::OpenVDBAssetAdapter;

// Public exports - Data Sources
pub use data_source_field_asset::{
    DataSourceFieldAsset, DataSourceFieldAssetHandle, DataSourceFieldAssetPrim,
    DataSourceFieldAssetPrimHandle,
};
