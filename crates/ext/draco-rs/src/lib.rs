//! Pure Rust port of Google Draco (reference: `_ref/draco`).
//!
//! This crate mirrors the C++ module layout under `draco/src/draco` and will
//! be ported linearly, file by file, to preserve API and algorithm parity.

pub mod animation;

pub use draco_core::attributes;
pub use draco_core::compression;
pub use draco_core::core;
pub use draco_core::mesh;
pub use draco_core::metadata;
pub use draco_core::point_cloud;

pub mod io;
pub mod javascript;
pub mod material;
pub mod maya;
pub mod scene;
pub mod texture;
pub mod tools;
pub mod unity;

#[cfg(test)]
mod animation_tests;
#[cfg(test)]
mod compression_tests;
#[cfg(test)]
mod material_tests;
#[cfg(test)]
mod mesh_tests;
#[cfg(test)]
mod mesh_utils_tests;
#[cfg(test)]
mod parity_tests;
#[cfg(test)]
mod point_cloud_tests;
#[cfg(test)]
mod scene_tests;
#[cfg(test)]
mod texture_tests;
