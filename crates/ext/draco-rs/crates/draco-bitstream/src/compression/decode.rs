//! Draco bitstream decoder.
//! Reference: `_ref/draco/src/draco/compression/decode.h|cc`.
//!
//! Provides top-level decode entry points for Draco point clouds and meshes.

use crate::compression::config::compression_shared::{
    DracoHeader, EncodedGeometryType, MeshEncoderMethod, PointCloudEncodingMethod,
};
use crate::compression::config::decoder_options::DecoderOptions;
use crate::compression::mesh::{MeshDecoder, MeshEdgebreakerDecoder, MeshSequentialDecoder};
use crate::compression::point_cloud::{
    decode_header, PointCloudDecoder, PointCloudKdTreeDecoder, PointCloudSequentialDecoder,
};
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::status::{Status, StatusCode};
use draco_core::core::status_or::StatusOr;
use draco_core::mesh::mesh::Mesh;
use draco_core::point_cloud::point_cloud::PointCloud;

pub struct Decoder {
    options: DecoderOptions,
}

pub enum DecodedPointCloud {
    PointCloud(Box<PointCloud>),
    Mesh(Box<Mesh>),
}

impl DecodedPointCloud {
    pub fn as_point_cloud(&self) -> &PointCloud {
        match self {
            Self::PointCloud(pc) => pc.as_ref(),
            Self::Mesh(mesh) => mesh.as_ref(),
        }
    }

    pub fn as_mesh(&self) -> Option<&Mesh> {
        match self {
            Self::Mesh(mesh) => Some(mesh.as_ref()),
            _ => None,
        }
    }
}

enum PointCloudDecoderKind {
    Sequential(PointCloudSequentialDecoder),
    KdTree(PointCloudKdTreeDecoder),
}

impl PointCloudDecoderKind {
    fn decode(
        &mut self,
        options: &DecoderOptions,
        in_buffer: &mut DecoderBuffer,
        out_geometry: &mut PointCloud,
    ) -> Status {
        match self {
            Self::Sequential(dec) => dec.decode(options, in_buffer, out_geometry),
            Self::KdTree(dec) => dec.decode(options, in_buffer, out_geometry),
        }
    }
}

enum MeshDecoderKind {
    Sequential(MeshSequentialDecoder),
    Edgebreaker(MeshEdgebreakerDecoder),
}

impl MeshDecoderKind {
    fn decode(
        &mut self,
        options: &DecoderOptions,
        in_buffer: &mut DecoderBuffer,
        out_geometry: &mut Mesh,
    ) -> Status {
        match self {
            Self::Sequential(dec) => MeshDecoder::decode(dec, options, in_buffer, out_geometry),
            Self::Edgebreaker(dec) => MeshDecoder::decode(dec, options, in_buffer, out_geometry),
        }
    }
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            options: DecoderOptions::new(),
        }
    }

    pub fn options(&mut self) -> &mut DecoderOptions {
        &mut self.options
    }

    pub fn set_skip_attribute_transform(
        &mut self,
        att_type: draco_core::attributes::geometry_attribute::GeometryAttributeType,
    ) {
        self.options
            .set_attribute_bool(&att_type, "skip_attribute_transform", true);
    }

    pub fn get_encoded_geometry_type(
        in_buffer: &mut DecoderBuffer,
    ) -> StatusOr<EncodedGeometryType> {
        let mut temp_buffer = DecoderBuffer::new();
        temp_buffer.init_with_version(in_buffer.data(), in_buffer.bitstream_version());
        temp_buffer.start_decoding_from(in_buffer.position() as i64);
        let mut header = DracoHeader::default();
        let status = decode_header(&mut temp_buffer, &mut header);
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        if header.encoder_type >= EncodedGeometryType::NumEncodedGeometryTypes as u8 {
            return StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Unsupported geometry type.",
            ));
        }
        let encoded_type = match header.encoder_type {
            x if x == EncodedGeometryType::PointCloud as u8 => EncodedGeometryType::PointCloud,
            x if x == EncodedGeometryType::TriangularMesh as u8 => {
                EncodedGeometryType::TriangularMesh
            }
            _ => EncodedGeometryType::InvalidGeometryType,
        };
        StatusOr::new_value(encoded_type)
    }

    pub fn decode_point_cloud_from_buffer(
        &mut self,
        in_buffer: &mut DecoderBuffer,
    ) -> StatusOr<DecodedPointCloud> {
        let encoded_type_or = Self::get_encoded_geometry_type(in_buffer);
        if !encoded_type_or.is_ok() {
            return StatusOr::new_status(encoded_type_or.status().clone());
        }
        let encoded_type = encoded_type_or.into_value();
        if encoded_type == EncodedGeometryType::PointCloud {
            let mut point_cloud = Box::new(PointCloud::new());
            let status = self.decode_buffer_to_geometry(in_buffer, point_cloud.as_mut());
            if !status.is_ok() {
                return StatusOr::new_status(status);
            }
            return StatusOr::new_value(DecodedPointCloud::PointCloud(point_cloud));
        }
        if encoded_type == EncodedGeometryType::TriangularMesh {
            let mut mesh = Box::new(Mesh::new());
            let status = self.decode_buffer_to_geometry_mesh(in_buffer, mesh.as_mut());
            if !status.is_ok() {
                return StatusOr::new_status(status);
            }
            return StatusOr::new_value(DecodedPointCloud::Mesh(mesh));
        }
        StatusOr::new_status(Status::new(
            StatusCode::DracoError,
            "Unsupported geometry type.",
        ))
    }

    pub fn decode_mesh_from_buffer(
        &mut self,
        in_buffer: &mut DecoderBuffer,
    ) -> StatusOr<Box<Mesh>> {
        let mut mesh = Box::new(Mesh::new());
        let status = self.decode_buffer_to_geometry_mesh(in_buffer, mesh.as_mut());
        if !status.is_ok() {
            return StatusOr::new_status(status);
        }
        StatusOr::new_value(mesh)
    }

    pub fn decode_buffer_to_geometry(
        &mut self,
        in_buffer: &mut DecoderBuffer,
        out_geometry: &mut PointCloud,
    ) -> Status {
        let mut temp_buffer = DecoderBuffer::new();
        temp_buffer.init_with_version(in_buffer.data(), in_buffer.bitstream_version());
        temp_buffer.start_decoding_from(in_buffer.position() as i64);
        let mut header = DracoHeader::default();
        let status = decode_header(&mut temp_buffer, &mut header);
        if !status.is_ok() {
            return status;
        }
        if header.encoder_type != EncodedGeometryType::PointCloud as u8 {
            return Status::new(StatusCode::DracoError, "Input is not a point cloud.");
        }
        let mut decoder = match create_point_cloud_decoder(header.encoder_method as i8) {
            Ok(decoder) => decoder,
            Err(status) => return status,
        };
        decoder.decode(&self.options, in_buffer, out_geometry)
    }

    pub fn decode_buffer_to_geometry_mesh(
        &mut self,
        in_buffer: &mut DecoderBuffer,
        out_geometry: &mut Mesh,
    ) -> Status {
        let mut temp_buffer = DecoderBuffer::new();
        temp_buffer.init_with_version(in_buffer.data(), in_buffer.bitstream_version());
        temp_buffer.start_decoding_from(in_buffer.position() as i64);
        let mut header = DracoHeader::default();
        let status = decode_header(&mut temp_buffer, &mut header);
        if !status.is_ok() {
            return status;
        }
        if header.encoder_type != EncodedGeometryType::TriangularMesh as u8 {
            return Status::new(StatusCode::DracoError, "Input is not a mesh.");
        }
        let mut decoder = match create_mesh_decoder(header.encoder_method as u8) {
            Ok(decoder) => decoder,
            Err(status) => return status,
        };
        decoder.decode(&self.options, in_buffer, out_geometry)
    }
}

fn create_point_cloud_decoder(method: i8) -> Result<PointCloudDecoderKind, Status> {
    if method == PointCloudEncodingMethod::PointCloudSequentialEncoding as i8 {
        Ok(PointCloudDecoderKind::Sequential(
            PointCloudSequentialDecoder::new(),
        ))
    } else if method == PointCloudEncodingMethod::PointCloudKdTreeEncoding as i8 {
        Ok(PointCloudDecoderKind::KdTree(PointCloudKdTreeDecoder::new()))
    } else {
        Err(Status::new(
            StatusCode::DracoError,
            "Unsupported encoding method.",
        ))
    }
}

fn create_mesh_decoder(method: u8) -> Result<MeshDecoderKind, Status> {
    if method == MeshEncoderMethod::MeshSequentialEncoding as u8 {
        Ok(MeshDecoderKind::Sequential(MeshSequentialDecoder::new()))
    } else if method == MeshEncoderMethod::MeshEdgebreakerEncoding as u8 {
        Ok(MeshDecoderKind::Edgebreaker(MeshEdgebreakerDecoder::new()))
    } else {
        Err(Status::new(
            StatusCode::DracoError,
            "Unsupported encoding method.",
        ))
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}
