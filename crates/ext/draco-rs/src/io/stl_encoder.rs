//! STL encoder.
//! Reference: `_ref/draco/src/draco/io/stl_encoder.h` + `.cc`.

use crate::attributes::geometry_attribute::GeometryAttributeType;
use crate::core::encoder_buffer::EncoderBuffer;
use crate::core::status::{Status, StatusCode};
use crate::core::vector_d::{cross_product, Vector3f};
use crate::io::file_writer_factory::FileWriterFactory;
use crate::mesh::Mesh;

pub struct StlEncoder;

impl StlEncoder {
    pub fn new() -> Self {
        Self
    }

    // Encodes the mesh and saves it into a file.
    pub fn encode_to_file(&mut self, mesh: &Mesh, file_name: &str) -> Status {
        let mut file = match FileWriterFactory::open_writer(file_name) {
            Some(file) => file,
            None => {
                return Status::new(StatusCode::IoError, "File couldn't be opened");
            }
        };
        let mut buffer = EncoderBuffer::new();
        let status = self.encode_to_buffer(mesh, &mut buffer);
        if !status.is_ok() {
            return status;
        }
        // Write the binary STL payload to the file.
        let _ = file.write(buffer.data());
        Status::ok()
    }

    // Encodes the mesh into a buffer.
    pub fn encode_to_buffer(&mut self, mesh: &Mesh, out_buffer: &mut EncoderBuffer) -> Status {
        Self::encode_internal(mesh, out_buffer)
    }

    fn encode_internal(mesh: &Mesh, buffer: &mut EncoderBuffer) -> Status {
        let pos_att_id = mesh.get_named_attribute_id(GeometryAttributeType::Position);
        if pos_att_id < 0 {
            return Status::new(
                StatusCode::DracoError,
                "Mesh is missing the position attribute.",
            );
        }
        let pos_att = match mesh.attribute(pos_att_id) {
            Some(att) => att,
            None => {
                return Status::new(
                    StatusCode::DracoError,
                    "Mesh is missing the position attribute.",
                );
            }
        };
        if pos_att.data_type() != crate::core::draco_types::DataType::Float32 {
            return Status::new(
                StatusCode::DracoError,
                "Mesh position attribute is not of type float32.",
            );
        }

        // Extract mesh data before encoding to avoid raw pointer aliasing
        // (reading from mesh buffers while writing to encoder buffer). Plan5 [M-9].
        let num_faces = mesh.num_faces();
        let mut face_data: Vec<([f32; 3], [f32; 3], [f32; 3], [f32; 3])> =
            Vec::with_capacity(num_faces as usize);
        for i in 0..num_faces {
            let face = mesh.face(crate::attributes::geometry_indices::FaceIndex::from(i));
            let p0 = pos_att.get_value_array::<f32, 3>(pos_att.mapped_index(face[0]));
            let p1 = pos_att.get_value_array::<f32, 3>(pos_att.mapped_index(face[1]));
            let p2 = pos_att.get_value_array::<f32, 3>(pos_att.mapped_index(face[2]));
            let v0 = Vector3f::new3(p0[0], p0[1], p0[2]);
            let v1 = Vector3f::new3(p1[0], p1[1], p1[2]);
            let v2 = Vector3f::new3(p2[0], p2[1], p2[2]);
            let mut norm = cross_product(&(v1 - v0), &(v2 - v0));
            norm.normalize();
            face_data.push(([norm[0], norm[1], norm[2]], p0, p1, p2));
        }

        // Encode from extracted data only (no mesh refs during write).
        let header = format!("{:<80}", "generated using Draco");
        let _ = buffer.encode_bytes(header.as_bytes());
        let _ = buffer.encode(num_faces);
        let unused: u16 = 0;
        for (norm, p0, p1, p2) in &face_data {
            let _ = buffer.encode(*norm);
            let _ = buffer.encode(*p0);
            let _ = buffer.encode(*p1);
            let _ = buffer.encode(*p2);
            let _ = buffer.encode(unused);
        }

        Status::ok()
    }
}

impl Default for StlEncoder {
    fn default() -> Self {
        Self::new()
    }
}
