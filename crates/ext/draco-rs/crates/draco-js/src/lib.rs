//! WASM/JavaScript bindings for the Draco Rust port.
//!
//! This crate mirrors the draco3d WebIDL surface (Decoder/Encoder/Builders)
//! while using the pure-Rust Draco core and bitstream implementations.

mod arrays;
mod buffer;
mod decoder;
mod encoder;
mod geometry;
mod metadata;
mod status;
mod types;

pub use arrays::*;
pub use buffer::{draco_free, draco_malloc, DecoderBuffer};
pub use decoder::Decoder;
pub use encoder::{Encoder, ExpertEncoder, MeshBuilder, PointCloudBuilder};
pub use geometry::{
    AttributeOctahedronTransform, AttributeQuantizationTransform, AttributeTransformData,
    GeometryAttribute, Mesh, PointAttribute, PointCloud,
};
pub use metadata::{Metadata, MetadataBuilder, MetadataQuerier};
pub use status::Status;
pub use types::*;
