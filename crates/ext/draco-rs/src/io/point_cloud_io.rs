//! Point cloud IO helpers.
//! Reference: `_ref/draco/src/draco/io/point_cloud_io.h` + `.cc`.

use crate::core::decoder_buffer::DecoderBuffer;
use crate::core::encoder_buffer::EncoderBuffer;
use crate::core::status::{ok_status, Status, StatusCode};
use crate::core::status_or::StatusOr;
use crate::io::file_utils::{lowercase_file_extension, read_file_to_buffer};
use crate::io::obj_decoder::ObjDecoder;
use crate::io::ply_decoder::PlyDecoder;
use crate::point_cloud::PointCloud;
use draco_bitstream::compression::config::compression_shared::PointCloudEncodingMethod;
use draco_bitstream::compression::config::encoder_options::EncoderOptions;
use draco_bitstream::compression::decode::{DecodedPointCloud, Decoder};
use draco_bitstream::compression::expert_encode::ExpertEncoder;
use std::io::{Read, Write};

fn decode_point_cloud_from_bytes(data: &[u8]) -> StatusOr<Box<PointCloud>> {
    let mut decoder_buffer = DecoderBuffer::new();
    decoder_buffer.init(data);
    let mut decoder = Decoder::new();
    let decoded = decoder.decode_point_cloud_from_buffer(&mut decoder_buffer);
    if !decoded.is_ok() {
        return StatusOr::new_status(decoded.status().clone());
    }
    let decoded = decoded.into_value();
    match decoded {
        DecodedPointCloud::PointCloud(point_cloud) => StatusOr::new_value(point_cloud),
        DecodedPointCloud::Mesh(mesh) => {
            let mut point_cloud = Box::new(PointCloud::new());
            point_cloud.copy(mesh.as_ref());
            StatusOr::new_value(point_cloud)
        }
    }
}

pub fn read_point_cloud_from_file(file_name: &str) -> StatusOr<Box<PointCloud>> {
    let mut pc = Box::new(PointCloud::new());
    let extension = lowercase_file_extension(file_name);

    if extension == "obj" {
        let mut obj_decoder = ObjDecoder::new();
        let status = obj_decoder.decode_from_file_point_cloud(file_name, &mut pc);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        return StatusOr::new_value(pc);
    }
    if extension == "ply" {
        let mut ply_decoder = PlyDecoder::new();
        let status = ply_decoder.decode_from_file_point_cloud(file_name, &mut pc);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        return StatusOr::new_value(pc);
    }

    let mut buffer: Vec<u8> = Vec::new();
    if !read_file_to_buffer(file_name, &mut buffer) {
        return StatusOr::new_status(Status::new(
            StatusCode::IoError,
            "Unable to read input file.",
        ));
    }
    decode_point_cloud_from_bytes(&buffer)
}

/// Writes a point cloud into a byte stream using explicit encoding options.
pub fn write_point_cloud_into_writer_with_options<W: Write>(
    pc: &PointCloud,
    writer: &mut W,
    method: PointCloudEncodingMethod,
    options: &EncoderOptions,
) -> Status {
    let mut buffer = EncoderBuffer::new();
    let local_options = options.clone();
    let mut encoder = ExpertEncoder::new_point_cloud(pc);
    encoder.reset(local_options);
    encoder.set_encoding_method(method as i32);
    let status = encoder.encode_to_buffer(&mut buffer);
    if !status.is_ok() {
        return status;
    }
    if let Err(err) = writer.write_all(buffer.data()) {
        return Status::new(
            StatusCode::IoError,
            &format!("Stream write failed: {}", err),
        );
    }
    ok_status()
}

/// Writes a point cloud into a byte stream using default encoder options.
pub fn write_point_cloud_into_writer_with_method<W: Write>(
    pc: &PointCloud,
    writer: &mut W,
    method: PointCloudEncodingMethod,
) -> Status {
    let options = EncoderOptions::create_default_options();
    write_point_cloud_into_writer_with_options(pc, writer, method, &options)
}

/// Writes a point cloud into a byte stream using the default encoding method.
pub fn write_point_cloud_into_writer<W: Write>(pc: &PointCloud, writer: &mut W) -> Status {
    write_point_cloud_into_writer_with_method(
        pc,
        writer,
        PointCloudEncodingMethod::PointCloudSequentialEncoding,
    )
}

/// Reads a point cloud from a byte stream encoded in Draco bitstream format.
pub fn read_point_cloud_from_reader<R: Read>(reader: &mut R) -> StatusOr<Box<PointCloud>> {
    let mut data: Vec<u8> = Vec::new();
    if let Err(err) = reader.read_to_end(&mut data) {
        return StatusOr::new_status(Status::new(
            StatusCode::IoError,
            &format!("Stream read failed: {}", err),
        ));
    }
    decode_point_cloud_from_bytes(&data)
}
