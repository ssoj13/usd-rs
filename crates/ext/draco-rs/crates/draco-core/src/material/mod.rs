//! Material module.
//! Reference: `_ref/draco/src/draco/material/*`.

pub mod material;
pub mod material_library;
pub mod material_utils;

pub use material::{Material, MaterialTransparencyMode};
pub use material_library::MaterialLibrary;
pub use material_utils::MaterialUtils;
