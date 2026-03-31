//! OBJ decoder.
//! Reference: `_ref/draco/src/draco/io/obj_decoder.h` + `.cc`.

use std::collections::HashMap;

use crate::attributes::geometry_attribute::{GeometryAttribute, GeometryAttributeType};
use crate::attributes::geometry_indices::{AttributeValueIndex, FaceIndex, PointIndex};
use crate::core::decoder_buffer::DecoderBuffer;
use crate::core::draco_types::DataType;
use crate::core::status::{Status, StatusCode};
use crate::io::file_utils::{get_full_path, read_file_to_buffer};
use crate::io::parser_utils;
use crate::mesh::Mesh;
use crate::metadata::geometry_metadata::AttributeMetadata;
use crate::point_cloud::PointCloud;

const MAX_CORNERS: usize = 8;

// Internal enum to represent decode target (Mesh or PointCloud).
enum DecodeTarget<'a> {
    Mesh { mesh: &'a mut Mesh, active: bool },
    PointCloud(&'a mut PointCloud),
}

impl<'a> DecodeTarget<'a> {
    // Get mutable ref to PointCloud (all meshes are also point clouds).
    fn as_pc_mut(&mut self) -> &mut PointCloud {
        match self {
            DecodeTarget::Mesh { mesh, .. } => &mut **mesh,
            DecodeTarget::PointCloud(pc) => pc,
        }
    }

    // Get mutable ref to Mesh only if active.
    fn as_mesh_mut(&mut self) -> Option<&mut Mesh> {
        match self {
            DecodeTarget::Mesh { mesh, active } if *active => Some(mesh),
            _ => None,
        }
    }

    // Deactivate mesh mode (e.g., when num_obj_faces == 0).
    fn deactivate_mesh(&mut self) {
        if let DecodeTarget::Mesh { active, .. } = self {
            *active = false;
        }
    }
}

pub struct ObjDecoder {
    counting_mode: bool,
    num_obj_faces: i32,
    num_positions: i32,
    num_tex_coords: i32,
    num_normals: i32,
    num_materials: i32,
    last_sub_obj_id: i32,
    pos_att_id: i32,
    tex_att_id: i32,
    norm_att_id: i32,
    material_att_id: i32,
    sub_obj_att_id: i32,
    added_edge_att_id: i32,
    deduplicate_input_values: bool,
    last_material_id: i32,
    material_file_name: String,
    input_file_name: String,
    material_name_to_id: HashMap<String, i32>,
    obj_name_to_id: HashMap<String, i32>,
    use_metadata: bool,
    preserve_polygons: bool,
    has_polygons: bool,
    // Field to collect material files during counting pass.
    collected_material_files: Vec<String>,
}

impl ObjDecoder {
    pub fn new() -> Self {
        Self {
            counting_mode: true,
            num_obj_faces: 0,
            num_positions: 0,
            num_tex_coords: 0,
            num_normals: 0,
            num_materials: 0,
            last_sub_obj_id: 0,
            pos_att_id: -1,
            tex_att_id: -1,
            norm_att_id: -1,
            material_att_id: -1,
            sub_obj_att_id: -1,
            added_edge_att_id: -1,
            deduplicate_input_values: true,
            last_material_id: 0,
            material_file_name: String::new(),
            input_file_name: String::new(),
            material_name_to_id: HashMap::new(),
            obj_name_to_id: HashMap::new(),
            use_metadata: false,
            preserve_polygons: false,
            has_polygons: false,
            collected_material_files: Vec::new(),
        }
    }

    pub fn set_deduplicate_input_values(&mut self, v: bool) {
        self.deduplicate_input_values = v;
    }

    pub fn set_use_metadata(&mut self, flag: bool) {
        self.use_metadata = flag;
    }

    pub fn set_preserve_polygons(&mut self, flag: bool) {
        self.preserve_polygons = flag;
    }

    pub fn decode_from_file_mesh(&mut self, file_name: &str, out_mesh: &mut Mesh) -> Status {
        self.decode_from_file_mesh_with_files(file_name, out_mesh, None)
    }

    pub fn decode_from_file_mesh_with_files(
        &mut self,
        file_name: &str,
        out_mesh: &mut Mesh,
        mesh_files: Option<&mut Vec<String>>,
    ) -> Status {
        let mut buffer_data: Vec<u8> = Vec::new();
        if !read_file_to_buffer(file_name, &mut buffer_data) {
            return Status::new(StatusCode::DracoError, "Unable to read input file.");
        }
        let mut buffer = DecoderBuffer::new();
        buffer.init(&buffer_data);
        self.input_file_name = file_name.to_string();
        let mut target = DecodeTarget::Mesh {
            mesh: out_mesh,
            active: true,
        };
        self.decode_internal(&mut buffer, &mut target, mesh_files)
    }

    pub fn decode_from_file_point_cloud(
        &mut self,
        file_name: &str,
        out_point_cloud: &mut PointCloud,
    ) -> Status {
        let mut buffer_data: Vec<u8> = Vec::new();
        if !read_file_to_buffer(file_name, &mut buffer_data) {
            return Status::new(StatusCode::DracoError, "Unable to read input file.");
        }
        let mut buffer = DecoderBuffer::new();
        buffer.init(&buffer_data);
        self.input_file_name = file_name.to_string();
        let mut target = DecodeTarget::PointCloud(out_point_cloud);
        self.decode_internal(&mut buffer, &mut target, None)
    }

    pub fn decode_from_buffer_mesh(
        &mut self,
        buffer: &DecoderBuffer,
        out_mesh: &mut Mesh,
    ) -> Status {
        let buffer_data = buffer.data_head().to_vec();
        let mut local_buffer = DecoderBuffer::new();
        local_buffer.init(&buffer_data);
        let mut target = DecodeTarget::Mesh {
            mesh: out_mesh,
            active: true,
        };
        self.decode_internal(&mut local_buffer, &mut target, None)
    }

    pub fn decode_from_buffer_point_cloud(
        &mut self,
        buffer: &DecoderBuffer,
        out_point_cloud: &mut PointCloud,
    ) -> Status {
        let buffer_data = buffer.data_head().to_vec();
        let mut local_buffer = DecoderBuffer::new();
        local_buffer.init(&buffer_data);
        let mut target = DecodeTarget::PointCloud(out_point_cloud);
        self.decode_internal(&mut local_buffer, &mut target, None)
    }

    fn decode_internal(
        &mut self,
        buffer: &mut DecoderBuffer,
        target: &mut DecodeTarget,
        mesh_files: Option<&mut Vec<String>>,
    ) -> Status {
        self.counting_mode = true;
        self.reset_counters();
        self.material_name_to_id.clear();
        self.obj_name_to_id.clear();
        self.last_sub_obj_id = 0;
        self.collected_material_files.clear();

        // Counting pass.
        let mut status = Status::ok();
        while self.parse_definition(buffer, target, &mut status) && status.is_ok() {}
        if !status.is_ok() {
            return status;
        }

        // Push files to mesh_files if provided.
        if let Some(files) = mesh_files {
            if !self.input_file_name.is_empty() {
                files.push(self.input_file_name.clone());
            }
            files.extend(self.collected_material_files.drain(..));
        }

        let mut use_identity_mapping = false;
        if self.num_obj_faces == 0 {
            if self.num_positions == 0 {
                return Status::new(StatusCode::DracoError, "No position attribute");
            }
            if self.num_tex_coords > 0 && self.num_tex_coords != self.num_positions {
                return Status::new(
                    StatusCode::DracoError,
                    "Invalid number of texture coordinates for a point cloud",
                );
            }
            if self.num_normals > 0 && self.num_normals != self.num_positions {
                return Status::new(
                    StatusCode::DracoError,
                    "Invalid number of normals for a point cloud",
                );
            }
            target.deactivate_mesh();
            use_identity_mapping = true;
        }

        let num_obj_faces = self.num_obj_faces;
        let num_positions = self.num_positions;
        let num_tex_coords = self.num_tex_coords;
        let num_normals = self.num_normals;

        if let Some(mesh) = target.as_mesh_mut() {
            mesh.set_num_faces(num_obj_faces as usize);
        }

        if num_obj_faces > 0 {
            target
                .as_pc_mut()
                .set_num_points((3 * num_obj_faces) as u32);
        } else {
            target.as_pc_mut().set_num_points(num_positions as u32);
        }

        if num_positions > 0 {
            let mut att = GeometryAttribute::new();
            att.init(
                GeometryAttributeType::Position,
                None,
                3,
                DataType::Float32,
                false,
                (std::mem::size_of::<f32>() * 3) as i64,
                0,
            );
            let id = target.as_pc_mut().add_attribute_from_geometry(
                &att,
                use_identity_mapping,
                num_positions as u32,
            );
            self.pos_att_id = id;
        }
        if num_tex_coords > 0 {
            let mut att = GeometryAttribute::new();
            att.init(
                GeometryAttributeType::TexCoord,
                None,
                2,
                DataType::Float32,
                false,
                (std::mem::size_of::<f32>() * 2) as i64,
                0,
            );
            let id = target.as_pc_mut().add_attribute_from_geometry(
                &att,
                use_identity_mapping,
                num_tex_coords as u32,
            );
            self.tex_att_id = id;
        }
        if num_normals > 0 {
            let mut att = GeometryAttribute::new();
            att.init(
                GeometryAttributeType::Normal,
                None,
                3,
                DataType::Float32,
                false,
                (std::mem::size_of::<f32>() * 3) as i64,
                0,
            );
            let id = target.as_pc_mut().add_attribute_from_geometry(
                &att,
                use_identity_mapping,
                num_normals as u32,
            );
            self.norm_att_id = id;
        }

        if self.preserve_polygons && self.has_polygons {
            let mut att = GeometryAttribute::new();
            att.init(
                GeometryAttributeType::Generic,
                None,
                1,
                DataType::Uint8,
                false,
                1,
                0,
            );
            let added_edge_att_id = target
                .as_pc_mut()
                .add_attribute_from_geometry(&att, false, 2);
            self.added_edge_att_id = added_edge_att_id;
            {
                let pc = target.as_pc_mut();
                for i in 0u8..=1u8 {
                    let avi = AttributeValueIndex::from(i as u32);
                    if let Some(att) = pc.attribute_mut(added_edge_att_id) {
                        att.set_attribute_value(avi, &i);
                    }
                }
                let mut metadata = AttributeMetadata::new();
                metadata.add_entry_string("name", "added_edges");
                pc.add_attribute_metadata(added_edge_att_id, metadata);
            }
        }

        if self.num_materials > 0 && self.num_obj_faces > 0 {
            let mut att = GeometryAttribute::new();
            // NOTE: C++ uses GENERIC here but we use Material type for transcoder compatibility
            let attr_type = GeometryAttributeType::Material;
            if self.num_materials < 256 {
                att.init(attr_type, None, 1, DataType::Uint8, false, 1, 0);
            } else if self.num_materials < (1 << 16) {
                att.init(attr_type, None, 1, DataType::Uint16, false, 2, 0);
            } else {
                att.init(attr_type, None, 1, DataType::Uint32, false, 4, 0);
            }
            let num_materials = self.num_materials;
            let material_att_id =
                target
                    .as_pc_mut()
                    .add_attribute_from_geometry(&att, false, num_materials as u32);
            self.material_att_id = material_att_id;
            {
                let pc = target.as_pc_mut();
                for i in 0..num_materials {
                    let avi = AttributeValueIndex::from(i as u32);
                    if let Some(att) = pc.attribute_mut(material_att_id) {
                        match att.data_type() {
                            DataType::Uint8 => {
                                let v = i as u8;
                                att.set_attribute_value(avi, &v);
                            }
                            DataType::Uint16 => {
                                let v = i as u16;
                                att.set_attribute_value(avi, &v);
                            }
                            _ => {
                                let v = i as u32;
                                att.set_attribute_value(avi, &v);
                            }
                        }
                    }
                }
            }

            let material_name_to_id: Vec<(String, i32)> = self
                .material_name_to_id
                .iter()
                .map(|(name, id)| (name.clone(), *id))
                .collect();
            if let Some(mesh) = target.as_mesh_mut() {
                // Keep mesh material library in sync with the material attribute.
                for i in 0..num_materials {
                    let _ = mesh.material_library_mut().mutable_material(i);
                }
                for (name, id) in &material_name_to_id {
                    if let Some(mat) = mesh.material_library_mut().mutable_material(*id) {
                        mat.set_name(name);
                    }
                }
            }

            if self.use_metadata {
                let mut material_metadata = AttributeMetadata::new();
                material_metadata.add_entry_string("name", "material");
                for (name, id) in &self.material_name_to_id {
                    material_metadata.add_entry_int(name, *id);
                }
                if !self.material_file_name.is_empty() {
                    material_metadata.add_entry_string("file_name", &self.material_file_name);
                }
                let pc = target.as_pc_mut();
                pc.add_attribute_metadata(material_att_id, material_metadata);
            }
        }

        if !self.obj_name_to_id.is_empty() && self.num_obj_faces > 0 {
            let mut att = GeometryAttribute::new();
            let count = self.obj_name_to_id.len() as i32;
            if count < 256 {
                att.init(
                    GeometryAttributeType::Generic,
                    None,
                    1,
                    DataType::Uint8,
                    false,
                    1,
                    0,
                );
            } else if count < (1 << 16) {
                att.init(
                    GeometryAttributeType::Generic,
                    None,
                    1,
                    DataType::Uint16,
                    false,
                    2,
                    0,
                );
            } else {
                att.init(
                    GeometryAttributeType::Generic,
                    None,
                    1,
                    DataType::Uint32,
                    false,
                    4,
                    0,
                );
            }
            let ids: Vec<i32> = self.obj_name_to_id.values().copied().collect();
            let name_pairs: Vec<(String, i32)> = self
                .obj_name_to_id
                .iter()
                .map(|(name, id)| (name.clone(), *id))
                .collect();
            let sub_obj_att_id =
                target
                    .as_pc_mut()
                    .add_attribute_from_geometry(&att, false, count as u32);
            self.sub_obj_att_id = sub_obj_att_id;
            {
                let pc = target.as_pc_mut();
                for id in &ids {
                    let avi = AttributeValueIndex::from(*id as u32);
                    if let Some(att) = pc.attribute_mut(sub_obj_att_id) {
                        match att.data_type() {
                            DataType::Uint8 => {
                                let v = *id as u8;
                                att.set_attribute_value(avi, &v);
                            }
                            DataType::Uint16 => {
                                let v = *id as u16;
                                att.set_attribute_value(avi, &v);
                            }
                            _ => {
                                let v = *id as u32;
                                att.set_attribute_value(avi, &v);
                            }
                        }
                    }
                }
            }
            if self.use_metadata {
                let mut sub_obj_metadata = AttributeMetadata::new();
                sub_obj_metadata.add_entry_string("name", "sub_obj");
                for (name, id) in &name_pairs {
                    sub_obj_metadata.add_entry_int(name, *id);
                }
                let pc = target.as_pc_mut();
                pc.add_attribute_metadata(sub_obj_att_id, sub_obj_metadata);
            }
        }

        self.counting_mode = false;
        self.reset_counters();
        buffer.start_decoding_from(0);
        while self.parse_definition(buffer, target, &mut status) && status.is_ok() {}
        if !status.is_ok() {
            return status;
        }

        let num_obj_faces = self.num_obj_faces;
        if let Some(mesh) = target.as_mesh_mut() {
            let mut face = [PointIndex::from(0u32); 3];
            for i in 0..num_obj_faces {
                for c in 0..3 {
                    let vert_id = (3 * i + c as i32) as u32;
                    face[c] = PointIndex::from(vert_id);
                }
                mesh.set_face(FaceIndex::from(i as u32), face);
            }
        }

        if self.deduplicate_input_values {
            let _ = target.as_pc_mut().deduplicate_attribute_values();
        }
        // Mesh dedup must update faces; point cloud dedup is only for attributes.
        if let Some(mesh) = target.as_mesh_mut() {
            mesh.deduplicate_point_ids();
            mesh.sync_attribute_data();
        } else {
            target.as_pc_mut().deduplicate_point_ids();
        }
        status
    }

    fn reset_counters(&mut self) {
        self.num_obj_faces = 0;
        self.num_positions = 0;
        self.num_tex_coords = 0;
        self.num_normals = 0;
        self.last_material_id = 0;
        self.last_sub_obj_id = 0;
    }

    fn parse_definition(
        &mut self,
        buffer: &mut DecoderBuffer,
        target: &mut DecodeTarget,
        status: &mut Status,
    ) -> bool {
        let mut c: u8 = 0;
        parser_utils::skip_whitespace(buffer);
        if !buffer.peek(&mut c) {
            return false;
        }
        if c == b'#' {
            parser_utils::skip_line(buffer);
            return true;
        }
        if self.parse_vertex_position(buffer, target, status) {
            return true;
        }
        if self.parse_normal(buffer, target, status) {
            return true;
        }
        if self.parse_tex_coord(buffer, target, status) {
            return true;
        }
        if self.parse_face(buffer, target, status) {
            return true;
        }
        if self.parse_material(buffer, status) {
            return true;
        }
        if self.parse_material_lib(buffer, status) {
            return true;
        }
        if self.parse_object(buffer, status) {
            return true;
        }
        parser_utils::skip_line(buffer);
        true
    }

    fn parse_vertex_position(
        &mut self,
        buffer: &mut DecoderBuffer,
        target: &mut DecodeTarget,
        status: &mut Status,
    ) -> bool {
        let mut c: [u8; 2] = [0, 0];
        if !buffer.peek(&mut c) {
            return false;
        }
        if c[0] != b'v' || c[1] != b' ' {
            return false;
        }
        buffer.advance(2);
        if !self.counting_mode {
            let mut val = [0.0f32; 3];
            for i in 0..3 {
                parser_utils::skip_whitespace(buffer);
                if !parser_utils::parse_float(buffer, &mut val[i]) {
                    *status = Status::new(StatusCode::DracoError, "Failed to parse a float number");
                    return true;
                }
            }
            let pos_att_id = self.pos_att_id;
            let num_positions = self.num_positions;
            if let Some(att) = target.as_pc_mut().attribute_mut(pos_att_id) {
                let avi = AttributeValueIndex::from(num_positions as u32);
                att.set_attribute_value(avi, &val);
            }
        }
        self.num_positions += 1;
        parser_utils::skip_line(buffer);
        true
    }

    fn parse_normal(
        &mut self,
        buffer: &mut DecoderBuffer,
        target: &mut DecodeTarget,
        status: &mut Status,
    ) -> bool {
        let mut c: [u8; 2] = [0, 0];
        if !buffer.peek(&mut c) {
            return false;
        }
        if c[0] != b'v' || c[1] != b'n' {
            return false;
        }
        buffer.advance(2);
        if !self.counting_mode {
            let mut val = [0.0f32; 3];
            for i in 0..3 {
                parser_utils::skip_whitespace(buffer);
                if !parser_utils::parse_float(buffer, &mut val[i]) {
                    *status = Status::new(StatusCode::DracoError, "Failed to parse a float number");
                    return true;
                }
            }
            let norm_att_id = self.norm_att_id;
            let num_normals = self.num_normals;
            if let Some(att) = target.as_pc_mut().attribute_mut(norm_att_id) {
                let avi = AttributeValueIndex::from(num_normals as u32);
                att.set_attribute_value(avi, &val);
            }
        }
        self.num_normals += 1;
        parser_utils::skip_line(buffer);
        true
    }

    fn parse_tex_coord(
        &mut self,
        buffer: &mut DecoderBuffer,
        target: &mut DecodeTarget,
        status: &mut Status,
    ) -> bool {
        let mut c: [u8; 2] = [0, 0];
        if !buffer.peek(&mut c) {
            return false;
        }
        if c[0] != b'v' || c[1] != b't' {
            return false;
        }
        buffer.advance(2);
        if !self.counting_mode {
            let mut val = [0.0f32; 2];
            for i in 0..2 {
                parser_utils::skip_whitespace(buffer);
                if !parser_utils::parse_float(buffer, &mut val[i]) {
                    *status = Status::new(StatusCode::DracoError, "Failed to parse a float number");
                    return true;
                }
            }
            let tex_att_id = self.tex_att_id;
            let num_tex_coords = self.num_tex_coords;
            if let Some(att) = target.as_pc_mut().attribute_mut(tex_att_id) {
                let avi = AttributeValueIndex::from(num_tex_coords as u32);
                att.set_attribute_value(avi, &val);
            }
        }
        self.num_tex_coords += 1;
        parser_utils::skip_line(buffer);
        true
    }

    fn parse_face(
        &mut self,
        buffer: &mut DecoderBuffer,
        target: &mut DecodeTarget,
        status: &mut Status,
    ) -> bool {
        let mut c: u8 = 0;
        if !buffer.peek(&mut c) {
            return false;
        }
        if c != b'f' {
            return false;
        }
        buffer.advance(1);
        if !self.counting_mode {
            let mut indices: [[i32; 3]; MAX_CORNERS] = [[0, 0, 0]; MAX_CORNERS];
            let mut num_valid = 0;
            for i in 0..MAX_CORNERS {
                if !self.parse_vertex_indices(buffer, &mut indices[i]) {
                    if i >= 3 {
                        break;
                    }
                    *status = Status::new(StatusCode::DracoError, "Failed to parse vertex indices");
                    return true;
                }
                num_valid += 1;
            }
            let nt = num_valid as i32 - 2;
            let added_edge_att_id = self.added_edge_att_id;
            for t in 0..nt {
                for corner in 0..3 {
                    let vert_id = PointIndex::from((3 * self.num_obj_faces + corner) as u32);
                    let tri_index = Self::triangulate(t, corner);
                    self.map_point_to_vertex_indices(target, vert_id, &indices[tri_index as usize]);
                    if added_edge_att_id >= 0 {
                        let avi =
                            AttributeValueIndex::from(Self::is_new_edge(nt, t, corner) as u32);
                        if let Some(att) = target.as_pc_mut().attribute_mut(added_edge_att_id) {
                            att.set_point_map_entry(vert_id, avi);
                        }
                    }
                }
                self.num_obj_faces += 1;
            }
        } else {
            parser_utils::skip_whitespace(buffer);
            let mut num_indices = 0;
            let mut is_end = false;
            while buffer.peek(&mut c) && c != b'\n' {
                if parser_utils::peek_whitespace(buffer, &mut is_end) {
                    buffer.advance(1);
                } else {
                    num_indices += 1;
                    while !parser_utils::peek_whitespace(buffer, &mut is_end) && !is_end {
                        buffer.advance(1);
                    }
                }
            }
            if num_indices > 3 {
                self.has_polygons = true;
            }
            if num_indices < 3 || num_indices > MAX_CORNERS as i32 {
                *status = Status::new(
                    StatusCode::DracoError,
                    "Invalid number of indices on a face",
                );
                return false;
            }
            self.num_obj_faces += num_indices - 2;
        }
        parser_utils::skip_line(buffer);
        true
    }

    fn parse_material_lib(&mut self, buffer: &mut DecoderBuffer, status: &mut Status) -> bool {
        if !self.material_name_to_id.is_empty() {
            return false;
        }
        let mut c: [u8; 6] = [0; 6];
        if !buffer.peek(&mut c) {
            return false;
        }
        if &c != b"mtllib" {
            return false;
        }
        buffer.advance(6);
        let mut line_buffer = parser_utils::parse_line_into_decoder_buffer(buffer);
        parser_utils::skip_whitespace(&mut line_buffer);
        self.material_file_name.clear();
        if !parser_utils::parse_string(&mut line_buffer, &mut self.material_file_name) {
            *status = Status::new(StatusCode::DracoError, "Failed to parse material file name");
            return true;
        }
        parser_utils::skip_line(&mut line_buffer);

        // Get buffer version BEFORE calling parse_material_file.
        let buffer_version = buffer.bitstream_version();

        if !self.material_file_name.is_empty() {
            let material_file_name = self.material_file_name.clone();
            // Collect material file during counting mode.
            if self.counting_mode {
                self.collected_material_files
                    .push(material_file_name.clone());
            }
            if !self.parse_material_file(&material_file_name, buffer_version, status) {
                return true;
            }
        }
        true
    }

    fn parse_material(&mut self, buffer: &mut DecoderBuffer, _status: &mut Status) -> bool {
        if !self.counting_mode && self.material_att_id < 0 {
            return false;
        }
        let mut c: [u8; 6] = [0; 6];
        if !buffer.peek(&mut c) {
            return false;
        }
        if &c != b"usemtl" {
            return false;
        }
        buffer.advance(6);
        let mut line_buffer = parser_utils::parse_line_into_decoder_buffer(buffer);
        parser_utils::skip_whitespace(&mut line_buffer);
        let mut mat_name = String::new();
        parser_utils::parse_line(&mut line_buffer, Some(&mut mat_name));
        if mat_name.is_empty() {
            return false;
        }
        if let Some(id) = self.material_name_to_id.get(&mat_name).copied() {
            self.last_material_id = id;
        } else {
            self.last_material_id = self.num_materials;
            self.material_name_to_id
                .insert(mat_name, self.num_materials);
            self.num_materials += 1;
        }
        true
    }

    fn parse_object(&mut self, buffer: &mut DecoderBuffer, _status: &mut Status) -> bool {
        let mut c: [u8; 2] = [0, 0];
        if !buffer.peek(&mut c) {
            return false;
        }
        if &c != b"o " {
            return false;
        }
        buffer.advance(1);
        let mut line_buffer = parser_utils::parse_line_into_decoder_buffer(buffer);
        parser_utils::skip_whitespace(&mut line_buffer);
        let mut obj_name = String::new();
        if !parser_utils::parse_string(&mut line_buffer, &mut obj_name) {
            return false;
        }
        if obj_name.is_empty() {
            return true;
        }
        if let Some(id) = self.obj_name_to_id.get(&obj_name).copied() {
            self.last_sub_obj_id = id;
        } else {
            let num_obj = self.obj_name_to_id.len() as i32;
            self.obj_name_to_id.insert(obj_name, num_obj);
            self.last_sub_obj_id = num_obj;
        }
        true
    }

    fn parse_vertex_indices(
        &mut self,
        buffer: &mut DecoderBuffer,
        out_indices: &mut [i32; 3],
    ) -> bool {
        parser_utils::skip_characters(buffer, " \t");
        if !parser_utils::parse_signed_int(buffer, &mut out_indices[0]) || out_indices[0] == 0 {
            return false;
        }
        out_indices[1] = 0;
        out_indices[2] = 0;
        let mut ch: u8 = 0;
        if !buffer.peek(&mut ch) {
            return true;
        }
        if ch != b'/' {
            return true;
        }
        buffer.advance(1);
        if !buffer.peek(&mut ch) {
            return false;
        }
        if ch != b'/' {
            if !parser_utils::parse_signed_int(buffer, &mut out_indices[1]) || out_indices[1] == 0 {
                return false;
            }
        }
        if !buffer.peek(&mut ch) {
            return true;
        }
        if ch == b'/' {
            buffer.advance(1);
            if !parser_utils::parse_signed_int(buffer, &mut out_indices[2]) || out_indices[2] == 0 {
                return false;
            }
        }
        true
    }

    fn map_point_to_vertex_indices(
        &mut self,
        target: &mut DecodeTarget,
        vert_id: PointIndex,
        indices: &[i32; 3],
    ) {
        let pos_att_id = self.pos_att_id;
        let tex_att_id = self.tex_att_id;
        let norm_att_id = self.norm_att_id;
        let material_att_id = self.material_att_id;
        let sub_obj_att_id = self.sub_obj_att_id;
        let num_positions = self.num_positions;
        let num_tex_coords = self.num_tex_coords;
        let num_normals = self.num_normals;
        let last_material_id = self.last_material_id;
        let last_sub_obj_id = self.last_sub_obj_id;

        let pc = target.as_pc_mut();

        if indices[0] > 0 {
            if let Some(att) = pc.attribute_mut(pos_att_id) {
                att.set_point_map_entry(
                    vert_id,
                    AttributeValueIndex::from((indices[0] - 1) as u32),
                );
            }
        } else if indices[0] < 0 {
            if let Some(att) = pc.attribute_mut(pos_att_id) {
                att.set_point_map_entry(
                    vert_id,
                    AttributeValueIndex::from((num_positions + indices[0]) as u32),
                );
            }
        }

        if tex_att_id >= 0 {
            if indices[1] > 0 {
                if let Some(att) = pc.attribute_mut(tex_att_id) {
                    att.set_point_map_entry(
                        vert_id,
                        AttributeValueIndex::from((indices[1] - 1) as u32),
                    );
                }
            } else if indices[1] < 0 {
                if let Some(att) = pc.attribute_mut(tex_att_id) {
                    att.set_point_map_entry(
                        vert_id,
                        AttributeValueIndex::from((num_tex_coords + indices[1]) as u32),
                    );
                }
            } else {
                if let Some(att) = pc.attribute_mut(tex_att_id) {
                    att.set_point_map_entry(vert_id, AttributeValueIndex::from(0u32));
                }
            }
        }

        if norm_att_id >= 0 {
            if indices[2] > 0 {
                if let Some(att) = pc.attribute_mut(norm_att_id) {
                    att.set_point_map_entry(
                        vert_id,
                        AttributeValueIndex::from((indices[2] - 1) as u32),
                    );
                }
            } else if indices[2] < 0 {
                if let Some(att) = pc.attribute_mut(norm_att_id) {
                    att.set_point_map_entry(
                        vert_id,
                        AttributeValueIndex::from((num_normals + indices[2]) as u32),
                    );
                }
            } else {
                if let Some(att) = pc.attribute_mut(norm_att_id) {
                    att.set_point_map_entry(vert_id, AttributeValueIndex::from(0u32));
                }
            }
        }

        if material_att_id >= 0 {
            if let Some(att) = pc.attribute_mut(material_att_id) {
                att.set_point_map_entry(
                    vert_id,
                    AttributeValueIndex::from(last_material_id as u32),
                );
            }
        }

        if sub_obj_att_id >= 0 {
            if let Some(att) = pc.attribute_mut(sub_obj_att_id) {
                att.set_point_map_entry(vert_id, AttributeValueIndex::from(last_sub_obj_id as u32));
            }
        }
    }

    // Parse material file with its own local buffer.
    fn parse_material_file(
        &mut self,
        file_name: &str,
        buffer_version: u16,
        status: &mut Status,
    ) -> bool {
        let full_path = get_full_path(file_name, &self.input_file_name);
        let mut mat_data: Vec<u8> = Vec::new();
        if !read_file_to_buffer(&full_path, &mut mat_data) {
            return false;
        }

        let mut mat_buffer = DecoderBuffer::new();
        mat_buffer.init_with_version(&mat_data, buffer_version);

        self.num_materials = 0;
        while self.parse_material_file_definition(&mut mat_buffer, status) {}
        true
    }

    fn parse_material_file_definition(
        &mut self,
        buffer: &mut DecoderBuffer,
        _status: &mut Status,
    ) -> bool {
        let mut c: u8 = 0;
        parser_utils::skip_whitespace(buffer);
        if !buffer.peek(&mut c) {
            return false;
        }
        if c == b'#' {
            parser_utils::skip_line(buffer);
            return true;
        }
        let mut str_val = String::new();
        if !parser_utils::parse_string(buffer, &mut str_val) {
            return false;
        }
        if str_val == "newmtl" {
            parser_utils::skip_whitespace(buffer);
            parser_utils::parse_line(buffer, Some(&mut str_val));
            if str_val.is_empty() {
                return false;
            }
            self.material_name_to_id.insert(str_val, self.num_materials);
            self.num_materials += 1;
        }
        true
    }

    fn triangulate(tri_index: i32, tri_corner: i32) -> i32 {
        if tri_corner == 0 {
            0
        } else {
            tri_index + tri_corner
        }
    }

    fn is_new_edge(tri_count: i32, tri_index: i32, tri_corner: i32) -> i32 {
        if tri_index != tri_count - 1 && tri_corner == 1 {
            1
        } else {
            0
        }
    }
}

impl Default for ObjDecoder {
    fn default() -> Self {
        Self::new()
    }
}
