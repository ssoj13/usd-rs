//! PLY encoder.
//! Reference: `_ref/draco/src/draco/io/ply_encoder.h` + `.cc`.

use crate::attributes::geometry_attribute::GeometryAttributeType;
use crate::attributes::geometry_indices::{FaceIndex, PointIndex};
use crate::attributes::point_attribute::PointAttribute;
use crate::core::draco_types::DataType;
use crate::core::encoder_buffer::EncoderBuffer;
use crate::io::file_writer_factory::FileWriterFactory;
use crate::mesh::Mesh;
use crate::point_cloud::PointCloud;

pub struct PlyEncoder;

impl PlyEncoder {
    pub fn new() -> Self {
        Self
    }

    pub fn encode_to_file_point_cloud(&mut self, pc: &PointCloud, file_name: &str) -> bool {
        let mut file = match FileWriterFactory::open_writer(file_name) {
            Some(file) => file,
            None => return false,
        };
        let mut buffer = EncoderBuffer::new();
        if !self.encode_internal(pc, None, &mut buffer) {
            return false;
        }
        file.write(buffer.data())
    }

    pub fn encode_to_file_mesh(&mut self, mesh: &Mesh, file_name: &str) -> bool {
        let mut file = match FileWriterFactory::open_writer(file_name) {
            Some(file) => file,
            None => return false,
        };
        let mut buffer = EncoderBuffer::new();
        let pc: &PointCloud = mesh;
        if !self.encode_internal(pc, Some(mesh), &mut buffer) {
            return false;
        }
        file.write(buffer.data())
    }

    pub fn encode_to_buffer_point_cloud(
        &mut self,
        pc: &PointCloud,
        out_buffer: &mut EncoderBuffer,
    ) -> bool {
        self.encode_internal(pc, None, out_buffer)
    }

    pub fn encode_to_buffer_mesh(&mut self, mesh: &Mesh, out_buffer: &mut EncoderBuffer) -> bool {
        let pc: &PointCloud = mesh;
        self.encode_internal(pc, Some(mesh), out_buffer)
    }

    fn encode_internal(
        &self,
        pc: &PointCloud,
        mesh: Option<&Mesh>,
        buffer: &mut EncoderBuffer,
    ) -> bool {
        let num_points = pc.num_points();

        // Get attribute IDs
        let pos_att_id = pc.get_named_attribute_id(GeometryAttributeType::Position);
        let normal_att_id = pc.get_named_attribute_id(GeometryAttributeType::Normal);
        let tex_coord_att_id = pc.get_named_attribute_id(GeometryAttributeType::TexCoord);
        let color_att_id = pc.get_named_attribute_id(GeometryAttributeType::Color);

        if pos_att_id < 0 {
            return false;
        }

        // Get position attribute reference (required)
        let pos_att = match pc.attribute(pos_att_id) {
            Some(att) => att,
            None => return false,
        };

        // Get normal attribute reference (optional, must have 3 components)
        let normal_att = if normal_att_id >= 0 {
            let att = match pc.attribute(normal_att_id) {
                Some(att) => att,
                None => return false,
            };
            if att.num_components() != 3 {
                None
            } else {
                Some(att)
            }
        } else {
            None
        };

        // Get texture coordinate attribute reference (optional, must have 2 components)
        let tex_att = if tex_coord_att_id >= 0 {
            let att = match pc.attribute(tex_coord_att_id) {
                Some(att) => att,
                None => return false,
            };
            if att.num_components() != 2 {
                None
            } else {
                Some(att)
            }
        } else {
            None
        };
        let has_texcoords = tex_att.is_some();

        // Get color attribute reference (optional)
        let color_att = if color_att_id >= 0 {
            match pc.attribute(color_att_id) {
                Some(att) => Some(att),
                None => return false,
            }
        } else {
            None
        };

        // Get data types for all attributes
        let pos_type = match Self::attribute_data_type(pos_att) {
            Some(t) => t,
            None => return false,
        };
        let normal_type = if let Some(att) = normal_att {
            match Self::attribute_data_type(att) {
                Some(t) => Some(t),
                None => return false,
            }
        } else {
            None
        };
        let tex_type = if let Some(att) = tex_att {
            match Self::attribute_data_type(att) {
                Some(t) => Some(t),
                None => return false,
            }
        } else {
            None
        };
        let color_type = if let Some(att) = color_att {
            match Self::attribute_data_type(att) {
                Some(t) => Some(t),
                None => return false,
            }
        } else {
            None
        };

        // Build PLY header
        let mut header = String::new();
        header.push_str("ply\n");
        header.push_str("format binary_little_endian 1.0\n");
        header.push_str(&format!("element vertex {}\n", num_points));
        header.push_str(&format!("property {} x\n", pos_type));
        header.push_str(&format!("property {} y\n", pos_type));
        header.push_str(&format!("property {} z\n", pos_type));

        if let Some(normal_type) = normal_type {
            header.push_str(&format!("property {} nx\n", normal_type));
            header.push_str(&format!("property {} ny\n", normal_type));
            header.push_str(&format!("property {} nz\n", normal_type));
        }

        if let Some(color_att) = color_att {
            if let Some(color_type) = color_type {
                if color_att.num_components() > 0 {
                    header.push_str(&format!("property {} red\n", color_type));
                }
                if color_att.num_components() > 1 {
                    header.push_str(&format!("property {} green\n", color_type));
                }
                if color_att.num_components() > 2 {
                    header.push_str(&format!("property {} blue\n", color_type));
                }
                if color_att.num_components() > 3 {
                    header.push_str(&format!("property {} alpha\n", color_type));
                }
            }
        }

        if let Some(mesh) = mesh {
            header.push_str(&format!("element face {}\n", mesh.num_faces()));
            header.push_str("property list uchar int vertex_indices\n");
            if let Some(tex_type) = tex_type {
                header.push_str(&format!("property list uchar {} texcoord\n", tex_type));
            }
        }

        header.push_str("end_header\n");
        buffer.encode_bytes(header.as_bytes());

        // Write vertex data (use get_mapped_value for safe access per geometry_attribute docs)
        let mut pos_bytes = vec![0u8; pos_att.byte_stride() as usize];
        let mut normal_bytes = normal_att
            .as_ref()
            .map(|a| vec![0u8; a.byte_stride() as usize])
            .unwrap_or_default();
        let mut color_bytes = color_att
            .as_ref()
            .map(|a| vec![0u8; a.byte_stride() as usize])
            .unwrap_or_default();
        for v in 0..num_points {
            let point_index = PointIndex::from(v);

            pos_att.get_mapped_value(point_index, &mut pos_bytes);
            buffer.encode_bytes(&pos_bytes);

            if let Some(normal_att) = normal_att {
                normal_att.get_mapped_value(point_index, &mut normal_bytes);
                buffer.encode_bytes(&normal_bytes);
            }

            if let Some(color_att) = color_att {
                color_att.get_mapped_value(point_index, &mut color_bytes);
                buffer.encode_bytes(&color_bytes);
            }
        }

        // Write face data (if mesh is present)
        if let Some(mesh) = mesh {
            let mut tex_bytes = tex_att
                .as_ref()
                .map(|a| vec![0u8; a.byte_stride() as usize])
                .unwrap_or_default();
            for face_id in 0..mesh.num_faces() {
                let face = mesh.face(FaceIndex::from(face_id));
                buffer.encode(3u8);
                for c in 0..3 {
                    if face[c].value() >= num_points {
                        return false;
                    }
                    buffer.encode(face[c].value());
                }

                if has_texcoords {
                    let tex_att = tex_att.as_ref().expect("has_texcoords implies tex_att");
                    buffer.encode(6u8);
                    for c in 0..3 {
                        tex_att.get_mapped_value(face[c], &mut tex_bytes);
                        buffer.encode_bytes(&tex_bytes);
                    }
                }
            }
        }

        true
    }

    fn attribute_data_type(att: &PointAttribute) -> Option<&'static str> {
        match att.data_type() {
            DataType::Float32 => Some("float"),
            DataType::Uint8 => Some("uchar"),
            DataType::Int32 => Some("int"),
            _ => None,
        }
    }
}
