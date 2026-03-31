//! UsdVol - Volumetric data schemas for USD.
//!
//! This module provides schemas for describing volumetric data primitives:
//!
//! - **Volume** - Renderable volume primitive containing field references
//! - **FieldBase** - Abstract base class for all field primitives
//! - **FieldAsset** - Abstract base for asset-backed fields
//! - **OpenVDBAsset** - OpenVDB format field primitive
//! - **Field3DAsset** - Field3D format field primitive
//!
//! # Field Relationships
//!
//! Volume prims reference field prims via namespaced relationships (field:*).
//! The relationship name maps to shader input parameters.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdVol/` module.

mod field_3d_asset;
mod field_asset;
mod field_base;
mod open_vdb_asset;
mod tokens;
mod volume;

// Public re-exports - Typed schemas
pub use field_3d_asset::Field3DAsset;
pub use field_asset::FieldAsset;
pub use field_base::FieldBase;
pub use open_vdb_asset::OpenVDBAsset;
pub use volume::Volume;

// Public re-exports - Tokens
pub use tokens::{USD_VOL_TOKENS, UsdVolTokensType};
