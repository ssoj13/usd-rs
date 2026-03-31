//! STL decoder.
//! Reference: `_ref/draco/src/draco/io/stl_decoder.h` + `.cc`.

use crate::core::decoder_buffer::DecoderBuffer;
use crate::core::status::{Status, StatusCode};
use crate::core::status_or::StatusOr;
use crate::io::file_utils::read_file_to_buffer;
use crate::mesh::mesh::Mesh;
use crate::mesh::triangle_soup_mesh_builder::TriangleSoupMeshBuilder;

pub struct StlDecoder;

impl StlDecoder {
    pub fn new() -> Self {
        Self
    }

    pub fn decode_from_file(&mut self, file_name: &str) -> StatusOr<Box<Mesh>> {
        let mut data: Vec<u8> = Vec::new();
        if !read_file_to_buffer(file_name, &mut data) {
            return StatusOr::new_status(Status::new(
                StatusCode::IoError,
                "Unable to read input file.",
            ));
        }
        let mut buffer = DecoderBuffer::new();
        buffer.init(&data);
        self.decode_from_buffer(&mut buffer)
    }

    pub fn decode_from_buffer(&mut self, buffer: &mut DecoderBuffer) -> StatusOr<Box<Mesh>> {
        let head = buffer.data_head();
        if head.len() >= 6 && &head[..6] == b"solid " {
            return StatusOr::new_status(Status::new(
                StatusCode::IoError,
                "Currently only binary STL files are supported.",
            ));
        }
        buffer.advance(80);
        let mut face_count: u32 = 0;
        if !buffer.decode(&mut face_count) {
            return StatusOr::new_status(Status::new(
                StatusCode::IoError,
                "Unable to decode STL face count.",
            ));
        }

        let mut builder = TriangleSoupMeshBuilder::new();
        builder.start(face_count as i32);

        let pos_att_id = builder.add_attribute(
            crate::attributes::geometry_attribute::GeometryAttributeType::Position,
            3,
            crate::core::draco_types::DataType::Float32,
        );
        let norm_att_id = builder.add_attribute(
            crate::attributes::geometry_attribute::GeometryAttributeType::Normal,
            3,
            crate::core::draco_types::DataType::Float32,
        );

        for i in 0..face_count {
            let mut data: [f32; 12] = [0.0; 12];
            if !buffer.decode(&mut data) {
                return StatusOr::new_status(Status::new(
                    StatusCode::IoError,
                    "Unable to decode STL face data.",
                ));
            }
            let mut unused: u16 = 0;
            if !buffer.decode(&mut unused) {
                return StatusOr::new_status(Status::new(
                    StatusCode::IoError,
                    "Unable to decode STL attribute bytes.",
                ));
            }

            let n = [data[0], data[1], data[2]];
            let p0 = [data[3], data[4], data[5]];
            let p1 = [data[6], data[7], data[8]];
            let p2 = [data[9], data[10], data[11]];
            let face_id = crate::attributes::geometry_indices::FaceIndex::from(i);

            builder.set_per_face_attribute_value_for_face(norm_att_id, face_id, &n);
            builder.set_attribute_values_for_face(pos_att_id, face_id, &p0, &p1, &p2);
        }

        match builder.finalize() {
            Some(mesh) => StatusOr::new_value(Box::new(mesh)),
            None => StatusOr::new_status(Status::new(
                StatusCode::DracoError,
                "Failed to finalize STL mesh.",
            )),
        }
    }
}

impl Default for StlDecoder {
    fn default() -> Self {
        Self::new()
    }
}
