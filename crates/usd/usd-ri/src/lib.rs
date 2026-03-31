//! UsdRi - RenderMan integration schemas for USD.
//!
//! This module provides schemas for RenderMan-specific features:
//!
//! - **RiMaterialAPI** - Connect materials to RenderMan shaders (deprecated)
//! - **RiSplineAPI** - Define named splines (deprecated)
//! - **StatementsAPI** - Container for RenderMan statements
//!
//! # Deprecation Notice
//!
//! Several schemas in this module are deprecated:
//! - `RiMaterialAPI` - Use `UsdShadeMaterial` instead
//! - `RiSplineAPI` - Will be removed in a future release
//! - Coordinate system APIs in `StatementsAPI` - Use `UsdShadeCoordSysAPI`
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRi/` module.

mod material_api;
mod rman_utilities;
mod spline_api;
mod statements_api;
mod tokens;
mod type_utils;

// Public re-exports - API schemas
pub use material_api::RiMaterialAPI;
pub use spline_api::RiSplineAPI;
pub use statements_api::StatementsAPI;

// Public re-exports - Utilities
pub use rman_utilities::{
    RManFaceVaryingLinearInterpolation, RManInterpolateBoundary, RManTriangleSubdivisionRule,
    convert_from_rman_face_varying_linear_interpolation, convert_from_rman_interpolate_boundary,
    convert_from_rman_triangle_subdivision_rule, convert_rman_set_specification_to_list_op,
    convert_to_rman_face_varying_linear_interpolation, convert_to_rman_interpolate_boundary,
    convert_to_rman_triangle_subdivision_rule, does_attribute_use_set_specification,
};

// Public re-exports - Tokens
pub use tokens::{USD_RI_TOKENS, UsdRiTokensType};

// Public re-exports - Type utilities
pub use type_utils::{get_ri_type, get_usd_type};
