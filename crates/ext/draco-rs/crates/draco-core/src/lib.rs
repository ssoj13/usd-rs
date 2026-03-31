//! Core geometry/types for the Draco Rust port.
//!
//! This crate hosts the core, attributes, mesh, point cloud, and metadata
//! implementations shared by IO and bitstream encode/decode crates.
//! Reference: `_ref/draco/src/draco/*`.

pub mod attributes;
pub mod compression;
pub mod core;
pub mod io;
pub mod material;
pub mod mesh;
pub mod metadata;
pub mod point_cloud;
pub mod texture;
