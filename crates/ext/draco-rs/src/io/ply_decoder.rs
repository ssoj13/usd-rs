//! PLY decoder.
//! Reference: `_ref/draco/src/draco/io/ply_decoder.h` + `.cc`.

use crate::attributes::geometry_attribute::{GeometryAttribute, GeometryAttributeType};
use crate::attributes::geometry_indices::{AttributeValueIndex, FaceIndex, PointIndex};
use crate::core::decoder_buffer::DecoderBuffer;
use crate::core::draco_types::{data_type_length, DataType};
use crate::core::status::{Status, StatusCode};
use crate::io::file_utils::read_file_to_buffer;
use crate::io::ply_property_reader::{PlyPropertyReader, PlyReadCast};
use crate::io::ply_reader::{PlyElement, PlyReader};
use crate::mesh::Mesh;
use crate::point_cloud::PointCloud;

/// Decode target type. Mesh is a PointCloud subtype.
enum DecodeTarget<'a> {
    Mesh(&'a mut Mesh),
    PointCloud(&'a mut PointCloud),
}

impl<'a> DecodeTarget<'a> {
    fn as_pc_mut(&mut self) -> &mut PointCloud {
        match self {
            DecodeTarget::Mesh(m) => &mut **m,
            DecodeTarget::PointCloud(pc) => pc,
        }
    }

    fn as_mesh_mut(&mut self) -> Option<&mut Mesh> {
        match self {
            DecodeTarget::Mesh(m) => Some(m),
            _ => None,
        }
    }

    fn is_mesh(&self) -> bool {
        matches!(self, DecodeTarget::Mesh(_))
    }
}

pub struct PlyDecoder;

impl PlyDecoder {
    pub fn new() -> Self {
        Self
    }

    pub fn decode_from_file_mesh(&mut self, file_name: &str, out_mesh: &mut Mesh) -> Status {
        let mut data = Vec::new();
        if !read_file_to_buffer(file_name, &mut data) {
            return Status::new(StatusCode::DracoError, "Unable to read input file.");
        }
        let mut buffer = DecoderBuffer::new();
        buffer.init(&data);
        let mut target = DecodeTarget::Mesh(out_mesh);
        self.decode_internal(&mut buffer, &mut target)
    }

    pub fn decode_from_file_point_cloud(
        &mut self,
        file_name: &str,
        out_point_cloud: &mut PointCloud,
    ) -> Status {
        let mut data = Vec::new();
        if !read_file_to_buffer(file_name, &mut data) {
            return Status::new(StatusCode::DracoError, "Unable to read input file.");
        }
        let mut buffer = DecoderBuffer::new();
        buffer.init(&data);
        let mut target = DecodeTarget::PointCloud(out_point_cloud);
        self.decode_internal(&mut buffer, &mut target)
    }

    pub fn decode_from_buffer_mesh(
        &mut self,
        buffer: &DecoderBuffer,
        out_mesh: &mut Mesh,
    ) -> Status {
        let data = buffer.data_head().to_vec();
        let mut buf = DecoderBuffer::new();
        buf.init(&data);
        let mut target = DecodeTarget::Mesh(out_mesh);
        self.decode_internal(&mut buf, &mut target)
    }

    pub fn decode_from_buffer_point_cloud(
        &mut self,
        buffer: &DecoderBuffer,
        out_point_cloud: &mut PointCloud,
    ) -> Status {
        let data = buffer.data_head().to_vec();
        let mut buf = DecoderBuffer::new();
        buf.init(&data);
        let mut target = DecodeTarget::PointCloud(out_point_cloud);
        self.decode_internal(&mut buf, &mut target)
    }

    fn decode_internal(&mut self, buffer: &mut DecoderBuffer, target: &mut DecodeTarget) -> Status {
        let mut ply_reader = PlyReader::new();
        let status = ply_reader.read(buffer);
        if !status.is_ok() {
            return status;
        }
        if target.is_mesh() {
            let status = self.decode_face_data(ply_reader.get_element_by_name("face"), target);
            if !status.is_ok() {
                return status;
            }
        }
        let status = self.decode_vertex_data(ply_reader.get_element_by_name("vertex"), target);
        if !status.is_ok() {
            return status;
        }
        if target.is_mesh() {
            let num_faces = target
                .as_mesh_mut()
                .map(|mesh| mesh.num_faces())
                .unwrap_or(0);
            if num_faces != 0 {
                if !target.as_pc_mut().deduplicate_attribute_values() {
                    return Status::new(
                        StatusCode::DracoError,
                        "Could not deduplicate attribute values",
                    );
                }
                // Use Mesh::deduplicate_point_ids when decoding to mesh so face indices are remapped (ref: ply_decoder.cc calls out_point_cloud_->DeduplicatePointIds() which for Mesh* uses Mesh::ApplyPointIdDeduplication).
                if let Some(mesh) = target.as_mesh_mut() {
                    mesh.deduplicate_point_ids();
                } else {
                    target.as_pc_mut().deduplicate_point_ids();
                }
            }
            if let Some(mesh) = target.as_mesh_mut() {
                mesh.sync_attribute_data();
            }
        }
        Status::ok()
    }

    fn decode_face_data(
        &mut self,
        face_element: Option<&PlyElement>,
        target: &mut DecodeTarget,
    ) -> Status {
        let face_element = match face_element {
            Some(el) => el,
            None => return Status::ok(),
        };
        let mut vertex_indices = face_element.get_property_by_name("vertex_indices");
        if vertex_indices.is_none() {
            vertex_indices = face_element.get_property_by_name("vertex_index");
        }
        let vertex_indices = match vertex_indices {
            Some(prop) => prop,
            None => return Status::new(StatusCode::DracoError, "No faces defined"),
        };
        if !vertex_indices.is_list() {
            return Status::new(StatusCode::DracoError, "No faces defined");
        }

        let num_triangles = count_num_triangles(face_element, vertex_indices);
        if let Some(mesh) = target.as_mesh_mut() {
            mesh.set_num_faces(num_triangles as usize);
        }

        let num_polygons = face_element.num_entries();
        let reader = PlyPropertyReader::<u32>::new(vertex_indices);
        let mut face = [PointIndex::from(0u32); 3];
        let mut face_index = FaceIndex::from(0u32);
        for i in 0..num_polygons {
            let list_offset = vertex_indices.get_list_entry_offset(i) as i32;
            let list_size = vertex_indices.get_list_entry_num_values(i);
            if list_size < 3 {
                continue;
            }
            let num_triangles = list_size - 2;
            face[0] = PointIndex::from(reader.read_value(list_offset) as u32);
            for ti in 0..num_triangles {
                for c in 1..3 {
                    face[c as usize] =
                        PointIndex::from(reader.read_value(list_offset + (ti + c) as i32) as u32);
                }
                if let Some(mesh) = target.as_mesh_mut() {
                    mesh.set_face(face_index, face);
                }
                face_index += 1u32;
            }
        }
        if let Some(mesh) = target.as_mesh_mut() {
            mesh.set_num_faces(face_index.value() as usize);
        }
        Status::ok()
    }

    fn decode_vertex_data(
        &mut self,
        vertex_element: Option<&PlyElement>,
        target: &mut DecodeTarget,
    ) -> Status {
        let vertex_element = match vertex_element {
            Some(el) => el,
            None => return Status::new(StatusCode::InvalidParameter, "vertex_element is null"),
        };
        let x_prop = vertex_element.get_property_by_name("x");
        let y_prop = vertex_element.get_property_by_name("y");
        let z_prop = vertex_element.get_property_by_name("z");
        if x_prop.is_none() || y_prop.is_none() || z_prop.is_none() {
            return Status::new(
                StatusCode::InvalidParameter,
                "x, y, or z property is missing",
            );
        }
        let x_prop = x_prop.unwrap();
        let y_prop = y_prop.unwrap();
        let z_prop = z_prop.unwrap();

        let num_vertices = vertex_element.num_entries();
        target.as_pc_mut().set_num_points(num_vertices as u32);

        if x_prop.data_type() != y_prop.data_type() || y_prop.data_type() != z_prop.data_type() {
            return Status::new(
                StatusCode::InvalidParameter,
                "x, y, and z properties must have the same type",
            );
        }
        let dt = x_prop.data_type();
        if dt != DataType::Float32 && dt != DataType::Int32 {
            return Status::new(
                StatusCode::InvalidParameter,
                "x, y, and z properties must be of type float32 or int32",
            );
        }
        let mut va = GeometryAttribute::new();
        va.init(
            GeometryAttributeType::Position,
            None,
            3,
            dt,
            false,
            (data_type_length(dt) * 3) as i64,
            0,
        );
        let att_id = target
            .as_pc_mut()
            .add_attribute_from_geometry(&va, true, num_vertices as u32);
        let properties = vec![x_prop, y_prop, z_prop];
        if dt == DataType::Float32 {
            if let Some(att) = target.as_pc_mut().attribute_mut(att_id) {
                read_properties_to_attribute::<f32>(&properties, att, num_vertices);
            }
        } else if dt == DataType::Int32 {
            if let Some(att) = target.as_pc_mut().attribute_mut(att_id) {
                read_properties_to_attribute::<i32>(&properties, att, num_vertices);
            }
        }

        let n_x_prop = vertex_element.get_property_by_name("nx");
        let n_y_prop = vertex_element.get_property_by_name("ny");
        let n_z_prop = vertex_element.get_property_by_name("nz");
        if let (Some(n_x), Some(n_y), Some(n_z)) = (n_x_prop, n_y_prop, n_z_prop) {
            if n_x.data_type() == DataType::Float32
                && n_y.data_type() == DataType::Float32
                && n_z.data_type() == DataType::Float32
            {
                let x_reader = PlyPropertyReader::<f32>::new(n_x);
                let y_reader = PlyPropertyReader::<f32>::new(n_y);
                let z_reader = PlyPropertyReader::<f32>::new(n_z);
                let mut na = GeometryAttribute::new();
                na.init(
                    GeometryAttributeType::Normal,
                    None,
                    3,
                    DataType::Float32,
                    false,
                    (std::mem::size_of::<f32>() * 3) as i64,
                    0,
                );
                let att_id =
                    target
                        .as_pc_mut()
                        .add_attribute_from_geometry(&na, true, num_vertices as u32);
                if let Some(att) = target.as_pc_mut().attribute_mut(att_id) {
                    for i in 0..num_vertices {
                        let mut val = [0.0f32; 3];
                        val[0] = x_reader.read_value(i);
                        val[1] = y_reader.read_value(i);
                        val[2] = z_reader.read_value(i);
                        att.set_attribute_value(AttributeValueIndex::from(i as u32), &val);
                    }
                }
            }
        }

        let r_prop = vertex_element.get_property_by_name("red");
        let g_prop = vertex_element.get_property_by_name("green");
        let b_prop = vertex_element.get_property_by_name("blue");
        let a_prop = vertex_element.get_property_by_name("alpha");
        let mut num_colors = 0;
        if r_prop.is_some() {
            num_colors += 1;
        }
        if g_prop.is_some() {
            num_colors += 1;
        }
        if b_prop.is_some() {
            num_colors += 1;
        }
        if a_prop.is_some() {
            num_colors += 1;
        }

        if num_colors > 0 {
            let mut readers: Vec<PlyPropertyReader<u8>> = Vec::new();
            if let Some(p) = r_prop {
                if p.data_type() != DataType::Uint8 {
                    return Status::new(
                        StatusCode::InvalidParameter,
                        "Type of 'red' property must be uint8",
                    );
                }
                readers.push(PlyPropertyReader::<u8>::new(p));
            }
            if let Some(p) = g_prop {
                if p.data_type() != DataType::Uint8 {
                    return Status::new(
                        StatusCode::InvalidParameter,
                        "Type of 'green' property must be uint8",
                    );
                }
                readers.push(PlyPropertyReader::<u8>::new(p));
            }
            if let Some(p) = b_prop {
                if p.data_type() != DataType::Uint8 {
                    return Status::new(
                        StatusCode::InvalidParameter,
                        "Type of 'blue' property must be uint8",
                    );
                }
                readers.push(PlyPropertyReader::<u8>::new(p));
            }
            if let Some(p) = a_prop {
                if p.data_type() != DataType::Uint8 {
                    return Status::new(
                        StatusCode::InvalidParameter,
                        "Type of 'alpha' property must be uint8",
                    );
                }
                readers.push(PlyPropertyReader::<u8>::new(p));
            }

            let mut ca = GeometryAttribute::new();
            ca.init(
                GeometryAttributeType::Color,
                None,
                num_colors as u8,
                DataType::Uint8,
                true,
                (std::mem::size_of::<u8>() * num_colors) as i64,
                0,
            );
            let att_id =
                target
                    .as_pc_mut()
                    .add_attribute_from_geometry(&ca, true, num_vertices as u32);
            if let Some(att) = target.as_pc_mut().attribute_mut(att_id) {
                for i in 0..num_vertices {
                    let mut val = [0u8; 4];
                    for j in 0..num_colors {
                        val[j] = readers[j].read_value(i);
                    }
                    att.set_attribute_value_bytes(
                        AttributeValueIndex::from(i as u32),
                        &val[..num_colors],
                    );
                }
            }
        }

        Status::ok()
    }
}

fn count_num_triangles(
    face_element: &PlyElement,
    vertex_indices: &crate::io::ply_reader::PlyProperty,
) -> i64 {
    let mut num_triangles = 0i64;
    for i in 0..face_element.num_entries() {
        let list_size = vertex_indices.get_list_entry_num_values(i);
        if list_size < 3 {
            continue;
        }
        num_triangles += list_size - 2;
    }
    num_triangles
}

fn read_properties_to_attribute<T: PlyReadCast + Copy + Default + bytemuck::Pod>(
    properties: &[&crate::io::ply_reader::PlyProperty],
    attribute: &mut crate::attributes::point_attribute::PointAttribute,
    num_vertices: i32,
) -> bool {
    let mut readers: Vec<PlyPropertyReader<T>> = Vec::with_capacity(properties.len());
    for prop in properties {
        readers.push(PlyPropertyReader::<T>::new(*prop));
    }
    let mut memory: Vec<T> = vec![T::default(); properties.len()];
    for i in 0..num_vertices {
        for (prop, reader) in readers.iter().enumerate() {
            memory[prop] = reader.read_value(i);
        }
        let bytes = bytemuck::cast_slice::<T, u8>(memory.as_slice());
        attribute.set_attribute_value_bytes(AttributeValueIndex::from(i as u32), bytes);
    }
    true
}

impl Default for PlyDecoder {
    fn default() -> Self {
        Self::new()
    }
}
