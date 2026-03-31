//! Rust port of Draco IO module.
//! Reference: `_ref/draco/src/draco/io`.

pub mod file_reader_factory;
pub mod file_reader_interface;
pub mod file_utils;
pub mod file_writer_factory;
pub mod file_writer_interface;
pub mod file_writer_utils;
pub mod gltf_decoder;
pub mod gltf_encoder;
pub mod gltf_test_helper;
pub mod gltf_utils;
pub mod mesh_io;
pub mod obj_decoder;
pub mod obj_encoder;
pub mod parser_utils;
pub mod ply_decoder;
pub mod ply_encoder;
pub mod ply_property_reader;
pub mod ply_property_writer;
pub mod ply_reader;
pub mod point_cloud_io;
pub mod scene_io;
pub mod stdio_file_reader;
pub mod stdio_file_writer;
pub mod stl_decoder;
pub mod stl_encoder;
pub mod texture_io;
pub mod tiny_gltf_utils;

pub use draco_core::io::image_compression_options::ImageFormat;

#[cfg(test)]
mod gltf_decoder_tests;
#[cfg(test)]
mod gltf_tests;
#[cfg(test)]
pub(crate) mod test_utils;
#[cfg(test)]
mod tests;
