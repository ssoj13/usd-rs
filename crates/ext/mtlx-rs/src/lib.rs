//! MaterialX — Rust port of the MaterialX library.
//!
//! MaterialX is an open standard for representing rich material and look-development
//! content in computer graphics.

#![deny(rust_2018_idioms, unused_qualifications)]

pub mod core;
pub mod format;
pub mod gen_glsl;
pub mod gen_hw;
pub mod gen_mdl;
pub mod gen_msl;
pub mod gen_osl;
pub mod gen_shader;
pub mod gen_slang;
#[cfg(feature = "wgsl-native")]
pub mod gen_wgsl;

/// Library version string
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// MaterialX library version string (matches reference MaterialX 1.39.x API).
/// Returns "1.39" for document compatibility.
pub fn get_version_string() -> &'static str {
    "1.39"
}

/// Library version as (major, minor) integers. Matches MaterialX 1.39.
pub fn get_version_integers() -> (i32, i32) {
    (1, 39)
}
